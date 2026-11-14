use crate::economy::finance::{BankLoan, BudgetSource};
use crate::economy::{CompanyId, CompanyState, Ownership, ResourceId, SectorKind};
use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;
use crate::world::{ConstructionProject, ConstructionTarget, WorldCommand};

pub struct InvestmentSystem;

impl System for InvestmentSystem {
    fn id(&self) -> &'static str {
        "core:investment_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_quarter()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        struct LineExpansion {
            comp_idx: usize,
            fac_idx: usize,
            line_idx: usize,
            step: u32,
            cost: f64,
        }

        let mut country_rates = std::collections::HashMap::with_capacity(ctx.countries.len());
        for c in ctx.countries.iter() {
            country_rates.insert(c.tag, c.economy.exchange_rate.max(0.001));
        }

        let mut company_facilities: std::collections::HashMap<CompanyId, Vec<usize>> =
            std::collections::HashMap::with_capacity(ctx.companies.len());
        for (fac_idx, facility) in ctx.facilities.iter().enumerate() {
            company_facilities
                .entry(facility.company_id)
                .or_default()
                .push(fac_idx);
        }

        let mut expansions: Vec<LineExpansion> = Vec::new();

        for (comp_idx, comp) in ctx.companies.iter().enumerate() {
            if comp.reinvestment_ratio <= 0.0 || comp.last_month_profit < 0.0 {
                continue;
            }

            let exchange_rate = country_rates.get(&comp.country_tag).copied().unwrap_or(1.0);

            let mut available_balance = comp.balance;
            let min_reserve = 5_000.0 * exchange_rate;
            if available_balance < min_reserve {
                continue;
            }

            let Some(fac_indices) = company_facilities.get(&comp.id) else {
                continue;
            };

            for &fac_idx in fac_indices {
                let facility = &ctx.facilities[fac_idx];

                for (line_idx, line) in facility.lines.iter().enumerate() {
                    let max_cap = line.natural_max_capacity.unwrap_or(u32::MAX);
                    if line.installed_capacity >= max_cap {
                        continue;
                    }

                    if line.utilization < 0.65 || line.consecutive_idle_days > 0 {
                        continue;
                    }

                    let Some(recipe) = ctx.defs.recipes.get(line.recipe_id) else {
                        continue;
                    };
                    if let Some(req_tech) = recipe.required_tech {
                        let country_has_tech = ctx
                            .countries
                            .iter()
                            .find(|c| c.tag == facility.country_tag)
                            .map(|c| c.has_technology(req_tech))
                            .unwrap_or(true);
                        if !country_has_tech {
                            continue;
                        }
                    }
                    let expansion_cost = (recipe.base_expansion_cost * 0.05 * exchange_rate)
                        .max(2_000.0 * exchange_rate);

                    let available_for_expansion = (available_balance - min_reserve).max(0.0);
                    let max_steps = (available_for_expansion / expansion_cost).floor() as u32;
                    let cap_deficit = max_cap.saturating_sub(line.installed_capacity);
                    let step = max_steps.min(cap_deficit).clamp(1, 100);
                    let total_cost = expansion_cost * (step as f64);

                    if available_balance >= total_cost + min_reserve {
                        available_balance -= total_cost;
                        expansions.push(LineExpansion {
                            comp_idx,
                            fac_idx,
                            line_idx,
                            step,
                            cost: total_cost,
                        });
                    }
                }
            }
        }

        for exp in expansions {
            ctx.companies[exp.comp_idx].balance -= exp.cost;
            ctx.facilities[exp.fac_idx].lines[exp.line_idx].installed_capacity += exp.step;
        }

        for country in ctx.countries.iter_mut() {
            let max_queue_len = 10;
            if country.construction_queue.len() >= max_queue_len {
                continue;
            }

            let mut resource_deficit: Vec<(ResourceId, f64)> = Vec::new();
            let country_market = &country.market;

            for (res_id, market) in country_market.iter() {
                if market.last_demanded <= 0.0 {
                    continue;
                }

                if market.buy_fulfillment < 0.95 || (market.price / market.base_price > 1.10) {
                    resource_deficit.push((res_id, market.last_demanded));
                }
            }

            if resource_deficit.is_empty() {
                continue;
            }

            resource_deficit
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut started_projects = 0;
            let max_starts_per_quarter = 2;

            for (res_id, _) in resource_deficit {
                if started_projects >= max_starts_per_quarter
                    || country.construction_queue.len() >= max_queue_len
                {
                    break;
                }
                let already_in_construction =
                    country
                        .construction_queue
                        .iter()
                        .any(|project| match project.target {
                            ConstructionTarget::NewFacility { recipe_id, .. } => ctx
                                .defs
                                .recipes
                                .get(recipe_id)
                                .map(|r| r.outputs.iter().any(|(out_res, _)| *out_res == res_id))
                                .unwrap_or(false),
                            _ => false,
                        });

                if already_in_construction {
                    continue;
                }

                let Some((recipe_id, recipe)) = ctx.defs.recipes.iter().find(|(_, r)| {
                    r.outputs.iter().any(|(out_res, _)| *out_res == res_id)
                        && r.required_tech
                            .map(|t| country.has_technology(t))
                            .unwrap_or(true)
                }) else {
                    continue;
                };

                let existing_underutilized = ctx.facilities.iter().any(|f| {
                    f.country_tag == country.tag
                        && f.lines.iter().any(|l| {
                            l.recipe_id == recipe_id
                                && l.installed_capacity > 0
                                && l.utilization < 0.60
                        })
                });
                if existing_underutilized {
                    continue;
                }

                let inputs_available = recipe.inputs.iter().all(|(in_res, _)| {
                    let domestic_avail = country
                        .market
                        .get(*in_res)
                        .map(|m| m.last_supplied > 0.0 || m.buy_fulfillment >= 0.4)
                        .unwrap_or(false);
                    let global_avail = ctx
                        .global_market
                        .get(*in_res)
                        .map(|q| q.global_surplus > 0.1 || q.daily_volume > 0.0)
                        .unwrap_or(false);
                    let world_producer_exists = ctx.facilities.iter().any(|f| {
                        f.lines.iter().any(|l| {
                            l.installed_capacity > 0
                                && ctx
                                    .defs
                                    .recipes
                                    .get(l.recipe_id)
                                    .map(|r| {
                                        r.outputs.iter().any(|(out_res, _)| *out_res == *in_res)
                                    })
                                    .unwrap_or(false)
                        })
                    });
                    domestic_avail || global_avail || world_producer_exists
                });
                if !inputs_available {
                    continue;
                }

                let (price_ratio, fulfillment) = country
                    .market
                    .get(res_id)
                    .map(|m| (m.price / m.base_price.max(0.001), m.buy_fulfillment))
                    .unwrap_or((1.0, 1.0));

                let active_national_lines: Vec<&crate::economy::ProductionLineState> = ctx
                    .facilities
                    .iter()
                    .filter(|f| f.country_tag == country.tag)
                    .flat_map(|f| &f.lines)
                    .filter(|l| l.recipe_id == recipe_id && !l.employed_workers.is_empty())
                    .collect();

                if !active_national_lines.is_empty() {
                    let avg_utilization: f64 = active_national_lines
                        .iter()
                        .map(|l| l.utilization)
                        .sum::<f64>()
                        / (active_national_lines.len() as f64);
                    if avg_utilization < 0.75 && fulfillment > 0.60 && price_ratio < 1.30 {
                        continue;
                    }
                }

                let target_sector = recipe.sector;
                let build_days = recipe.build_time_days.max(30.0);
                let exchange_rate = country.economy.exchange_rate.max(0.001);

                let total_materials_cost: f64 = recipe
                    .construction_cost
                    .iter()
                    .map(|(mat_id, qty)| {
                        qty * country.market.get(*mat_id).map(|m| m.price).unwrap_or(50.0)
                    })
                    .sum();

                let daily_workforce_wage: f64 = recipe
                    .construction_workforce
                    .iter()
                    .map(|(pop_type_id, count)| {
                        let mult = ctx
                            .defs
                            .pop_types
                            .get(*pop_type_id)
                            .map(|t| t.wage_multiplier)
                            .unwrap_or(1.0);
                        (*count as f64) * (1.2 * exchange_rate) * mult
                    })
                    .sum();
                let total_expected_wages = daily_workforce_wage * build_days;
                let contractor_profit = (recipe.base_expansion_cost * 0.10 * exchange_rate
                    + total_materials_cost * 0.15)
                    .max(5_000.0 * exchange_rate);
                let contractor_fee = total_expected_wages + contractor_profit;
                let total_build_cost = total_materials_cost + contractor_fee;
                let required_capital = total_build_cost.max(10_000.0 * exchange_rate);

                let (price_ratio, fulfillment) = country
                    .market
                    .get(res_id)
                    .map(|m| (m.price / m.base_price.max(0.001), m.buy_fulfillment))
                    .unwrap_or((1.0, 1.0));

                let sector_companies_count = ctx
                    .companies
                    .iter()
                    .filter(|c| c.country_tag == country.tag && c.sector_kind == target_sector)
                    .count();

                let mut compatible_companies: Vec<(CompanyId, f64, bool)> = ctx
                    .companies
                    .iter()
                    .filter(|c| {
                        c.country_tag == country.tag
                            && is_sector_compatible(c.sector_kind, target_sector)
                    })
                    .map(|c| {
                        let is_building =
                            country.construction_queue.iter().any(|p| match p.payer {
                                BudgetSource::Company(payer_id) => payer_id == c.id,
                                _ => false,
                            });
                        (c.id, c.balance, is_building)
                    })
                    .collect();

                compatible_companies.sort_by(|a, b| {
                    a.2.cmp(&b.2)
                        .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                        .then_with(|| a.0.0.cmp(&b.0.0))
                });

                let max_sector_companies = 3 + country.regions.len() / 2;
                let startup_entry_threshold = 1.25 + (sector_companies_count as f64) * 0.08;
                let should_found_startup = sector_companies_count < max_sector_companies
                    && (sector_companies_count == 0
                        || compatible_companies.is_empty()
                        || ((price_ratio >= startup_entry_threshold || fulfillment <= 0.60)
                            && compatible_companies
                                .iter()
                                .all(|(_, bal, is_b)| *is_b || *bal < required_capital * 0.20)));

                let startup_equity = required_capital * 0.50 + 5_000.0 * exchange_rate;
                let total_national_savings: f64 = ctx
                    .regions
                    .iter()
                    .filter(|r| r.owner == Some(country.tag))
                    .flat_map(|r| &r.pops)
                    .map(|p| p.total_savings())
                    .sum();

                let investor_company_id =
                    if should_found_startup && total_national_savings >= startup_equity {
                        let take_ratio = (startup_equity / total_national_savings).min(1.0);
                        for reg in ctx
                            .regions
                            .iter_mut()
                            .filter(|r| r.owner == Some(country.tag))
                        {
                            for p in &mut reg.pops {
                                p.employed.savings *= 1.0 - take_ratio;
                                p.unemployed.savings *= 1.0 - take_ratio;
                            }
                        }

                        let queued_max = ctx
                            .commands
                            .iter()
                            .filter_map(|cmd| {
                                if let WorldCommand::SpawnCompany(comp) = cmd {
                                    Some(comp.id.0)
                                } else {
                                    None
                                }
                            })
                            .max()
                            .unwrap_or(0);
                        let existing_max = ctx.companies.iter().map(|c| c.id.0).max().unwrap_or(0);
                        let next_id = existing_max.max(queued_max) + 1;
                        let new_comp_id = CompanyId(next_id);
                        let comp_name =
                            generate_company_name(country.tag, target_sector, new_comp_id.0);

                        let new_company = CompanyState {
                            id: new_comp_id,
                            name: comp_name,
                            country_tag: country.tag,
                            ownership: Ownership::Private {
                                shareholders_group_id: 0,
                            },
                            sector_kind: target_sector,
                            balance: startup_equity,
                            reinvestment_ratio: 0.3,
                            monthly_revenue: 0.0,
                            monthly_expenses: 0.0,
                            last_month_profit: 0.0,
                        };

                        ctx.commands.push(WorldCommand::SpawnCompany(new_company));
                        new_comp_id
                    } else if let Some(&(best_id, _, _)) = compatible_companies.first() {
                        best_id
                    } else {
                        continue;
                    };

                let contractor_opt = ctx
                    .companies
                    .iter()
                    .filter(|c| {
                        c.country_tag == country.tag && c.sector_kind == SectorKind::Construction
                    })
                    .min_by_key(|c| {
                        country
                            .construction_queue
                            .iter()
                            .filter(|p| p.contractor == c.id)
                            .count()
                    })
                    .map(|c| c.id);

                let Some(contractor) = contractor_opt else {
                    continue;
                };

                let active_projects = country
                    .construction_queue
                    .iter()
                    .filter(|p| p.contractor == contractor)
                    .count();
                if active_projects >= 5 {
                    continue;
                }

                let max_country_projects = country.regions.len().clamp(2, 6);
                if country.construction_queue.len() >= max_country_projects {
                    continue;
                }

                let required_factory_workforce: u64 = recipe
                    .workforce_requirements
                    .iter()
                    .map(|(_, count)| (*count as u64) * 10)
                    .sum();
                let required_construction_workforce: u64 = recipe
                    .construction_workforce
                    .iter()
                    .map(|(_, count)| *count as u64)
                    .sum();
                let min_labor_needed = required_factory_workforce + required_construction_workforce;

                let target_region_opt = ctx
                    .regions
                    .iter()
                    .filter(|r| r.owner == Some(country.tag))
                    .filter(|r| {
                        let unemployed: u64 = r.pops.iter().map(|p| p.unemployed.count).sum();
                        if unemployed < min_labor_needed {
                            return false;
                        }
                        let has_university = r.structures.iter().any(|s| {
                            ctx.defs
                                .structures
                                .get(s.def_id)
                                .map(|d| d.science_output_per_level > 0.0)
                                .unwrap_or(false)
                        });
                        recipe.workforce_requirements.iter().all(|(pop_type, req)| {
                            let available = r
                                .pops
                                .iter()
                                .filter(|p| p.pop_type_id == *pop_type)
                                .map(|p| p.unemployed.count)
                                .sum::<u64>();
                            if available >= *req as u64 {
                                return true;
                            }
                            let is_specialized = ctx
                                .defs
                                .pop_types
                                .get(*pop_type)
                                .map(|p| {
                                    p.science_contribution > 0.0
                                        || p.wage_multiplier >= 2.0
                                        || !p.is_taxable
                                })
                                .unwrap_or(false);
                            is_specialized && has_university
                        })
                    })
                    .max_by_key(|r| {
                        let unemployed_count: u64 = r.pops.iter().map(|p| p.unemployed.count).sum();
                        unemployed_count
                    });

                let Some(target_region) = target_region_opt else {
                    continue;
                };
                let target_region_id = target_region.id;

                let daily_materials: Vec<(ResourceId, f64)> = recipe
                    .construction_cost
                    .iter()
                    .map(|(mat, qty)| (*mat, qty / build_days))
                    .collect();

                let daily_contractor_fee = contractor_fee / build_days;

                let mobilization_advance =
                    (daily_contractor_fee * 5.0).min(required_capital * 0.20);
                if mobilization_advance > 0.0 {
                    if let Some(investor_idx) = ctx
                        .companies
                        .iter()
                        .position(|c| c.id == investor_company_id)
                    {
                        if ctx.companies[investor_idx].balance < mobilization_advance {
                            let _ = BankLoan::request_corporate_loan(
                                investor_idx,
                                required_capital.max(mobilization_advance * 3.0),
                                country.tag,
                                &mut ctx.companies,
                                &mut ctx.loans,
                            );
                        }
                        let actual_advance =
                            mobilization_advance.min(ctx.companies[investor_idx].balance);
                        if actual_advance > 0.0 {
                            ctx.companies[investor_idx].balance -= actual_advance;
                            if let Some(cont) =
                                ctx.companies.iter_mut().find(|c| c.id == contractor)
                            {
                                cont.balance += actual_advance;
                            }
                        }
                    }
                }

                country.construction_queue.push(ConstructionProject {
                    target: ConstructionTarget::NewFacility {
                        company_id: investor_company_id,
                        recipe_id,
                        region_id: target_region_id,
                        sector_kind: target_sector,
                    },
                    daily_contractor_fee,
                    contractor,
                    payer: BudgetSource::Company(investor_company_id),
                    progress: 0.0,
                    total_days: build_days,
                    daily_materials,
                    current_workforce: vec![],
                    required_workers: recipe.construction_workforce.clone(),
                    last_effective_speed: 1.0,
                    consecutive_stalled_days: 0,
                });

                started_projects += 1;
            }
        }
    }
}

fn is_sector_compatible(company_sector: SectorKind, target_sector: SectorKind) -> bool {
    if company_sector == target_sector {
        return true;
    }
    match company_sector {
        SectorKind::Extraction => {
            matches!(
                target_sector,
                SectorKind::Petrochemistry | SectorKind::Energy
            )
        }
        SectorKind::HeavyIndustry => {
            matches!(
                target_sector,
                SectorKind::Automotive | SectorKind::Extraction
            )
        }
        SectorKind::Petrochemistry => matches!(target_sector, SectorKind::Consumer),
        SectorKind::Energy => matches!(target_sector, SectorKind::Extraction),
        SectorKind::InformationTechnology => matches!(target_sector, SectorKind::HighTech),
        SectorKind::HighTech => matches!(target_sector, SectorKind::InformationTechnology),
        SectorKind::Construction => matches!(target_sector, SectorKind::RealEstate),
        SectorKind::RealEstate => matches!(target_sector, SectorKind::Construction),
        _ => false,
    }
}

fn generate_company_name(country_tag: crate::CountryTag, sector: SectorKind, id: u32) -> String {
    let prefix = match country_tag.as_str() {
        "USA" => match sector {
            SectorKind::HighTech => "Apex Technologies",
            SectorKind::Automotive => "American Dynamics",
            SectorKind::HeavyIndustry => "United Industrial",
            SectorKind::Consumer => "General Consumer Goods",
            SectorKind::Petrochemistry => "Pacific Chemical",
            SectorKind::Agro => "Midland Foods",
            SectorKind::InformationTechnology => "Silicon Software",
            SectorKind::RealEstate => "Metro Real Estate",
            _ => "American Enterprises",
        },
        "RUS" => match sector {
            SectorKind::HighTech => "Sever HighTech",
            SectorKind::Automotive => "Soyuz Motors",
            SectorKind::HeavyIndustry => "Ural Metallurgy",
            SectorKind::Consumer => "Progress Consumer Combine",
            SectorKind::Petrochemistry => "Volga Petrochem",
            SectorKind::Agro => "Zarya Agro Group",
            SectorKind::InformationTechnology => "Sever Systems",
            SectorKind::RealEstate => "GlavStroy Capital",
            _ => "Rossiya Industrial",
        },
        "CHN" => match sector {
            SectorKind::HighTech => "Dragon Microelectronics",
            SectorKind::Automotive => "Great Wall Motors",
            SectorKind::HeavyIndustry => "Huaxia Heavy Industries",
            SectorKind::Consumer => "Cathay Consumer Products",
            SectorKind::Petrochemistry => "Yangtze Petrochemical",
            SectorKind::Agro => "Golden Harvest Agriculture",
            SectorKind::InformationTechnology => "Huaxia Digital",
            SectorKind::RealEstate => "Cathay Real Estate",
            _ => "Sino Enterprises",
        },
        "DEU" => match sector {
            SectorKind::HighTech => "Nordic Silicon Systems",
            SectorKind::Automotive => "Bavaria Motor Works",
            SectorKind::HeavyIndustry => "Rheinland Heavy Industries",
            SectorKind::Consumer => "Hanseatic Consumer Products",
            SectorKind::Petrochemistry => "Weser Chemical Combine",
            SectorKind::Agro => "Alpen Agrar Union",
            SectorKind::InformationTechnology => "Rhine Software Systems",
            SectorKind::RealEstate => "Bavaria Immobilien",
            _ => "Germania Holdings",
        },
        _ => "United Corporation",
    };
    format!("{prefix} #{id}")
}
