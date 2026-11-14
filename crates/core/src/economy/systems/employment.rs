use crate::economy::{RealEstateKind, SectorKind};
use crate::system::{System, SystemContext};
use crate::world::map::RegionId;
use crate::world::time::TimeTick;
use crate::world::{CountryTag, PopTypeId};

#[derive(Debug, Clone, Copy)]
enum EmployerRef {
    FacilityLine {
        fac_idx: usize,
        line_idx: usize,
    },
    Structure {
        region_idx: usize,
        struct_idx: usize,
    },
    Construction {
        country_idx: usize,
        project_idx: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct Vacancy {
    target: EmployerRef,
    pop_type_id: PopTypeId,
    count: u32,
}

#[derive(Default)]
pub struct EmploymentSystem {
    layoffs: Vec<(RegionId, PopTypeId, u32)>,
    region_vacancies: Vec<Vec<Vacancy>>,
    hires: Vec<(EmployerRef, PopTypeId, u32)>,
    country_tags: Vec<CountryTag>,
    company_indices: Vec<usize>,
    region_id_to_idx: Vec<usize>,
    office_capacity: Vec<u32>,
    office_employed: Vec<u32>,
}

impl System for EmploymentSystem {
    fn id(&self) -> &'static str {
        "core:employment_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let recipes = &ctx.defs.recipes;

        self.layoffs.clear();
        self.hires.clear();

        self.country_tags.clear();
        for c in ctx.countries.iter() {
            self.country_tags.push(c.tag);
        }

        let max_comp_id = ctx
            .companies
            .iter()
            .map(|c| c.id.0 as usize)
            .max()
            .unwrap_or(0);
        self.company_indices.clear();
        self.company_indices.resize(max_comp_id + 1, usize::MAX);
        for (idx, comp) in ctx.companies.iter().enumerate() {
            self.company_indices[comp.id.0 as usize] = idx;
        }

        let num_regions = ctx.regions.len();
        let max_region_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);
        self.region_id_to_idx.clear();
        self.region_id_to_idx.resize(max_region_id + 1, usize::MAX);
        for (idx, r) in ctx.regions.iter().enumerate() {
            self.region_id_to_idx[r.id.0 as usize] = idx;
        }

        if self.region_vacancies.len() != num_regions {
            self.region_vacancies.resize_with(num_regions, Vec::new);
        }
        for sub_vec in &mut self.region_vacancies {
            sub_vec.clear();
        }

        self.office_capacity.resize(num_regions, 0);
        self.office_capacity.fill(0);
        for re in ctx.real_estate.iter() {
            if re.kind == RealEstateKind::CommercialOffice {
                let r_id = re.region_id.0 as usize;
                if r_id < self.region_id_to_idx.len() {
                    let reg_idx = self.region_id_to_idx[r_id];
                    if reg_idx != usize::MAX && reg_idx < num_regions {
                        self.office_capacity[reg_idx] += re.total_units;
                    }
                }
            }
        }

        self.office_employed.resize(num_regions, 0);
        self.office_employed.fill(0);
        for f in ctx.facilities.iter() {
            if f.sector_kind == SectorKind::InformationTechnology
                || f.sector_kind == SectorKind::Financial
            {
                let r_id = f.region_id.0 as usize;
                if r_id < self.region_id_to_idx.len() {
                    let reg_idx = self.region_id_to_idx[r_id];
                    if reg_idx != usize::MAX && reg_idx < num_regions {
                        let mut f_workers = 0u32;
                        for l in &f.lines {
                            for &(_, c) in &l.employed_workers {
                                f_workers += c;
                            }
                        }
                        self.office_employed[reg_idx] += f_workers;
                    }
                }
            }
        }

        for (fac_idx, facility) in ctx.facilities.iter_mut().enumerate() {
            let ex_rate = self
                .country_tags
                .iter()
                .position(|t| *t == facility.country_tag)
                .map(|idx| ctx.countries[idx].economy.exchange_rate.max(0.001))
                .unwrap_or(1.0);

            let comp_id = facility.company_id.0 as usize;
            let comp_balance = if comp_id < self.company_indices.len() {
                let mapped = self.company_indices[comp_id];
                if mapped != usize::MAX && mapped < ctx.companies.len() {
                    ctx.companies[mapped].balance
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let total_funds = comp_balance + facility.local_reserve;
            let fac_reg_idx = self.region_id_to_idx[facility.region_id.0 as usize];

            let is_office_sector = facility.sector_kind == SectorKind::InformationTechnology
                || facility.sector_kind == SectorKind::Financial;
            let has_office_capacity = if is_office_sector {
                if fac_reg_idx != usize::MAX && fac_reg_idx < num_regions {
                    self.office_capacity[fac_reg_idx] > self.office_employed[fac_reg_idx]
                } else {
                    true
                }
            } else {
                true
            };

            for (line_idx, line) in facility.lines.iter_mut().enumerate() {
                if let Some(recipe) = recipes.get(line.recipe_id) {
                    let total_nominal_labor: u32 = recipe
                        .workforce_requirements
                        .iter()
                        .map(|(_, count)| *count as u32)
                        .sum();
                    let daily_labor_cost = (line.installed_capacity as f64)
                        * (total_nominal_labor as f64)
                        * line.target_wage;

                    let is_bankrupt =
                        comp_balance <= 0.0 && facility.local_reserve < daily_labor_cost * 1.5;
                    let is_underfunded = total_funds < daily_labor_cost * 2.0;

                    let should_layoff = is_bankrupt
                        || (is_underfunded
                            && (line.utilization < 0.2 || line.consecutive_idle_days > 2));

                    let should_hire = !is_bankrupt
                        && !is_underfunded
                        && has_office_capacity
                        && (line.material_utilization > 0.1
                            || line.consecutive_idle_days == 0
                            || line.employed_workers.is_empty())
                        && total_funds >= (5_000.0 * ex_rate).max(daily_labor_cost * 2.0);

                    for &(pop_type_id, req_per_cap) in &recipe.workforce_requirements {
                        let target_needed = if should_layoff {
                            0
                        } else if should_hire {
                            (req_per_cap as u32) * (line.installed_capacity as u32)
                        } else {
                            line.get_employed(pop_type_id)
                        };

                        let currently_employed = line.get_employed(pop_type_id);

                        if target_needed < currently_employed {
                            let diff = currently_employed - target_needed;
                            let max_layoff_per_day = if is_bankrupt {
                                ((currently_employed as f64) * 0.35).ceil() as u32
                            } else {
                                ((currently_employed as f64) * 0.20).ceil() as u32
                            }
                            .max(1);
                            let to_fire = diff.min(max_layoff_per_day);

                            let new_employed = currently_employed - to_fire;
                            line.set_employed(pop_type_id, new_employed);
                            self.layoffs
                                .push((facility.region_id, pop_type_id, to_fire));
                        } else if target_needed > currently_employed && fac_reg_idx != usize::MAX {
                            self.region_vacancies[fac_reg_idx].push(Vacancy {
                                target: EmployerRef::FacilityLine { fac_idx, line_idx },
                                pop_type_id,
                                count: target_needed - currently_employed,
                            });
                        }
                    }
                }
            }
        }

        for (region_idx, region) in ctx.regions.iter_mut().enumerate() {
            let region_id = region.id;
            for (struct_idx, structure) in region.structures.iter_mut().enumerate() {
                let Some(def) = ctx.defs.structures.get(structure.def_id) else {
                    continue;
                };

                let is_funded = structure.allocated_budget > 0.0;
                let level_mult = structure.level as f64;

                for &(pop_type_id, req_per_level) in &def.workforce_requirements {
                    let target_needed = if is_funded {
                        ((req_per_level as f64) * level_mult).ceil() as u32
                    } else {
                        0
                    };

                    let currently_employed = structure
                        .current_workforce
                        .iter()
                        .find(|&&(id, _)| id == pop_type_id)
                        .map(|&(_, c)| c)
                        .unwrap_or(0);

                    if target_needed < currently_employed {
                        let to_fire = currently_employed - target_needed;
                        if let Some(entry) = structure
                            .current_workforce
                            .iter_mut()
                            .find(|(id, _)| *id == pop_type_id)
                        {
                            entry.1 = target_needed;
                        }
                        self.layoffs.push((region_id, pop_type_id, to_fire));
                    } else if target_needed > currently_employed {
                        self.region_vacancies[region_idx].push(Vacancy {
                            target: EmployerRef::Structure {
                                region_idx,
                                struct_idx,
                            },
                            pop_type_id,
                            count: target_needed - currently_employed,
                        });
                    }
                }
            }
        }

        for (country_idx, country) in ctx.countries.iter().enumerate() {
            for (project_idx, project) in country.construction_queue.iter().enumerate() {
                let project_region = match project.target {
                    crate::world::ConstructionTarget::NewFacility { region_id, .. } => region_id,
                    crate::world::ConstructionTarget::UpgradeStructure { region_id, .. } => {
                        region_id
                    }
                    crate::world::ConstructionTarget::NewRealEstate { region_id, .. } => region_id,
                };

                let proj_reg_idx = self.region_id_to_idx[project_region.0 as usize];

                for &(pop_type_id, req_count) in &project.required_workers {
                    let currently_employed = project
                        .current_workforce
                        .iter()
                        .find(|&&(id, _)| id == pop_type_id)
                        .map(|&(_, c)| c)
                        .unwrap_or(0);

                    if req_count > currently_employed {
                        let diff = req_count - currently_employed;
                        let mut vacancy_posted = false;

                        if proj_reg_idx != usize::MAX && proj_reg_idx < ctx.regions.len() {
                            let available = ctx.regions[proj_reg_idx]
                                .pops
                                .iter()
                                .find(|p| p.pop_type_id == pop_type_id)
                                .map(|p| p.free_workforce())
                                .unwrap_or(0);
                            if available > 0 {
                                self.region_vacancies[proj_reg_idx].push(Vacancy {
                                    target: EmployerRef::Construction {
                                        country_idx,
                                        project_idx,
                                    },
                                    pop_type_id,
                                    count: diff,
                                });
                                vacancy_posted = true;
                            }
                        }

                        if !vacancy_posted {
                            for (r_idx, r) in ctx.regions.iter().enumerate() {
                                if r.owner == Some(country.tag) && r_idx != proj_reg_idx {
                                    let available = r
                                        .pops
                                        .iter()
                                        .find(|p| p.pop_type_id == pop_type_id)
                                        .map(|p| p.free_workforce())
                                        .unwrap_or(0);
                                    if available > 0 {
                                        self.region_vacancies[r_idx].push(Vacancy {
                                            target: EmployerRef::Construction {
                                                country_idx,
                                                project_idx,
                                            },
                                            pop_type_id,
                                            count: diff,
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for &(region_id, pop_type_id, count) in &self.layoffs {
            let r_idx = self.region_id_to_idx[region_id.0 as usize];
            if r_idx != usize::MAX {
                if let Some(pop) = ctx.regions[r_idx]
                    .pops
                    .iter_mut()
                    .find(|p| p.pop_type_id == pop_type_id)
                {
                    pop.fire(count as u64);
                }
            }
        }

        for (reg_idx, region) in ctx.regions.iter_mut().enumerate() {
            let vacancies = &self.region_vacancies[reg_idx];

            for vacancy in vacancies {
                if let Some(pop) = region
                    .pops
                    .iter_mut()
                    .find(|p| p.pop_type_id == vacancy.pop_type_id)
                {
                    let available = pop.free_workforce();
                    if available == 0 {
                        continue;
                    }

                    let to_hire = (available as u32).min(vacancy.count);
                    if to_hire > 0 {
                        pop.hire(to_hire as u64);
                        self.hires
                            .push((vacancy.target, vacancy.pop_type_id, to_hire));
                    }
                }
            }
        }

        for &(target, pop_type_id, hired_count) in &self.hires {
            match target {
                EmployerRef::FacilityLine { fac_idx, line_idx } => {
                    if let Some(fac) = ctx.facilities.get_mut(fac_idx) {
                        if let Some(line) = fac.lines.get_mut(line_idx) {
                            let curr = line.get_employed(pop_type_id);
                            line.set_employed(pop_type_id, curr + hired_count);
                        }
                    }
                }
                EmployerRef::Structure {
                    region_idx,
                    struct_idx,
                } => {
                    if let Some(reg) = ctx.regions.get_mut(region_idx) {
                        if let Some(st) = reg.structures.get_mut(struct_idx) {
                            if let Some(entry) = st
                                .current_workforce
                                .iter_mut()
                                .find(|(id, _)| *id == pop_type_id)
                            {
                                entry.1 += hired_count;
                            } else {
                                st.current_workforce.push((pop_type_id, hired_count));
                            }
                        }
                    }
                }
                EmployerRef::Construction {
                    country_idx,
                    project_idx,
                } => {
                    if let Some(country) = ctx.countries.get_mut(country_idx) {
                        if let Some(project) = country.construction_queue.get_mut(project_idx) {
                            if let Some(entry) = project
                                .current_workforce
                                .iter_mut()
                                .find(|(id, _)| *id == pop_type_id)
                            {
                                entry.1 += hired_count;
                            } else {
                                project.current_workforce.push((pop_type_id, hired_count));
                            }
                        }
                    }
                }
            }
        }
    }
}
