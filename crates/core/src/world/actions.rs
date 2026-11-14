use crate::CountryTag;
use crate::economy::finance::BudgetSource;
use crate::economy::{
    CompanyId, FacilityId, Ownership, Priority, RealEstateKind, RecipeId, ResourceId, SectorKind,
};
use crate::world::country::{BailoutPolicy, WelfarePolicy};
use crate::world::map::RegionId;
use crate::world::structure::StructureDefId;
use crate::world::{ConstructionProject, ConstructionTarget, World};
use std::sync::Arc;

impl World {
    pub fn set_tax_rates(
        &mut self,
        country_tag: CountryTag,
        corporate_tax: f64,
        income_tax: f64,
        tariff_rate: f64,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.economy.corporate_tax = corporate_tax.clamp(0.0, 0.70);
        country.economy.income_tax = income_tax.clamp(0.0, 0.70);
        country.economy.tariff_rate = tariff_rate.clamp(0.0, 0.50);

        Ok(())
    }

    pub fn set_welfare_policy(
        &mut self,
        country_tag: CountryTag,
        policy: WelfarePolicy,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.economy.welfare = WelfarePolicy {
            max_tax_share: policy.max_tax_share.clamp(0.0, 1.0),
            min_daily_benefit: policy.min_daily_benefit.max(0.0),
            max_daily_benefit: policy
                .max_daily_benefit
                .max(policy.min_daily_benefit.max(0.0)),
        };

        Ok(())
    }

    pub fn set_bailout_policy(
        &mut self,
        country_tag: CountryTag,
        policy: BailoutPolicy,
        max_subsidy_ratio: f64,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.economy.bailout_policy = policy;
        country.economy.max_subsidy_budget_ratio = max_subsidy_ratio.clamp(0.0, 0.50);

        Ok(())
    }

    pub fn take_national_loan(
        &mut self,
        country_tag: CountryTag,
        amount: f64,
    ) -> Result<f64, String> {
        if amount <= 0.0 {
            return Err("Loan amount must be positive".to_string());
        }

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.economy.treasury += amount;
        country.economy.national_debt += amount;

        Ok(country.economy.treasury)
    }

    pub fn repay_national_debt(
        &mut self,
        country_tag: CountryTag,
        amount: f64,
    ) -> Result<f64, String> {
        if amount <= 0.0 {
            return Err("Repayment amount must be positive".to_string());
        }

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.economy.national_debt <= 0.0 {
            return Err("National debt is already zero".to_string());
        }

        let actual_repay = amount
            .min(country.economy.treasury)
            .min(country.economy.national_debt);

        if actual_repay <= 0.0 {
            return Err("Insufficient funds in treasury to repay debt".to_string());
        }

        country.economy.treasury -= actual_repay;
        country.economy.national_debt -= actual_repay;

        Ok(actual_repay)
    }

    pub fn issue_emergency_welfare(
        &mut self,
        country_tag: CountryTag,
        total_budget: f64,
    ) -> Result<f64, String> {
        if total_budget <= 0.0 {
            return Err("Budget must be positive".to_string());
        }

        let (actual_spend, unemployed_count) = {
            let country = self
                .country(country_tag)
                .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

            if country.economy.treasury < total_budget {
                return Err("Insufficient treasury funds".to_string());
            }

            let mut count = 0u64;
            for r in self.regions.iter().filter(|r| r.owner == Some(country_tag)) {
                for p in &r.pops {
                    count += p.unemployed.count;
                }
            }

            (total_budget.min(country.economy.treasury), count)
        };

        if unemployed_count == 0 {
            return Err("No unemployed population found in country".to_string());
        }

        let per_capita = actual_spend / (unemployed_count as f64);

        for r in self
            .regions
            .iter_mut()
            .filter(|r| r.owner == Some(country_tag))
        {
            for p in &mut r.pops {
                if p.unemployed.count > 0 {
                    p.unemployed.savings += per_capita * (p.unemployed.count as f64);
                    p.satisfaction = (p.satisfaction + 0.10).min(1.0);
                    p.radicalism = (p.radicalism - 0.05).max(0.0);
                }
            }
        }

        if let Some(country) = self.country_mut(country_tag) {
            country.economy.treasury -= actual_spend;
        }

        Ok(actual_spend)
    }

    pub fn calculate_facility_construction_cost(
        &self,
        country_tag: CountryTag,
        recipe_id: RecipeId,
    ) -> Result<crate::world::FacilityConstructionEstimate, String> {
        let recipe = self
            .defs
            .recipes
            .get(recipe_id)
            .ok_or_else(|| format!("Recipe {:?} not found", recipe_id))?;

        let country = self
            .country(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        let is_tech_unlocked = match recipe.required_tech {
            Some(req_tech) => country.has_technology(req_tech),
            None => true,
        };

        let exchange_rate = country.economy.exchange_rate.max(0.001);
        let build_days = recipe.build_time_days.max(30.0);

        let mut materials = Vec::with_capacity(recipe.construction_cost.len());
        let mut total_materials_cost = 0.0;

        for &(mat_id, qty) in &recipe.construction_cost {
            let unit_price = country.market.get(mat_id).map(|m| m.price).unwrap_or(50.0);
            let cost = qty * unit_price;
            total_materials_cost += cost;
            materials.push(crate::world::ConstructionMaterialEstimate {
                resource_id: mat_id,
                total_quantity: qty,
                daily_quantity: qty / build_days,
                local_unit_price: unit_price,
                total_cost: cost,
            });
        }

        let daily_workforce_wage: f64 = recipe
            .construction_workforce
            .iter()
            .map(|(pop_type_id, count)| {
                let mult = self
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
        let daily_contractor_fee = contractor_fee / build_days;

        let mobilization_advance = (daily_contractor_fee * 5.0).max(1_000.0 * exchange_rate);
        let total_estimated_cost = total_materials_cost + contractor_fee;
        let can_afford_advance = country.economy.treasury >= mobilization_advance;

        Ok(crate::world::FacilityConstructionEstimate {
            recipe_id,
            build_days,
            total_materials_cost,
            materials,
            daily_workforce_wage,
            total_expected_wages,
            contractor_profit,
            contractor_fee,
            daily_contractor_fee,
            mobilization_advance,
            total_estimated_cost,
            is_tech_unlocked,
            can_afford_advance,
        })
    }

    pub fn commission_facility_construction(
        &mut self,
        country_tag: CountryTag,
        company_id: CompanyId,
        recipe_id: RecipeId,
        region_id: RegionId,
        contractor_id: Option<CompanyId>,
    ) -> Result<(), String> {
        let estimate = self.calculate_facility_construction_cost(country_tag, recipe_id)?;
        if !estimate.is_tech_unlocked {
            return Err("Required technology is not unlocked".to_string());
        }

        let contractor = match contractor_id {
            Some(cid) => {
                let comp = self
                    .companies
                    .iter()
                    .find(|c| c.id == cid && c.country_tag == country_tag)
                    .ok_or_else(|| format!("Contractor company {:?} not found", cid))?;
                if comp.sector_kind != SectorKind::Construction {
                    return Err("Selected company is not a construction contractor".to_string());
                }
                cid
            }
            None => self
                .companies
                .iter()
                .filter(|c| {
                    c.country_tag == country_tag && c.sector_kind == SectorKind::Construction
                })
                .min_by_key(|c| {
                    self.country(country_tag)
                        .map(|co| {
                            co.construction_queue
                                .iter()
                                .filter(|p| p.contractor == c.id)
                                .count()
                        })
                        .unwrap_or(0)
                })
                .map(|c| c.id)
                .ok_or_else(|| "No construction contractor available in country".to_string())?,
        };

        {
            let country = self
                .country_mut(country_tag)
                .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

            if country.economy.treasury < estimate.mobilization_advance {
                return Err(format!(
                    "Insufficient treasury funds for initial advance: need {:.0}, have {:.0}",
                    estimate.mobilization_advance, country.economy.treasury
                ));
            }

            country.economy.treasury -= estimate.mobilization_advance;
        }

        if let Some(cont) = self.companies.iter_mut().find(|c| c.id == contractor) {
            cont.balance += estimate.mobilization_advance;
        }

        let (sector_kind, required_workers) = {
            let recipe = self
                .defs
                .recipes
                .get(recipe_id)
                .ok_or_else(|| format!("Recipe {:?} not found", recipe_id))?;
            (recipe.sector, recipe.construction_workforce.clone())
        };

        let daily_materials: Vec<(ResourceId, f64)> = estimate
            .materials
            .iter()
            .map(|m| (m.resource_id, m.daily_quantity))
            .collect();

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.construction_queue.push(ConstructionProject {
            target: ConstructionTarget::NewFacility {
                company_id,
                recipe_id,
                region_id,
                sector_kind,
            },
            daily_contractor_fee: estimate.daily_contractor_fee,
            contractor,
            payer: BudgetSource::Federal(country_tag),
            progress: 0.0,
            total_days: estimate.build_days,
            daily_materials,
            current_workforce: Vec::new(),
            required_workers,
            last_effective_speed: 1.0,
            consecutive_stalled_days: 0,
        });

        Ok(())
    }

    pub fn calculate_structure_upgrade_cost(
        &self,
        country_tag: CountryTag,
        structure_def_id: StructureDefId,
    ) -> Result<crate::world::StructureUpgradeEstimate, String> {
        let def = self
            .defs
            .structures
            .get(structure_def_id)
            .ok_or_else(|| format!("Structure {:?} not found", structure_def_id))?;

        let country = self
            .country(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        let exchange_rate = country.economy.exchange_rate.max(0.001);
        let build_days = def.build_time_days.max(30.0);

        let mut materials = Vec::with_capacity(def.construction_cost.len());
        let mut total_materials_cost = 0.0;

        for &(mat_id, qty) in &def.construction_cost {
            let unit_price = country.market.get(mat_id).map(|m| m.price).unwrap_or(50.0);
            let cost = qty * unit_price;
            total_materials_cost += cost;
            materials.push(crate::world::ConstructionMaterialEstimate {
                resource_id: mat_id,
                total_quantity: qty,
                daily_quantity: qty / build_days,
                local_unit_price: unit_price,
                total_cost: cost,
            });
        }

        let daily_workforce_wage: f64 = def
            .workforce_requirements
            .iter()
            .map(|(pop_type_id, count)| {
                let mult = self
                    .defs
                    .pop_types
                    .get(*pop_type_id)
                    .map(|t| t.wage_multiplier)
                    .unwrap_or(1.0);
                (*count as f64) * (1.2 * exchange_rate) * mult
            })
            .sum();

        let total_expected_wages = daily_workforce_wage * build_days;
        let contractor_profit =
            (total_materials_cost * 0.15 + 3_000.0 * exchange_rate).max(3_000.0 * exchange_rate);
        let contractor_fee = total_expected_wages + contractor_profit;
        let daily_contractor_fee = contractor_fee / build_days;

        let mobilization_advance = (daily_contractor_fee * 5.0).max(1_000.0 * exchange_rate);
        let total_estimated_cost = total_materials_cost + contractor_fee;
        let can_afford_advance = country.economy.treasury >= mobilization_advance;

        Ok(crate::world::StructureUpgradeEstimate {
            structure_def_id,
            build_days,
            total_materials_cost,
            materials,
            daily_workforce_wage,
            total_expected_wages,
            contractor_profit,
            contractor_fee,
            daily_contractor_fee,
            mobilization_advance,
            total_estimated_cost,
            can_afford_advance,
        })
    }

    pub fn commission_structure_upgrade(
        &mut self,
        country_tag: CountryTag,
        region_id: RegionId,
        structure_def_id: StructureDefId,
        contractor_id: Option<CompanyId>,
    ) -> Result<(), String> {
        let estimate = self.calculate_structure_upgrade_cost(country_tag, structure_def_id)?;

        let contractor = match contractor_id {
            Some(cid) => {
                let comp = self
                    .companies
                    .iter()
                    .find(|c| c.id == cid && c.country_tag == country_tag)
                    .ok_or_else(|| format!("Contractor company {:?} not found", cid))?;
                if comp.sector_kind != SectorKind::Construction {
                    return Err("Selected company is not a construction contractor".to_string());
                }
                cid
            }
            None => self
                .companies
                .iter()
                .filter(|c| {
                    c.country_tag == country_tag && c.sector_kind == SectorKind::Construction
                })
                .min_by_key(|c| {
                    self.country(country_tag)
                        .map(|co| {
                            co.construction_queue
                                .iter()
                                .filter(|p| p.contractor == c.id)
                                .count()
                        })
                        .unwrap_or(0)
                })
                .map(|c| c.id)
                .ok_or_else(|| "No construction contractor available in country".to_string())?,
        };

        {
            let country = self
                .country_mut(country_tag)
                .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

            if country.economy.treasury < estimate.mobilization_advance {
                return Err(format!(
                    "Insufficient treasury funds for initial advance: need {:.0}, have {:.0}",
                    estimate.mobilization_advance, country.economy.treasury
                ));
            }

            country.economy.treasury -= estimate.mobilization_advance;
        }

        if let Some(cont) = self.companies.iter_mut().find(|c| c.id == contractor) {
            cont.balance += estimate.mobilization_advance;
        }

        let required_workers = {
            let def = self
                .defs
                .structures
                .get(structure_def_id)
                .ok_or_else(|| format!("Structure {:?} not found", structure_def_id))?;
            def.workforce_requirements.clone()
        };

        let daily_materials: Vec<(ResourceId, f64)> = estimate
            .materials
            .iter()
            .map(|m| (m.resource_id, m.daily_quantity))
            .collect();

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.construction_queue.push(ConstructionProject {
            target: ConstructionTarget::UpgradeStructure {
                region_id,
                structure_id: structure_def_id,
                def_id: structure_def_id,
            },
            daily_contractor_fee: estimate.daily_contractor_fee,
            contractor,
            payer: BudgetSource::Federal(country_tag),
            progress: 0.0,
            total_days: estimate.build_days,
            daily_materials,
            current_workforce: Vec::new(),
            required_workers,
            last_effective_speed: 1.0,
            consecutive_stalled_days: 0,
        });

        Ok(())
    }

    pub fn cancel_construction_project(
        &mut self,
        country_tag: CountryTag,
        project_index: usize,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if project_index >= country.construction_queue.len() {
            return Err("Invalid project index".to_string());
        }

        let project = country.construction_queue.remove(project_index);

        let target_region = match project.target {
            ConstructionTarget::NewFacility { region_id, .. } => region_id,
            ConstructionTarget::UpgradeStructure { region_id, .. } => region_id,
            ConstructionTarget::NewRealEstate { region_id, .. } => region_id,
        };

        if let Some(region) = self.regions.iter_mut().find(|r| r.id == target_region) {
            for &(pop_type_id, count) in &project.current_workforce {
                if let Some(pop) = region
                    .pops
                    .iter_mut()
                    .find(|p| p.pop_type_id == pop_type_id)
                {
                    pop.fire(count as u64);
                }
            }
        }

        Ok(())
    }

    pub fn transfer_treasury_to_company(
        &mut self,
        country_tag: CountryTag,
        company_id: CompanyId,
        amount: f64,
    ) -> Result<(), String> {
        if amount <= 0.0 {
            return Err("Amount must be positive".to_string());
        }

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.economy.treasury < amount {
            return Err("Insufficient funds in treasury".to_string());
        }

        country.economy.treasury -= amount;

        let comp = self
            .companies
            .iter_mut()
            .find(|c| c.id == company_id && c.country_tag == country_tag)
            .ok_or_else(|| format!("Company {:?} not found in country", company_id))?;

        comp.balance += amount;

        Ok(())
    }

    pub fn nationalize_company(
        &mut self,
        country_tag: CountryTag,
        company_id: CompanyId,
        compensation: f64,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if compensation > 0.0 {
            if country.economy.treasury < compensation {
                return Err("Insufficient funds for nationalization compensation".to_string());
            }
            country.economy.treasury -= compensation;
        }

        let comp = self
            .companies
            .iter_mut()
            .find(|c| c.id == company_id && c.country_tag == country_tag)
            .ok_or_else(|| format!("Company {:?} not found in country", company_id))?;

        comp.ownership = Ownership::State;

        Ok(())
    }

    pub fn privatize_company(
        &mut self,
        country_tag: CountryTag,
        company_id: CompanyId,
        proceeds: f64,
    ) -> Result<(), String> {
        let comp = self
            .companies
            .iter_mut()
            .find(|c| c.id == company_id && c.country_tag == country_tag)
            .ok_or_else(|| format!("Company {:?} not found in country", company_id))?;

        if comp.ownership != Ownership::State {
            return Err("Company is not state-owned".to_string());
        }

        comp.ownership = Ownership::Private {
            shareholders_group_id: 0,
        };

        if proceeds > 0.0 {
            if let Some(country) = self.country_mut(country_tag) {
                country.economy.treasury += proceeds;
            }
        }

        Ok(())
    }

    pub fn set_company_reinvestment_ratio(
        &mut self,
        company_id: CompanyId,
        ratio: f64,
    ) -> Result<(), String> {
        let comp = self
            .companies
            .iter_mut()
            .find(|c| c.id == company_id)
            .ok_or_else(|| format!("Company {:?} not found", company_id))?;

        comp.reinvestment_ratio = ratio.clamp(0.0, 1.0);

        Ok(())
    }

    pub fn transfer_treasury_to_facility(
        &mut self,
        country_tag: CountryTag,
        facility_id: FacilityId,
        amount: f64,
    ) -> Result<(), String> {
        if amount <= 0.0 {
            return Err("Amount must be positive".to_string());
        }

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.economy.treasury < amount {
            return Err("Insufficient funds in treasury".to_string());
        }

        country.economy.treasury -= amount;

        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id && f.country_tag == country_tag)
            .ok_or_else(|| format!("Facility {:?} not found in country", facility_id))?;

        fac.local_reserve += amount;

        Ok(())
    }

    pub fn set_facility_priority_zone(
        &mut self,
        facility_id: FacilityId,
        enabled: bool,
    ) -> Result<(), String> {
        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id)
            .ok_or_else(|| format!("Facility {:?} not found", facility_id))?;

        fac.is_priority_development_zone = enabled;

        Ok(())
    }

    pub fn set_production_line_priority(
        &mut self,
        facility_id: FacilityId,
        line_index: usize,
        priority: Priority,
    ) -> Result<(), String> {
        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id)
            .ok_or_else(|| format!("Facility {:?} not found", facility_id))?;

        let line = fac
            .lines
            .get_mut(line_index)
            .ok_or_else(|| format!("Production line #{} not found", line_index))?;

        line.priority = priority;

        Ok(())
    }

    pub fn set_production_line_capacity_cap(
        &mut self,
        facility_id: FacilityId,
        line_index: usize,
        cap: Option<u32>,
    ) -> Result<(), String> {
        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id)
            .ok_or_else(|| format!("Facility {:?} not found", facility_id))?;

        let line = fac
            .lines
            .get_mut(line_index)
            .ok_or_else(|| format!("Production line #{} not found", line_index))?;

        line.target_capacity_cap = cap;

        Ok(())
    }

    pub fn set_production_line_target_wage(
        &mut self,
        facility_id: FacilityId,
        line_index: usize,
        target_wage: f64,
    ) -> Result<(), String> {
        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id)
            .ok_or_else(|| format!("Facility {:?} not found", facility_id))?;

        let line = fac
            .lines
            .get_mut(line_index)
            .ok_or_else(|| format!("Production line #{} not found", line_index))?;

        line.target_wage = target_wage.max(0.1);

        Ok(())
    }

    pub fn set_market_price_corridor(
        &mut self,
        country_tag: CountryTag,
        resource_id: ResourceId,
        min_price: f64,
        max_price: f64,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country
            .market
            .set_corridor(resource_id, min_price.max(0.01), max_price.max(min_price));

        Ok(())
    }

    pub fn reset_market_price_corridor(
        &mut self,
        country_tag: CountryTag,
        resource_id: ResourceId,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if let Some(rm) = country.market.get_mut(resource_id) {
            rm.min_price = rm.base_price * 0.3;
            rm.max_price = rm.base_price * 3.0;
        }

        Ok(())
    }

    pub fn transfer_treasury_to_region(
        &mut self,
        country_tag: CountryTag,
        region_id: RegionId,
        amount: f64,
    ) -> Result<(), String> {
        if amount <= 0.0 {
            return Err("Amount must be positive".to_string());
        }

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.economy.treasury < amount {
            return Err("Insufficient funds in treasury".to_string());
        }

        country.economy.treasury -= amount;

        let reg = self
            .regions
            .iter_mut()
            .find(|r| r.id == region_id && r.owner == Some(country_tag))
            .ok_or_else(|| format!("Region {:?} not found in country", region_id))?;

        reg.local_treasury += amount;

        Ok(())
    }

    pub fn repair_region_structure(
        &mut self,
        country_tag: CountryTag,
        region_id: RegionId,
        structure_def_id: StructureDefId,
        repair_cost: f64,
    ) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if repair_cost > 0.0 {
            if country.economy.treasury < repair_cost {
                return Err("Insufficient funds in treasury for repairs".to_string());
            }
            country.economy.treasury -= repair_cost;
        }

        let reg = self
            .regions
            .iter_mut()
            .find(|r| r.id == region_id && r.owner == Some(country_tag))
            .ok_or_else(|| format!("Region {:?} not found in country", region_id))?;

        let structure = reg
            .structures
            .iter_mut()
            .find(|s| s.def_id == structure_def_id)
            .ok_or_else(|| format!("Structure {:?} not found in region", structure_def_id))?;

        structure.wear = 0.0;
        structure.efficiency = 1.0;

        Ok(())
    }

    pub fn calculate_line_expansion_cost(
        &self,
        country_tag: CountryTag,
        recipe_id: RecipeId,
        steps: u16,
    ) -> Option<f64> {
        let exchange_rate = self.country(country_tag)?.economy.exchange_rate.max(0.001);
        let recipe = self.defs.recipes.get(recipe_id)?;
        let cost_per_step =
            (recipe.base_expansion_cost * 0.05 * exchange_rate).max(2_000.0 * exchange_rate);
        Some(cost_per_step * (steps as f64))
    }

    pub fn expand_facility_line_capacity(
        &mut self,
        country_tag: CountryTag,
        facility_id: FacilityId,
        line_index: usize,
        steps: u32,
        pay_from_treasury: bool,
    ) -> Result<(u32, f64), String> {
        if steps == 0 {
            return Err("Expansion steps must be positive".to_string());
        }

        let defs = Arc::clone(&self.defs);
        let exchange_rate = self
            .country(country_tag)
            .map(|c| c.economy.exchange_rate.max(0.001))
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        let (comp_id, recipe_id, current_cap, max_cap) = {
            let fac = self
                .facilities
                .iter()
                .find(|f| f.id == facility_id && f.country_tag == country_tag)
                .ok_or_else(|| format!("Facility {:?} not found in country", facility_id))?;

            let line = fac
                .lines
                .get(line_index)
                .ok_or_else(|| format!("Line #{} not found", line_index))?;

            (
                fac.company_id,
                line.recipe_id,
                line.installed_capacity,
                line.natural_max_capacity.unwrap_or(1000),
            )
        };

        if current_cap >= max_cap {
            return Err(format!(
                "Capacity reached natural limit: {} / {}",
                current_cap, max_cap
            ));
        }

        let actual_steps = steps.min(max_cap.saturating_sub(current_cap));
        if actual_steps == 0 {
            return Err("Cannot expand beyond maximum capacity".to_string());
        }

        let recipe = defs
            .recipes
            .get(recipe_id)
            .ok_or_else(|| format!("Recipe {:?} not found", recipe_id))?;

        if let Some(req_tech) = recipe.required_tech {
            let country = self
                .country(country_tag)
                .ok_or_else(|| format!("Country {:?} not found", country_tag))?;
            if !country.has_technology(req_tech) {
                return Err("Required technology is not unlocked".to_string());
            }
        }

        let cost_per_step =
            (recipe.base_expansion_cost * 0.05 * exchange_rate).max(2_000.0 * exchange_rate);
        let total_cost = cost_per_step * (actual_steps as f64);

        if pay_from_treasury {
            let country = self
                .country_mut(country_tag)
                .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

            if country.economy.treasury < total_cost {
                return Err(format!(
                    "Insufficient treasury funds: need {:.0}, have {:.0}",
                    total_cost, country.economy.treasury
                ));
            }
            country.economy.treasury -= total_cost;
        } else {
            let comp = self
                .companies
                .iter_mut()
                .find(|c| c.id == comp_id)
                .ok_or_else(|| format!("Company {:?} not found", comp_id))?;

            if comp.balance < total_cost {
                return Err(format!(
                    "Insufficient company balance: need {:.0}, have {:.0}",
                    total_cost, comp.balance
                ));
            }
            comp.balance -= total_cost;
        }

        let fac = self
            .facilities
            .iter_mut()
            .find(|f| f.id == facility_id)
            .ok_or_else(|| format!("Facility {:?} not found", facility_id))?;

        let line = fac
            .lines
            .get_mut(line_index)
            .ok_or_else(|| format!("Line #{} not found", line_index))?;

        line.installed_capacity += actual_steps;

        Ok((line.installed_capacity, total_cost))
    }

    pub fn set_active_research(
        &mut self,
        country_tag: CountryTag,
        tech_id: crate::world::TechnologyId,
    ) -> Result<(), String> {
        let prerequisites = self
            .defs
            .technologies
            .get(tech_id)
            .ok_or_else(|| format!("Technology {:?} not found", tech_id))?
            .prerequisites
            .clone();

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.has_technology(tech_id) {
            return Err("Technology already researched".to_string());
        }

        for prereq in &prerequisites {
            if !country.has_technology(*prereq) {
                return Err("Prerequisites not satisfied".to_string());
            }
        }

        if country.active_research != Some(tech_id) {
            country.active_research = Some(tech_id);
            country.research_progress = 0.0;
        }

        Ok(())
    }

    pub fn cancel_active_research(&mut self, country_tag: CountryTag) -> Result<(), String> {
        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.active_research = None;
        country.research_progress = 0.0;
        Ok(())
    }

    pub fn get_technologies_overview(
        &self,
        country_tag: CountryTag,
    ) -> Result<Vec<crate::world::TechnologyOverviewItem>, String> {
        let country = self
            .country(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        let mut overview = Vec::with_capacity(self.defs.technologies.len());

        for (tech_id, tech_def) in self.defs.technologies.iter() {
            let (status, progress, progress_ratio) = if country.has_technology(tech_id) {
                (
                    crate::world::TechnologyResearchStatus::Researched,
                    tech_def.cost,
                    1.0,
                )
            } else if country.active_research == Some(tech_id) {
                let ratio = if tech_def.cost > 0.0 {
                    (country.research_progress / tech_def.cost).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                (
                    crate::world::TechnologyResearchStatus::Active,
                    country.research_progress,
                    ratio,
                )
            } else {
                let prerequisites_met = tech_def
                    .prerequisites
                    .iter()
                    .all(|prereq| country.has_technology(*prereq));
                if prerequisites_met {
                    (crate::world::TechnologyResearchStatus::Available, 0.0, 0.0)
                } else {
                    (crate::world::TechnologyResearchStatus::Locked, 0.0, 0.0)
                }
            };

            overview.push(crate::world::TechnologyOverviewItem {
                id: tech_id,
                key: tech_def.key.clone(),
                name: tech_def.name.clone(),
                cost: tech_def.cost,
                prerequisites: tech_def.prerequisites.clone(),
                status,
                progress,
                progress_ratio,
            });
        }

        Ok(overview)
    }

    pub fn get_active_research_overview(
        &self,
        country_tag: CountryTag,
    ) -> Option<crate::world::TechnologyOverviewItem> {
        let country = self.country(country_tag)?;
        let active_id = country.active_research?;
        let tech_def = self.defs.technologies.get(active_id)?;
        let progress_ratio = if tech_def.cost > 0.0 {
            (country.research_progress / tech_def.cost).clamp(0.0, 1.0)
        } else {
            1.0
        };

        Some(crate::world::TechnologyOverviewItem {
            id: active_id,
            key: tech_def.key.clone(),
            name: tech_def.name.clone(),
            cost: tech_def.cost,
            prerequisites: tech_def.prerequisites.clone(),
            status: crate::world::TechnologyResearchStatus::Active,
            progress: country.research_progress,
            progress_ratio,
        })
    }

    pub fn commission_real_estate_construction(
        &mut self,
        country_tag: CountryTag,
        region_id: RegionId,
        kind: RealEstateKind,
        units: u32,
        base_rent: f64,
        owner_company_id: Option<CompanyId>,
        contractor_id: Option<CompanyId>,
        payer: BudgetSource,
    ) -> Result<(), String> {
        let region = self
            .regions
            .iter()
            .find(|r| r.id == region_id)
            .ok_or_else(|| format!("Region {:?} not found", region_id))?;

        if region.owner != Some(country_tag) {
            return Err("Cannot build in foreign region".to_string());
        }

        let exchange_rate = self
            .country(country_tag)
            .map(|c| c.economy.exchange_rate.max(0.001))
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        let contractor = match contractor_id {
            Some(cid) => {
                let comp = self
                    .companies
                    .iter()
                    .find(|c| c.id == cid && c.country_tag == country_tag)
                    .ok_or_else(|| format!("Contractor company {:?} not found", cid))?;
                if comp.sector_kind != SectorKind::Construction {
                    return Err("Selected company is not a construction contractor".to_string());
                }
                cid
            }
            None => self
                .companies
                .iter()
                .filter(|c| {
                    c.country_tag == country_tag && c.sector_kind == SectorKind::Construction
                })
                .min_by_key(|c| {
                    self.country(country_tag)
                        .map(|co| {
                            co.construction_queue
                                .iter()
                                .filter(|p| p.contractor == c.id)
                                .count()
                        })
                        .unwrap_or(0)
                })
                .map(|c| c.id)
                .ok_or_else(|| "No construction contractor available in country".to_string())?,
        };

        let build_days = 60.0;
        let units_scale = (units as f64 / 1000.0).max(0.1);

        let steel_id = self.defs.resources.get_id("steel");
        let cm_id = self.defs.resources.get_id("construction_materials");

        let mut daily_materials = Vec::new();
        if let Some(cm) = cm_id {
            daily_materials.push((cm, 400.0 * units_scale / build_days));
        }
        if let Some(steel) = steel_id {
            daily_materials.push((steel, 80.0 * units_scale / build_days));
        }

        let worker_pop = self.defs.pop_types.get_id("workers");
        let engineer_pop = self.defs.pop_types.get_id("engineers");

        let mut required_workers = Vec::new();
        if let Some(w) = worker_pop {
            required_workers.push((w, (150.0 * units_scale) as u32));
        }
        if let Some(e) = engineer_pop {
            required_workers.push((e, (25.0 * units_scale) as u32));
        }

        let daily_contractor_fee = 100.0 * exchange_rate * units_scale;

        let country = self
            .country_mut(country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        country.construction_queue.push(ConstructionProject {
            target: ConstructionTarget::NewRealEstate {
                company_id: owner_company_id,
                kind,
                region_id,
                units,
                base_rent,
            },
            daily_contractor_fee,
            contractor,
            payer,
            progress: 0.0,
            total_days: build_days,
            daily_materials,
            current_workforce: Vec::new(),
            required_workers,
            last_effective_speed: 1.0,
            consecutive_stalled_days: 0,
        });

        Ok(())
    }
}
