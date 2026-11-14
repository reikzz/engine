use crate::bridge::commands::{ClientCommand, SimulationSpeed};
use crate::bridge::snapshot::{
    CompanySnapshot, ConstructionOptionSnapshot, CountrySnapshot, FacilityDetailSnapshot,
    InspectedRegionDetail, ProvinceSnapshot, RegionSnapshot, ResourcePriceSnapshot,
    SystemMetricSnapshot, TechItemSnapshot, WorldSnapshot,
};
use crossbeam_channel::{Receiver, Sender};
use engine_core::economy::{CompanyId, RecipeId};
use engine_core::loader::DataLoader;
use engine_core::world::map::RegionId;
use engine_core::world::{CountryTag, TechnologyId};
use engine_core::{SystemProfiler, World};
use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub struct SimulationWorker {
    command_receiver: Receiver<ClientCommand>,
    snapshot_sender: Sender<WorldSnapshot>,
    speed: SimulationSpeed,
    world: World,
    profiler: SystemProfiler,
    total_ticks: u64,
    inspected_region: Option<u16>,
    provinces_cache: BTreeMap<u16, ProvinceSnapshot>,
    cached_construction_options: Vec<ConstructionOptionSnapshot>,
    cached_tech_tree: Vec<TechItemSnapshot>,
    last_cache_ticks: u64,
}

impl SimulationWorker {
    pub fn spawn(
        defs_path: &Path,
        scenario_path: &Path,
        command_receiver: Receiver<ClientCommand>,
        snapshot_sender: Sender<WorldSnapshot>,
    ) -> Result<thread::JoinHandle<()>, Box<dyn std::error::Error>> {
        let defs = DataLoader::load_game_defs(defs_path)?;
        let world = DataLoader::load_world(scenario_path, defs)?;
        let profiler = SystemProfiler::default();

        let mut provinces_cache = BTreeMap::new();
        for p in &world.provinces {
            provinces_cache.insert(
                p.id.0,
                ProvinceSnapshot {
                    province_id: p.id.0,
                    region_id: p.region_id.0,
                    terrain: format!("{:?}", p.terrain),
                    controller: p.controller.map(|t| t.as_str().to_string()),
                },
            );
        }

        let target_country = world
            .countries
            .first()
            .map(|c| c.tag)
            .unwrap_or_else(|| CountryTag::new("USA"));

        let cached_construction_options =
            Self::compute_construction_options(&world, target_country);
        let cached_tech_tree = Self::compute_tech_tree(&world, target_country);

        let mut worker = Self {
            command_receiver,
            snapshot_sender,
            speed: SimulationSpeed::Speed1x,
            world,
            profiler,
            total_ticks: 0,
            inspected_region: None,
            provinces_cache,
            cached_construction_options,
            cached_tech_tree,
            last_cache_ticks: 0,
        };

        let handle = thread::Builder::new()
            .name("simulation_worker".to_string())
            .spawn(move || {
                worker.run_loop();
            })?;

        Ok(handle)
    }

    fn run_loop(&mut self) {
        let mut last_snapshot_time = Instant::now();
        let mut tick_counter_for_tps = 0u64;
        let mut tps_measure_start = Instant::now();
        let mut measured_tps = 0.0;

        loop {
            self.process_commands();

            let target_interval = match self.speed {
                SimulationSpeed::Paused => None,
                SimulationSpeed::Speed1x => Some(Duration::from_millis(41)),
                SimulationSpeed::Speed2x => Some(Duration::from_millis(20)),
                SimulationSpeed::Speed5x => Some(Duration::from_millis(8)),
                SimulationSpeed::MaxSpeed => Some(Duration::ZERO),
            };

            if let Some(delay) = target_interval {
                let tick_start = Instant::now();
                self.world.tick_profiled(&mut self.profiler);
                self.total_ticks = self.total_ticks.saturating_add(1);
                tick_counter_for_tps = tick_counter_for_tps.saturating_add(1);

                if let Some(remaining) = delay.checked_sub(tick_start.elapsed()) {
                    if remaining > Duration::from_millis(1) {
                        thread::sleep(remaining);
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(15));
            }

            if tps_measure_start.elapsed() >= Duration::from_millis(500) {
                let elapsed_secs = tps_measure_start.elapsed().as_secs_f64();
                measured_tps = (tick_counter_for_tps as f64) / elapsed_secs.max(1e-6);
                tick_counter_for_tps = 0;
                tps_measure_start = Instant::now();
            }

            if last_snapshot_time.elapsed() >= Duration::from_millis(50) {
                let snapshot = self.create_snapshot(measured_tps);
                let _ = self.snapshot_sender.try_send(snapshot);
                last_snapshot_time = Instant::now();
            }
        }
    }

    fn process_commands(&mut self) {
        while let Ok(cmd) = self.command_receiver.try_recv() {
            match cmd {
                ClientCommand::ChangeSpeed(new_speed) => {
                    self.speed = new_speed;
                }
                ClientCommand::StepOnce => {
                    self.world.tick_profiled(&mut self.profiler);
                    self.total_ticks = self.total_ticks.saturating_add(1);
                }
                ClientCommand::InspectRegion(region_id) => {
                    self.inspected_region = region_id;
                }
                ClientCommand::CommissionFacilityConstruction {
                    country,
                    region_id,
                    recipe_id,
                } => {
                    let company_id = self
                        .world
                        .companies
                        .iter()
                        .find(|c| c.country_tag == country)
                        .map(|c| c.id)
                        .unwrap_or(CompanyId(1));

                    let _ = self.world.commission_facility_construction(
                        country,
                        company_id,
                        RecipeId(recipe_id),
                        RegionId(region_id),
                        None,
                    );
                }
                ClientCommand::UpgradeInfrastructure { country, region_id } => {
                    if let Some(r) = self.world.region(RegionId(region_id)) {
                        if let Some(s) = r.structures.first() {
                            let _ = self.world.commission_structure_upgrade(
                                country,
                                RegionId(region_id),
                                s.def_id,
                                None,
                            );
                        }
                    }
                }
                ClientCommand::StartResearch { country, tech_id } => {
                    let _ = self
                        .world
                        .set_active_research(country, TechnologyId(tech_id));
                    self.last_cache_ticks = 0;
                }
                ClientCommand::SetTaxRates {
                    country,
                    corporate_tax,
                    income_tax,
                    tariff_rate,
                } => {
                    let _ =
                        self.world
                            .set_tax_rates(country, corporate_tax, income_tax, tariff_rate);
                }
                ClientCommand::SetWelfarePolicy { country, policy } => {
                    let _ = self.world.set_welfare_policy(country, policy);
                }
                ClientCommand::IssueEmergencyWelfare { country, amount } => {
                    let _ = self.world.issue_emergency_welfare(country, amount);
                }
                ClientCommand::TakeNationalLoan { country, amount } => {
                    let _ = self.world.take_national_loan(country, amount);
                }
                ClientCommand::RepayNationalDebt { country, amount } => {
                    let _ = self.world.repay_national_debt(country, amount);
                }
            }
        }
    }

    fn compute_construction_options(
        world: &World,
        target_country: CountryTag,
    ) -> Vec<ConstructionOptionSnapshot> {
        let mut construction_options = Vec::with_capacity(world.defs.recipes.all().len());
        for (idx, recipe) in world.defs.recipes.all().iter().enumerate() {
            let recipe_id = RecipeId(idx as u16);
            let estimate = world.calculate_facility_construction_cost(target_country, recipe_id);
            let (build_days, materials_text, tech_unlocked, req_tech_name) =
                if let Ok(est) = estimate {
                    let mat_summary = est
                        .materials
                        .iter()
                        .map(|m| {
                            let res_name = world
                                .defs
                                .resources
                                .get(m.resource_id)
                                .map(|r| r.key.as_str())
                                .unwrap_or("Resource");
                            format!("{}x {:.0}", res_name, m.total_quantity)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let req_name = recipe
                        .required_tech
                        .and_then(|tid| world.defs.technologies.get(tid))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "None".to_string());
                    (
                        est.build_days as u32,
                        mat_summary,
                        est.is_tech_unlocked,
                        req_name,
                    )
                } else {
                    (
                        recipe.build_time_days as u32,
                        "-".to_string(),
                        true,
                        "-".to_string(),
                    )
                };

            construction_options.push(ConstructionOptionSnapshot {
                recipe_id: idx as u16,
                key: recipe.key.clone(),
                name: recipe.key.clone(),
                sector: format!("{:?}", recipe.sector),
                build_time_days: build_days,
                cost_text: format!("{} days", build_days),
                materials_summary: materials_text,
                tech_unlocked,
                required_tech_name: req_tech_name,
            });
        }
        construction_options
    }

    fn compute_tech_tree(world: &World, target_country: CountryTag) -> Vec<TechItemSnapshot> {
        if let Ok(overview) = world.get_technologies_overview(target_country) {
            overview
                .into_iter()
                .map(|t| {
                    let is_active =
                        t.status == engine_core::world::TechnologyResearchStatus::Active;
                    let is_completed =
                        t.status == engine_core::world::TechnologyResearchStatus::Researched;
                    let can_research =
                        t.status == engine_core::world::TechnologyResearchStatus::Available;
                    TechItemSnapshot {
                        id: t.id.0,
                        key: t.key,
                        name: t.name,
                        category: "Applied Science".to_string(),
                        cost_points: t.cost as u32,
                        progress_ratio: t.progress_ratio as f32,
                        status: format!("{:?}", t.status),
                        is_active,
                        is_completed,
                        can_research,
                        unlocks_summary: format!("Cost: {:.0} pts", t.cost),
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn create_snapshot(&mut self, tps: f64) -> WorldSnapshot {
        let mut countries = BTreeMap::new();
        for c in &self.world.countries {
            let mut total_pop = 0u64;
            let mut employed_pop = 0u64;

            for r in self.world.regions.iter().filter(|r| r.owner == Some(c.tag)) {
                for p in &r.pops {
                    total_pop = total_pop.saturating_add(p.count());
                    employed_pop = employed_pop.saturating_add(p.employed.count);
                }
            }

            countries.insert(
                c.tag.as_str().to_string(),
                CountrySnapshot {
                    tag: c.tag.as_str().to_string(),
                    currency_symbol: c.economy.currency_symbol.clone(),
                    treasury: c.economy.treasury,
                    national_debt: c.economy.national_debt,
                    total_population: total_pop,
                    employed_population: employed_pop,
                    nominal_gdp: c.economy.nominal_gdp,
                    corporate_tax: c.economy.corporate_tax,
                    income_tax: c.economy.income_tax,
                    tariff_rate: c.economy.tariff_rate,
                },
            );
        }

        let mut regions = BTreeMap::new();
        for r in &self.world.regions {
            let total_pop: u64 = r.pops.iter().map(|p| p.count()).sum();
            let employed_pop: u64 = r.pops.iter().map(|p| p.employed.count).sum();
            let avg_price = if r.local_prices.is_empty() {
                0.0
            } else {
                r.local_prices.iter().copied().sum::<f64>() / (r.local_prices.len() as f64)
            };

            regions.insert(
                r.id.0,
                RegionSnapshot {
                    region_id: r.id.0,
                    owner: r.owner.map(|t| t.as_str().to_string()),
                    total_population: total_pop,
                    employed_population: employed_pop,
                    infrastructure_level: r.infrastructure_level,
                    infrastructure_capacity: r.infrastructure_capacity,
                    market_access: r.market_access,
                    average_price: avg_price,
                    facility_count: r.facilities.len(),
                },
            );
        }

        let provinces = self.provinces_cache.clone();

        let mut system_metrics = Vec::new();
        for item in self.profiler.records() {
            system_metrics.push(SystemMetricSnapshot {
                name: item.id.to_string(),
                last_duration_micros: item.min_duration().as_micros() as u64,
                average_duration_micros: item.average_duration().as_micros() as f64,
            });
        }

        let mut local_market_prices = BTreeMap::new();
        for c in &self.world.countries {
            let mut list = Vec::new();
            for (idx, r) in self.world.defs.resources.all().iter().enumerate() {
                let res_id = engine_core::economy::ResourceId(idx as u16);
                let (supplied, demanded, price) = if let Some(rm) = c.market.get(res_id) {
                    (rm.supplied, rm.demanded, rm.price)
                } else {
                    (0.0, 0.0, r.base_price)
                };
                let global_q = self.world.global_market.get(res_id);
                let (global_price, global_volume) = if let Some(gq) = global_q {
                    (gq.benchmark_price, gq.daily_volume)
                } else {
                    (r.base_price, 0.0)
                };

                list.push(ResourcePriceSnapshot {
                    id: idx as u16,
                    name: r.key.clone(),
                    base_price: r.base_price,
                    local_price: price,
                    supply: supplied,
                    demand: demanded,
                    global_price,
                    global_volume,
                });
            }
            local_market_prices.insert(c.tag.as_str().to_string(), list);
        }

        let mut global_market_prices = Vec::new();
        for (idx, r) in self.world.defs.resources.all().iter().enumerate() {
            let res_id = engine_core::economy::ResourceId(idx as u16);
            let global_q = self.world.global_market.get(res_id);
            let (global_price, global_volume) = if let Some(gq) = global_q {
                (gq.benchmark_price, gq.daily_volume)
            } else {
                (r.base_price, 0.0)
            };
            global_market_prices.push(ResourcePriceSnapshot {
                id: idx as u16,
                name: r.key.clone(),
                base_price: r.base_price,
                local_price: global_price,
                supply: 0.0,
                demand: 0.0,
                global_price,
                global_volume,
            });
        }

        let mut companies = Vec::with_capacity(self.world.companies.len());
        for c in &self.world.companies {
            companies.push(CompanySnapshot {
                id: c.id.0,
                name: c.name.clone(),
                country_tag: c.country_tag.as_str().to_string(),
                sector: format!("{:?}", c.sector_kind),
                balance: c.balance,
                monthly_revenue: c.monthly_revenue,
                monthly_expenses: c.monthly_expenses,
                last_month_profit: c.monthly_profit(),
            });
        }

        let inspected_region_detail = if let Some(rid) = self.inspected_region {
            if let Some(detail) = self.world.inspect_region(RegionId(rid)) {
                let country_tag_str = detail
                    .owner
                    .map(|t| t.as_str().to_string())
                    .unwrap_or_default();
                let terrain_str = self
                    .world
                    .provinces
                    .iter()
                    .find(|p| p.region_id.0 == rid)
                    .map(|p| format!("{:?}", p.terrain))
                    .unwrap_or_else(|| "Plains".to_string());
                let avg_price = self
                    .world
                    .region(RegionId(rid))
                    .map(|r| {
                        if r.local_prices.is_empty() {
                            0.0
                        } else {
                            r.local_prices.iter().copied().sum::<f64>()
                                / (r.local_prices.len() as f64)
                        }
                    })
                    .unwrap_or(0.0);
                let facilities = detail
                    .facilities
                    .into_iter()
                    .map(|f| {
                        let line_capacity: u32 = f.lines.iter().map(|l| l.installed_capacity).sum();
                        let output_est = f
                            .lines
                            .iter()
                            .map(|l| l.installed_capacity as f64 * l.utilization)
                            .sum();
                        FacilityDetailSnapshot {
                            facility_id: f.facility_id.0,
                            name: f.company_name,
                            sector: format!("{:?}", f.sector_kind),
                            workforce_count: (line_capacity as f64 * 0.8) as u64,
                            workforce_capacity: line_capacity as u64,
                            daily_output: output_est,
                        }
                    })
                    .collect();

                Some(InspectedRegionDetail {
                    region_id: rid,
                    name: format!("Region #{}", rid),
                    country_tag: country_tag_str,
                    terrain: terrain_str,
                    total_pop: detail.total_population,
                    employed_pop: detail.employed_population,
                    satisfaction: detail.average_satisfaction,
                    radicalism: detail.average_radicalism,
                    infrastructure_level: detail.infrastructure_level,
                    infrastructure_capacity: detail.infrastructure_capacity,
                    infrastructure_load: detail.infrastructure_usage,
                    market_access: detail.market_access,
                    average_price: avg_price,
                    facilities,
                })
            } else {
                None
            }
        } else {
            None
        };

        let target_country = self
            .world
            .countries
            .first()
            .map(|c| c.tag)
            .unwrap_or_else(|| CountryTag::new("USA"));

        if self.total_ticks.saturating_sub(self.last_cache_ticks) >= 20 {
            self.cached_construction_options =
                Self::compute_construction_options(&self.world, target_country);
            self.cached_tech_tree = Self::compute_tech_tree(&self.world, target_country);
            self.last_cache_ticks = self.total_ticks;
        }

        let construction_options = self.cached_construction_options.clone();
        let tech_tree = self.cached_tech_tree.clone();

        let (active_tech_name, active_tech_category, active_tech_progress) = {
            let active = self.world.get_active_research_overview(target_country);
            if let Some(a) = active {
                (
                    Some(a.name),
                    Some("Science".to_string()),
                    a.progress_ratio as f32,
                )
            } else {
                (None, None, 0.0)
            }
        };

        WorldSnapshot {
            date_display: self.world.date.to_string(),
            total_ticks: self.total_ticks,
            ticks_per_second: tps,
            countries,
            regions,
            provinces,
            system_metrics,
            local_market_prices,
            global_market_prices,
            companies,
            inspected_region_detail,
            construction_options,
            tech_tree,
            active_tech_name,
            active_tech_category,
            active_tech_progress,
        }
    }
}
