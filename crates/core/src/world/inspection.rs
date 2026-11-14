use crate::CountryTag;
use crate::economy::{CompanyId, FacilityId, RecipeId, SectorKind};
use crate::world::World;
use crate::world::construction::ConstructionTarget;
use crate::world::map::RegionId;
use crate::world::pops::PopTypeId;
use crate::world::structure::StructureDefId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionPopInspection {
    pub pop_type_id: PopTypeId,
    pub key: String,
    pub total_count: u64,
    pub employed_count: u64,
    pub unemployed_count: u64,
    pub average_wage: f64,
    pub daily_income: f64,
    pub satisfaction: f32,
    pub radicalism: f32,
    pub savings: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionFacilityLineInspection {
    pub recipe_id: RecipeId,
    pub recipe_key: String,
    pub installed_capacity: u32,
    pub utilization: f64,
    pub target_wage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionFacilityInspection {
    pub facility_id: FacilityId,
    pub company_id: CompanyId,
    pub company_name: String,
    pub sector_kind: SectorKind,
    pub is_priority_zone: bool,
    pub net_profit: f64,
    pub local_reserve: f64,
    pub lines: Vec<RegionFacilityLineInspection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionConstructionInspection {
    pub target_description: String,
    pub contractor_id: CompanyId,
    pub contractor_name: String,
    pub progress: f64,
    pub total_days: f64,
    pub progress_ratio: f64,
    pub daily_contractor_fee: f64,
    pub is_stalled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionStructureInspection {
    pub def_id: StructureDefId,
    pub key: String,
    pub level: u32,
    pub efficiency: f64,
    pub wear: f64,
    pub required_budget: f64,
    pub allocated_budget: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionDetailedInspection {
    pub region_id: RegionId,
    pub owner: Option<CountryTag>,
    pub local_treasury: f64,
    pub infrastructure_level: f64,
    pub infrastructure_capacity: f64,
    pub infrastructure_usage: f64,
    pub market_access: f64,
    pub total_population: u64,
    pub employed_population: u64,
    pub unemployed_population: u64,
    pub unemployment_rate: f64,
    pub average_satisfaction: f32,
    pub average_radicalism: f32,
    pub pops: Vec<RegionPopInspection>,
    pub facilities: Vec<RegionFacilityInspection>,
    pub construction_projects: Vec<RegionConstructionInspection>,
    pub structures: Vec<RegionStructureInspection>,
}

impl World {
    pub fn inspect_region(&self, region_id: RegionId) -> Option<RegionDetailedInspection> {
        let region = self.region(region_id)?;

        let mut total_population = 0u64;
        let mut employed_population = 0u64;
        let mut unemployed_population = 0u64;
        let mut weighted_satisfaction = 0.0f64;
        let mut weighted_radicalism = 0.0f64;

        let mut pops_inspection = Vec::with_capacity(region.pops.len());
        for pop in &region.pops {
            let total = pop.employed.count.saturating_add(pop.unemployed.count);
            total_population = total_population.saturating_add(total);
            employed_population = employed_population.saturating_add(pop.employed.count);
            unemployed_population = unemployed_population.saturating_add(pop.unemployed.count);

            weighted_satisfaction += (pop.satisfaction as f64) * (total as f64);
            weighted_radicalism += (pop.radicalism as f64) * (total as f64);

            let key = self
                .defs
                .pop_types
                .get(pop.pop_type_id)
                .map(|p| p.key.clone())
                .unwrap_or_else(|| format!("pop_{}", pop.pop_type_id.0));

            pops_inspection.push(RegionPopInspection {
                pop_type_id: pop.pop_type_id,
                key,
                total_count: total,
                employed_count: pop.employed.count,
                unemployed_count: pop.unemployed.count,
                average_wage: pop.average_wage,
                daily_income: pop.daily_income,
                satisfaction: pop.satisfaction,
                radicalism: pop.radicalism,
                savings: pop.employed.savings + pop.unemployed.savings,
            });
        }

        let unemployment_rate = if total_population > 0 {
            (unemployed_population as f64 / total_population as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let average_satisfaction = if total_population > 0 {
            (weighted_satisfaction / total_population as f64).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };

        let average_radicalism = if total_population > 0 {
            (weighted_radicalism / total_population as f64).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let mut facilities_inspection = Vec::with_capacity(region.facilities.len());
        for &facility_id in &region.facilities {
            if let Some(facility) = self.facilities.iter().find(|f| f.id == facility_id) {
                let company = self.companies.iter().find(|c| c.id == facility.company_id);
                let company_name = company
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| format!("Company #{}", facility.company_id.0));

                let mut lines = Vec::with_capacity(facility.lines.len());
                for line in &facility.lines {
                    let recipe_key = self
                        .defs
                        .recipes
                        .get(line.recipe_id)
                        .map(|r| r.key.clone())
                        .unwrap_or_else(|| format!("recipe_{}", line.recipe_id.0));

                    lines.push(RegionFacilityLineInspection {
                        recipe_id: line.recipe_id,
                        recipe_key,
                        installed_capacity: line.installed_capacity,
                        utilization: line.utilization,
                        target_wage: line.target_wage,
                    });
                }

                facilities_inspection.push(RegionFacilityInspection {
                    facility_id,
                    company_id: facility.company_id,
                    company_name,
                    sector_kind: facility.sector_kind,
                    is_priority_zone: facility.is_priority_development_zone,
                    net_profit: facility.net_profit(),
                    local_reserve: facility.local_reserve,
                    lines,
                });
            }
        }

        let mut construction_inspections = Vec::new();
        if let Some(owner_tag) = region.owner {
            if let Some(country) = self.country(owner_tag) {
                for project in &country.construction_queue {
                    let target_in_region = match &project.target {
                        ConstructionTarget::NewFacility {
                            region_id: r_id, ..
                        } => *r_id == region_id,
                        ConstructionTarget::UpgradeStructure {
                            region_id: r_id, ..
                        } => *r_id == region_id,
                        ConstructionTarget::NewRealEstate {
                            region_id: r_id, ..
                        } => *r_id == region_id,
                    };

                    if target_in_region {
                        let target_description = match &project.target {
                            ConstructionTarget::NewFacility { recipe_id, .. } => self
                                .defs
                                .recipes
                                .get(*recipe_id)
                                .map(|r| r.key.clone())
                                .unwrap_or_else(|| format!("facility_{}", recipe_id.0)),
                            ConstructionTarget::UpgradeStructure { def_id, .. } => self
                                .defs
                                .structures
                                .get(*def_id)
                                .map(|s| s.key.clone())
                                .unwrap_or_else(|| format!("structure_{}", def_id.0)),
                            ConstructionTarget::NewRealEstate { kind, units, .. } => {
                                format!("{:?} ({} units)", kind, units)
                            }
                        };

                        let contractor_name = self
                            .companies
                            .iter()
                            .find(|c| c.id == project.contractor)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| format!("Contractor #{}", project.contractor.0));

                        let progress_ratio = if project.total_days > 0.0 {
                            (project.progress / project.total_days).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };

                        construction_inspections.push(RegionConstructionInspection {
                            target_description,
                            contractor_id: project.contractor,
                            contractor_name,
                            progress: project.progress,
                            total_days: project.total_days,
                            progress_ratio,
                            daily_contractor_fee: project.daily_contractor_fee,
                            is_stalled: project.consecutive_stalled_days > 0,
                        });
                    }
                }
            }
        }

        let mut structures_inspection = Vec::with_capacity(region.structures.len());
        for structure in &region.structures {
            let key = self
                .defs
                .structures
                .get(structure.def_id)
                .map(|s| s.key.clone())
                .unwrap_or_else(|| format!("structure_{}", structure.def_id.0));

            structures_inspection.push(RegionStructureInspection {
                def_id: structure.def_id,
                key,
                level: structure.level,
                efficiency: structure.efficiency,
                wear: structure.wear,
                required_budget: structure.required_budget,
                allocated_budget: structure.allocated_budget,
            });
        }

        Some(RegionDetailedInspection {
            region_id,
            owner: region.owner,
            local_treasury: region.local_treasury,
            infrastructure_level: region.infrastructure_level,
            infrastructure_capacity: region.infrastructure_capacity,
            infrastructure_usage: region.infrastructure_usage,
            market_access: region.market_access,
            total_population,
            employed_population,
            unemployed_population,
            unemployment_rate,
            average_satisfaction,
            average_radicalism,
            pops: pops_inspection,
            facilities: facilities_inspection,
            construction_projects: construction_inspections,
            structures: structures_inspection,
        })
    }
}
