use crate::system::{System, SystemContext};
use crate::world::Pop;
use crate::world::time::TimeTick;

pub struct EducationSystem;

impl System for EducationSystem {
    fn id(&self) -> &'static str {
        "core:education_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        matches!(tick.month(), Some(6) | Some(9))
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let month = ctx.tick.month();
        let structures_defs = &ctx.defs.structures;

        for region in ctx.regions.iter_mut() {
            if month == Some(6) {
                for structure in &region.structures {
                    if structure.efficiency <= 0.0 {
                        continue;
                    }
                    let Some(def) = structures_defs.get(structure.def_id) else {
                        continue;
                    };
                    let Some(edu_def) = &def.education else {
                        continue;
                    };

                    let (to_graduate, capital_to_move) = {
                        let Some(student_pop) = region
                            .pops
                            .iter_mut()
                            .find(|p| p.pop_type_id == edu_def.intermediate_pop)
                        else {
                            continue;
                        };

                        let total_students = student_pop.unemployed.count;
                        if total_students == 0 {
                            continue;
                        }

                        let potential =
                            ((total_students as f64) * edu_def.graduation_rate).round() as u64;
                        let to_graduate = potential.min(total_students);
                        if to_graduate == 0 {
                            continue;
                        }

                        let ratio = (to_graduate as f64) / (total_students as f64);
                        let capital = student_pop.unemployed.savings * ratio;

                        student_pop.unemployed.count -= to_graduate;
                        student_pop.unemployed.savings -= capital;

                        (to_graduate, capital)
                    };

                    for &(target_pop_id, share) in &edu_def.graduates {
                        let count = ((to_graduate as f64) * share).round() as u64;
                        let capital = capital_to_move * share;
                        if count == 0 {
                            continue;
                        }

                        let target_pop = match region
                            .pops
                            .iter_mut()
                            .find(|p| p.pop_type_id == target_pop_id)
                        {
                            Some(pop) => pop,
                            None => {
                                region.pops.push(Pop::new(target_pop_id, 0));
                                region.pops.last_mut().unwrap()
                            }
                        };

                        target_pop.unemployed.count += count;
                        target_pop.unemployed.savings += capital;
                    }
                }
            }

            if month == Some(9) {
                let mut total_capacity: u64 = 0;
                let mut target_edu_def = None;

                for structure in &region.structures {
                    if structure.efficiency <= 0.0 {
                        continue;
                    }
                    let Some(def) = structures_defs.get(structure.def_id) else {
                        continue;
                    };
                    let Some(edu_def) = &def.education else {
                        continue;
                    };

                    let cap = (def.capacity_per_level as f64
                        * structure.level as f64
                        * structure.efficiency)
                        .round() as u64;
                    total_capacity += cap;
                    target_edu_def = Some(edu_def);
                }

                let Some(edu_def) = target_edu_def else {
                    continue;
                };
                if total_capacity == 0 {
                    continue;
                }

                let current_students = region
                    .pops
                    .iter()
                    .find(|p| p.pop_type_id == edu_def.intermediate_pop)
                    .map_or(0, |p| p.unemployed.count);

                if current_students < total_capacity {
                    let seats_left = total_capacity - current_students;

                    let (enrolled, capital_to_move) = {
                        let Some(candidate_pop) = region
                            .pops
                            .iter_mut()
                            .find(|p| p.pop_type_id == edu_def.candidate_pop)
                        else {
                            continue;
                        };

                        let available = candidate_pop.unemployed.count;
                        if available == 0 {
                            (0, 0.0)
                        } else {
                            let to_enroll = seats_left.min(available);
                            let ratio = (to_enroll as f64) / (available as f64);
                            let capital = (candidate_pop.unemployed.savings * ratio)
                                .min(candidate_pop.unemployed.savings);

                            candidate_pop.unemployed.count -= to_enroll;
                            candidate_pop.unemployed.savings -= capital;

                            (to_enroll, capital)
                        }
                    };

                    if enrolled > 0 {
                        let student_pop = match region
                            .pops
                            .iter_mut()
                            .find(|p| p.pop_type_id == edu_def.intermediate_pop)
                        {
                            Some(pop) => pop,
                            None => {
                                region.pops.push(Pop::new(edu_def.intermediate_pop, 0));
                                region.pops.last_mut().unwrap()
                            }
                        };
                        student_pop.unemployed.count += enrolled;
                        student_pop.unemployed.savings += capital_to_move;
                    }
                }
            }
        }
    }
}
