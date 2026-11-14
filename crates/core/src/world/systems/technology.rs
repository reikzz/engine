use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;

pub struct TechnologySystem;

impl System for TechnologySystem {
    fn id(&self) -> &'static str {
        "core:technology_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        for country in ctx.countries.iter_mut() {
            let mut monthly_science = 10.0;

            for region in ctx.regions.iter().filter(|r| r.owner == Some(country.tag)) {
                for structure in &region.structures {
                    if let Some(def) = ctx.defs.structures.get(structure.def_id) {
                        if def.science_output_per_level > 0.0 {
                            monthly_science += (structure.level as f64)
                                * structure.efficiency.max(0.0)
                                * def.science_output_per_level;
                        }
                    }
                }

                for pop in &region.pops {
                    let employed = pop.employed.count as f64;
                    if let Some(pop_def) = ctx.defs.pop_types.get(pop.pop_type_id) {
                        if pop_def.science_contribution > 0.0 {
                            monthly_science += employed * pop_def.science_contribution;
                        }
                    }
                }
            }

            country.last_period_science_output = monthly_science;

            if let Some(active_id) = country.active_research {
                if country.has_technology(active_id) {
                    country.active_research = None;
                    country.research_progress = 0.0;
                } else if let Some(tech_def) = ctx.defs.technologies.get(active_id) {
                    country.research_progress += monthly_science;
                    if country.research_progress >= tech_def.cost {
                        country.unlock_technology(active_id);
                        country.research_progress = 0.0;
                        country.active_research = None;
                    }
                }
            }

            if country.active_research.is_none() {
                let mut best_candidate = None;
                let mut min_cost = f64::MAX;

                for (tech_id, tech_def) in ctx.defs.technologies.iter() {
                    if country.has_technology(tech_id) {
                        continue;
                    }

                    let prereqs_met = tech_def
                        .prerequisites
                        .iter()
                        .all(|req| country.has_technology(*req));

                    if prereqs_met && tech_def.cost < min_cost {
                        min_cost = tech_def.cost;
                        best_candidate = Some(tech_id);
                    }
                }

                if let Some(candidate) = best_candidate {
                    country.active_research = Some(candidate);
                }
            }
        }
    }
}
