use crate::CountryTag;
use crate::economy::digital_platform::{DigitalPlatform, DigitalPlatformId, DigitalPlatformKind};
use crate::economy::sector::SectorKind;
use crate::system::{System, SystemContext};
use crate::world::WorldCommand;
use crate::world::map::RegionModifier;
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct DigitalEconomySystem {
    country_tags: Vec<CountryTag>,
    exchange_rates: Vec<f64>,
}

impl System for DigitalEconomySystem {
    fn id(&self) -> &'static str {
        "core:digital_economy_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        self.country_tags.clear();
        self.exchange_rates.clear();
        for c in ctx.countries.iter() {
            self.country_tags.push(c.tag);
            self.exchange_rates
                .push(c.economy.exchange_rate.max(0.0001));
        }

        self.sync_platforms_from_facilities(ctx);
        self.update_platform_capacities(ctx);
        self.distribute_consumer_subscriptions(ctx);
        self.distribute_enterprise_subscriptions(ctx);
        self.apply_digital_modifiers(ctx);
    }
}

impl DigitalEconomySystem {
    fn sync_platforms_from_facilities(&self, ctx: &mut SystemContext) {
        let recipe_software_id = ctx.defs.recipes.get_id("recipe_software_development");
        let recipe_cloud_id = ctx.defs.recipes.get_id("recipe_cloud_hosting");
        let recipe_platforms_id = ctx.defs.recipes.get_id("recipe_digital_platforms");

        let mut next_id = ctx.digital_platforms.len() as u32;

        for facility in ctx.facilities.iter() {
            if facility.sector_kind != SectorKind::InformationTechnology {
                continue;
            }

            let comp_name = ctx
                .companies
                .iter()
                .find(|c| c.id == facility.company_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Tech Enterprise".to_string());

            let ex_rate = self
                .country_tags
                .iter()
                .position(|t| *t == facility.country_tag)
                .map(|idx| self.exchange_rates[idx])
                .unwrap_or(1.0);

            for line in &facility.lines {
                let (kind, fee_base, suffix) = if Some(line.recipe_id) == recipe_software_id {
                    (
                        DigitalPlatformKind::BusinessSoftware,
                        45.0,
                        "Enterprise Software",
                    )
                } else if Some(line.recipe_id) == recipe_cloud_id {
                    (
                        DigitalPlatformKind::EnterpriseCloud,
                        60.0,
                        "Cloud Infrastructure",
                    )
                } else if Some(line.recipe_id) == recipe_platforms_id {
                    (
                        DigitalPlatformKind::ConsumerServices,
                        15.0,
                        "Digital Ecosystem",
                    )
                } else {
                    continue;
                };

                let already_exists = ctx.digital_platforms.iter().any(|p| {
                    p.company_id == facility.company_id
                        && p.facility_id == facility.id
                        && p.kind == kind
                });

                if !already_exists {
                    let platform_name = format!("{comp_name} {suffix}");
                    let max_subscribers = (line.installed_capacity as u64) * 1200;
                    let monthly_fee = (fee_base * ex_rate).round().max(1.0);
                    let new_platform = DigitalPlatform::new(
                        DigitalPlatformId(next_id),
                        facility.company_id,
                        facility.id,
                        facility.region_id,
                        facility.country_tag,
                        platform_name,
                        kind,
                        max_subscribers,
                        monthly_fee,
                    );
                    next_id += 1;
                    ctx.commands
                        .push(WorldCommand::SpawnDigitalPlatform(new_platform));
                }
            }
        }
    }

    fn update_platform_capacities(&self, ctx: &mut SystemContext) {
        let recipe_software_id = ctx.defs.recipes.get_id("recipe_software_development");
        let recipe_cloud_id = ctx.defs.recipes.get_id("recipe_cloud_hosting");
        let recipe_platforms_id = ctx.defs.recipes.get_id("recipe_digital_platforms");

        for platform in ctx.digital_platforms.iter_mut() {
            platform.active_subscribers = 0;
            platform.monthly_revenue = 0.0;

            let Some(facility) = ctx.facilities.iter().find(|f| f.id == platform.facility_id)
            else {
                platform.is_active = false;
                continue;
            };

            let target_recipe_id = match platform.kind {
                DigitalPlatformKind::BusinessSoftware => recipe_software_id,
                DigitalPlatformKind::EnterpriseCloud => recipe_cloud_id,
                DigitalPlatformKind::ConsumerServices => recipe_platforms_id,
            };

            let matching_line = facility
                .lines
                .iter()
                .find(|l| Some(l.recipe_id) == target_recipe_id);

            let Some(line) = matching_line else {
                platform.is_active = false;
                continue;
            };

            if line.installed_capacity == 0 {
                platform.is_active = false;
                platform.max_subscribers = 0;
                platform.quality = 0.5;
                continue;
            }

            platform.is_active = true;
            platform.max_subscribers = (line.installed_capacity as u64) * 1200;

            let country_tech_bonus = ctx
                .countries
                .iter()
                .find(|c| c.tag == platform.country_tag)
                .map(|c| {
                    let mut b = 0.0;
                    if let Some(t_id) = ctx.defs.technologies.get_id("tech_microelectronics") {
                        if c.has_technology(t_id) {
                            b += 0.2;
                        }
                    }
                    if let Some(t_id) = ctx.defs.technologies.get_id("tech_telecommunications") {
                        if c.has_technology(t_id) {
                            b += 0.2;
                        }
                    }
                    if let Some(t_id) = ctx.defs.technologies.get_id("tech_advanced_semiconductors")
                    {
                        if c.has_technology(t_id) {
                            b += 0.4;
                        }
                    }
                    b
                })
                .unwrap_or(0.0);

            platform.quality = (line.utilization * 1.4 + country_tech_bonus).clamp(0.5, 3.5);
        }
    }

    fn distribute_consumer_subscriptions(&self, ctx: &mut SystemContext) {
        let consumer_indices: Vec<usize> = ctx
            .digital_platforms
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_active && p.kind == DigitalPlatformKind::ConsumerServices)
            .map(|(i, _)| i)
            .collect();

        if consumer_indices.is_empty() {
            return;
        }

        for region in ctx.regions.iter_mut() {
            let Some(owner_tag) = region.owner else {
                continue;
            };
            let ex_rate_reg = self
                .country_tags
                .iter()
                .position(|t| *t == owner_tag)
                .map(|idx| self.exchange_rates[idx])
                .unwrap_or(1.0);

            let mut potential_subscribers = 0u64;
            for pop in &region.pops {
                let pop_type_name = ctx
                    .defs
                    .pop_types
                    .get(pop.pop_type_id)
                    .map(|d| d.key.as_str())
                    .unwrap_or("");

                let factor = match pop_type_name {
                    "professionals" | "engineers" => 0.90,
                    "state_employees" => 0.80,
                    "service_workers" => 0.65,
                    "workers" => 0.40,
                    _ => 0.20,
                };

                let emp_count = (pop.employed.count as f64 * factor) as u64;
                let unemp_count = (pop.unemployed.count as f64 * (factor * 0.5)) as u64;
                potential_subscribers += emp_count + unemp_count;
            }

            if potential_subscribers == 0 {
                continue;
            }

            let mut platform_scores: Vec<(usize, f64)> = Vec::with_capacity(consumer_indices.len());
            let mut total_score = 0.0;

            for &p_idx in &consumer_indices {
                let platform = &ctx.digital_platforms[p_idx];
                if platform.available_capacity() == 0 {
                    continue;
                }

                let ex_rate_plat = self
                    .country_tags
                    .iter()
                    .position(|t| *t == platform.country_tag)
                    .map(|idx| self.exchange_rates[idx])
                    .unwrap_or(1.0);

                let fx_ratio = ex_rate_reg / ex_rate_plat;
                let mut score = platform.attractiveness(fx_ratio);

                if platform.country_tag == owner_tag {
                    score *= 1.6;
                }

                platform_scores.push((p_idx, score));
                total_score += score;
            }

            if total_score <= 0.0 {
                continue;
            }

            let mut remaining_to_assign = potential_subscribers;

            for &(p_idx, score) in &platform_scores {
                if remaining_to_assign == 0 {
                    break;
                }

                let platform = &mut ctx.digital_platforms[p_idx];
                let quota = ((potential_subscribers as f64) * (score / total_score)).round() as u64;
                let to_add = quota
                    .min(platform.available_capacity())
                    .min(remaining_to_assign);

                if to_add == 0 {
                    continue;
                }

                platform.active_subscribers += to_add;
                let revenue = (to_add as f64) * platform.monthly_fee;
                platform.monthly_revenue += revenue;

                if let Some(comp) = ctx
                    .companies
                    .iter_mut()
                    .find(|c| c.id == platform.company_id)
                {
                    comp.balance += revenue;
                }

                remaining_to_assign = remaining_to_assign.saturating_sub(to_add);
            }
        }
    }

    fn distribute_enterprise_subscriptions(&self, ctx: &mut SystemContext) {
        let b2b_indices: Vec<usize> = ctx
            .digital_platforms
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.is_active
                    && (p.kind == DigitalPlatformKind::BusinessSoftware
                        || p.kind == DigitalPlatformKind::EnterpriseCloud)
            })
            .map(|(i, _)| i)
            .collect();

        if b2b_indices.is_empty() {
            return;
        }

        for region in ctx.regions.iter_mut() {
            if region.owner.is_none() {
                continue;
            }

            let mut b2b_demand = 0u64;
            for st in &region.structures {
                b2b_demand += (st.level as u64) * 80;
            }
            b2b_demand += (region.facilities.len() as u64) * 200;

            if b2b_demand == 0 {
                continue;
            }

            for &p_idx in &b2b_indices {
                if b2b_demand == 0 {
                    break;
                }

                let platform = &mut ctx.digital_platforms[p_idx];
                let capacity = platform.available_capacity();
                if capacity == 0 {
                    continue;
                }

                let to_allocate = b2b_demand.min(capacity);
                platform.active_subscribers += to_allocate;
                let revenue = (to_allocate as f64) * platform.monthly_fee;
                platform.monthly_revenue += revenue;

                if let Some(comp) = ctx
                    .companies
                    .iter_mut()
                    .find(|c| c.id == platform.company_id)
                {
                    comp.balance += revenue;
                }

                b2b_demand = b2b_demand.saturating_sub(to_allocate);
            }
        }
    }

    fn apply_digital_modifiers(&self, ctx: &mut SystemContext) {
        for region in ctx.regions.iter_mut() {
            let mut region_subscribers = 0u64;
            for platform in ctx.digital_platforms.iter() {
                if platform.region_id == region.id {
                    region_subscribers += platform.active_subscribers;
                }
            }

            let pop_count = region.population().max(1);
            let penetration = (region_subscribers as f64 / pop_count as f64).clamp(0.0, 1.0);

            if penetration > 0.15 {
                let productivity_boost = penetration * 0.18;
                region
                    .modifiers
                    .add(RegionModifier::LaborProductivity, productivity_boost);
                region
                    .modifiers
                    .add(RegionModifier::TaxEfficiency, penetration * 0.08);
            }
        }
    }
}
