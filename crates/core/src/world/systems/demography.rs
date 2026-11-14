use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::map::RegionModifier;
use crate::world::pops::{NeedCategory, PopTypeId};
use crate::world::time::TimeTick;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct MigrationMove {
    to_region_idx: usize,
    pop_type_id: PopTypeId,
    count: u64,
    capital: f64,
}

#[derive(Default)]
pub struct DemographySystem {
    tag_to_country_idx: BTreeMap<CountryTag, usize>,
    needs_fulfillment: Vec<f64>,
    country_region_indices: Vec<Vec<usize>>,
    region_scores: Vec<(usize, f64)>,
    planned_migrations: Vec<MigrationMove>,
}

impl System for DemographySystem {
    fn id(&self) -> &'static str {
        "core:demography_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let SystemContext {
            defs,
            countries,
            regions,
            ..
        } = ctx;

        let num_countries = countries.len();

        self.tag_to_country_idx.clear();
        for (idx, country) in countries.iter().enumerate() {
            self.tag_to_country_idx.insert(country.tag, idx);
        }

        self.needs_fulfillment.clear();
        self.needs_fulfillment.resize(num_countries, 1.0);

        for (idx, country) in countries.iter().enumerate() {
            let mut total_weight = 0.0;
            let mut total_fulfillment = 0.0;

            for (_, pop_type) in defs.pop_types.iter() {
                for need in &pop_type.needs {
                    if need.category != NeedCategory::LifeSupport {
                        continue;
                    }

                    if let Some(rm) = country.market.get(need.resource_id) {
                        let weight = if need.weight > 0.0 { need.weight } else { 1.0 };
                        if rm.last_demanded > 0.0 {
                            total_weight += weight * rm.last_demanded;
                            total_fulfillment += weight * rm.last_demanded * rm.buy_fulfillment;
                        } else if rm.last_supplied > 0.0 {
                            total_weight += weight;
                            total_fulfillment += weight * rm.buy_fulfillment.clamp(0.0f64, 1.0f64);
                        }
                    }
                }
            }

            let avg_fulfillment: f64 = if total_weight > 0.0 {
                (total_fulfillment / total_weight).clamp(0.0f64, 1.0f64)
            } else {
                1.0
            };
            self.needs_fulfillment[idx] = avg_fulfillment;
        }

        for region in regions.iter_mut() {
            let country_fulfillment = region
                .owner
                .and_then(|tag| self.tag_to_country_idx.get(&tag))
                .map(|&idx| self.needs_fulfillment[idx])
                .unwrap_or(1.0);

            let base_birth_rate = 0.0010;
            let hospital_growth = region.modifiers.get(RegionModifier::PopulationGrowth);
            let effective_birth_rate = (base_birth_rate + hospital_growth).max(0.0001)
                * country_fulfillment.clamp(0.8, 1.2);

            let base_death_rate = 0.0007;
            let hospital_death_reduction = region.modifiers.get(RegionModifier::MortalityRate);
            let famine_penalty = if country_fulfillment < 0.75 {
                (0.75 - country_fulfillment) * 0.005
            } else {
                0.0
            };
            let effective_death_rate =
                (base_death_rate + hospital_death_reduction + famine_penalty).max(0.0001);

            for pop in &mut region.pops {
                let total = pop.count();
                if total == 0 {
                    continue;
                }

                let births = ((total as f64) * effective_birth_rate).round() as u64;
                pop.unemployed.count += births;

                let deaths = ((total as f64) * effective_death_rate).round() as u64;
                if deaths > 0 {
                    if pop.unemployed.count >= deaths {
                        pop.unemployed.count -= deaths;
                    } else {
                        let rem = deaths - pop.unemployed.count;
                        pop.unemployed.count = 0;
                        pop.employed.count = pop.employed.count.saturating_sub(rem);
                    }
                }
            }
        }

        if self.country_region_indices.len() < num_countries {
            self.country_region_indices
                .resize(num_countries, Vec::new());
        }
        for sub_vec in &mut self.country_region_indices[..num_countries] {
            sub_vec.clear();
        }

        for (r_idx, region) in regions.iter().enumerate() {
            if let Some(owner) = region.owner {
                if let Some(&c_idx) = self.tag_to_country_idx.get(&owner) {
                    self.country_region_indices[c_idx].push(r_idx);
                }
            }
        }

        self.planned_migrations.clear();

        for c_idx in 0..num_countries {
            let country_reg_indices = &self.country_region_indices[c_idx];
            if country_reg_indices.len() < 2 {
                continue;
            }

            self.region_scores.clear();

            for &r_idx in country_reg_indices {
                let r = &regions[r_idx];
                let total_pop: u64 = r.pops.iter().map(|p| p.count()).sum();
                let total_employed: u64 = r.pops.iter().map(|p| p.employed.count).sum();

                let employment_rate = if total_pop > 0 {
                    total_employed as f64 / total_pop as f64
                } else {
                    0.5
                };

                let avg_wage = if total_employed > 0 {
                    r.pops
                        .iter()
                        .map(|p| p.average_wage * p.employed.count as f64)
                        .sum::<f64>()
                        / total_employed as f64
                } else {
                    1.0
                };

                let avg_satisfaction = if total_pop > 0 {
                    r.pops
                        .iter()
                        .map(|p| p.satisfaction as f64 * p.count() as f64)
                        .sum::<f64>()
                        / total_pop as f64
                } else {
                    1.0
                };

                let infrastructure = r.infrastructure_level;
                let stability = r.modifiers.get(RegionModifier::SocialStability);

                let score = employment_rate * 2.5
                    + (avg_wage / 2.0).clamp(0.5, 2.5)
                    + avg_satisfaction * 1.5
                    + infrastructure * 0.5
                    + stability * 0.5;

                self.region_scores.push((r_idx, score));
            }

            self.region_scores
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let (best_idx, best_score) = self.region_scores[0];
            let (worst_idx, worst_score) = self.region_scores[self.region_scores.len() - 1];

            let score_gap = best_score - worst_score;
            if score_gap > 0.25 {
                let migration_rate = (score_gap * 0.01).clamp(0.002, 0.008);

                let source_region = &mut regions[worst_idx];
                for pop in &mut source_region.pops {
                    let unemployed = pop.unemployed.count;
                    if unemployed < 100 {
                        continue;
                    }

                    let count_to_move = ((unemployed as f64) * migration_rate).round() as u64;
                    if count_to_move == 0 {
                        continue;
                    }

                    let capital_share = (count_to_move as f64) / (unemployed as f64);
                    let capital_to_move = pop.unemployed.savings * capital_share;

                    pop.unemployed.count -= count_to_move;
                    pop.unemployed.savings -= capital_to_move;

                    self.planned_migrations.push(MigrationMove {
                        to_region_idx: best_idx,
                        pop_type_id: pop.pop_type_id,
                        count: count_to_move,
                        capital: capital_to_move,
                    });
                }
            }
        }

        for m in &self.planned_migrations {
            let target_region = &mut regions[m.to_region_idx];
            if let Some(target_pop) = target_region
                .pops
                .iter_mut()
                .find(|p| p.pop_type_id == m.pop_type_id)
            {
                target_pop.unemployed.count += m.count;
                target_pop.unemployed.savings += m.capital;
            }
        }
    }
}
