use super::dto::{
    CompanyDto, CountryDto, FacilityDto, ProductionLineDto, ProvinceDto, RegionDto,
    RegionStructureDto, ScenarioMeta,
};
use super::error::LoadError;
use super::fs_utils::{load_file, load_list_from_dir};
use crate::defs::GameDefs;
use crate::economy::finance::BudgetSource;
use crate::economy::market::NationalMarket;
use crate::economy::{
    CompanyId, CompanyState, DigitalPlatform, DigitalPlatformId, DigitalPlatformKind, FacilityId,
    FacilityState, ProductionLineId, ProductionLineState, RealEstateId, RealEstateProperty,
    SectorKind, Stockpile,
};
use crate::world::map::{
    Province, ProvinceId, Region, RegionId, TransportConnectionType, TransportEdge,
    TransportNetwork,
};
use crate::world::{
    CountryEconomy, GameDate, Pop, PopTypeId, RegionStructure, StateStructure, StructureKind,
};
use crate::{Country, CountryTag, World};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub fn load_world<P: AsRef<Path>>(scenario_dir: P, defs: GameDefs) -> Result<World, LoadError> {
    let p = scenario_dir.as_ref();

    let meta: ScenarioMeta = load_file(p.join("scenario.ron"))?;
    let start_date = parse_game_date(&meta.start_date)?;
    let mut world = World::new(defs, start_date);

    load_countries(p, &mut world)?;
    load_regions(p, &mut world)?;
    load_provinces(p, &mut world)?;
    load_companies(p, &mut world)?;
    load_transport_network(p, &mut world)?;

    initialize_digital_platforms(&mut world);
    initialize_baseline_gdp(&mut world);

    Ok(world)
}

fn load_companies(scenario_dir: &Path, world: &mut World) -> Result<(), LoadError> {
    let companies_dir = scenario_dir.join("companies");

    let companies_dto: Vec<CompanyDto> = if companies_dir.is_dir() {
        load_list_from_dir(companies_dir, "ron")?
    } else {
        let single_file = scenario_dir.join("companies.ron");
        if single_file.exists() {
            load_file(single_file)?
        } else {
            Vec::new()
        }
    };

    for c in companies_dto {
        let country_tag = CountryTag::try_from(c.country_tag.as_str())
            .map_err(|_| LoadError::InvalidCountryTag(c.country_tag.clone()))?;

        world.companies.push(CompanyState {
            id: CompanyId(c.id),
            name: c.name,
            country_tag,
            ownership: c.ownership,
            balance: c.balance,
            reinvestment_ratio: c.reinvestment_ratio,
            monthly_revenue: 0.0,
            monthly_expenses: 0.0,
            last_month_profit: 0.0,
            sector_kind: c.sector_kind,
        });
    }

    Ok(())
}

fn load_transport_network(scenario_dir: &Path, world: &mut World) -> Result<(), LoadError> {
    let transport_file = scenario_dir.join("transport_network.ron");
    let mut edges: Vec<TransportEdge> = if transport_file.exists() {
        load_file(transport_file)?
    } else {
        Vec::new()
    };

    let mut existing_pairs: HashSet<(u16, u16)> = HashSet::new();
    for edge in &edges {
        let a = edge.from.0.min(edge.to.0);
        let b = edge.from.0.max(edge.to.0);
        existing_pairs.insert((a, b));
    }

    for region in &world.regions {
        for &neighbor_id in &region.neighbors {
            let a = region.id.0.min(neighbor_id.0);
            let b = region.id.0.max(neighbor_id.0);
            if a != b && existing_pairs.insert((a, b)) {
                edges.push(TransportEdge {
                    from: RegionId(a),
                    to: RegionId(b),
                    connection_type: TransportConnectionType::Road,
                    distance_km: 200.0,
                });
            }
        }
    }

    let country_regions: Vec<Vec<RegionId>> =
        world.countries.iter().map(|c| c.regions.clone()).collect();

    for regions in country_regions {
        if regions.len() <= 1 {
            continue;
        }

        let has_explicit_neighbors = regions
            .iter()
            .filter_map(|&rid| world.region(rid))
            .any(|r| !r.neighbors.is_empty());

        if !has_explicit_neighbors {
            for i in 0..regions.len() - 1 {
                let r1 = regions[i];
                let r2 = regions[i + 1];
                let a = r1.0.min(r2.0);
                let b = r1.0.max(r2.0);
                if a != b && existing_pairs.insert((a, b)) {
                    edges.push(TransportEdge {
                        from: RegionId(a),
                        to: RegionId(b),
                        connection_type: TransportConnectionType::Road,
                        distance_km: 250.0,
                    });
                }
            }

            let hub = regions[0];
            for &rid in regions.iter().step_by(5) {
                if rid != hub {
                    let a = hub.0.min(rid.0);
                    let b = hub.0.max(rid.0);
                    if a != b && existing_pairs.insert((a, b)) {
                        edges.push(TransportEdge {
                            from: RegionId(a),
                            to: RegionId(b),
                            connection_type: TransportConnectionType::Road,
                            distance_km: 350.0,
                        });
                    }
                }
            }
        }
    }

    for edge in &edges {
        if matches!(
            edge.connection_type,
            TransportConnectionType::Road | TransportConnectionType::Rail
        ) {
            let from = edge.from;
            let to = edge.to;
            if let Some(r) = world.region_mut(from) {
                r.add_neighbor(to);
            }
            if let Some(r) = world.region_mut(to) {
                r.add_neighbor(from);
            }
        }
    }

    let max_reg_id = world
        .regions
        .iter()
        .map(|r| r.id.0 as usize)
        .max()
        .unwrap_or(0);
    world.transport_network = TransportNetwork::new(edges, max_reg_id);
    Ok(())
}

fn load_countries(scenario_dir: &Path, world: &mut World) -> Result<(), LoadError> {
    let countries: Vec<CountryDto> = load_list_from_dir(scenario_dir.join("countries"), "ron")?;

    let all_kinds = [
        StructureKind::BasicEducation,
        StructureKind::HighEducation,
        StructureKind::Health,
        StructureKind::LawEnforcement,
        StructureKind::Emergency,
        StructureKind::Administration,
        StructureKind::Infrastructure,
    ];

    for c in countries {
        let tag = parse_country_tag(&c.tag)?;

        let mut structures: Vec<StateStructure> = all_kinds
            .iter()
            .map(|&kind| StateStructure {
                kind,
                payer: BudgetSource::Federal(tag),
                funding_level: 1.0,
                monthly_budget: 0.0,
                oversight: 1.0,
                total_demand: 0.0,
                period_spent: 0.0,
                period_corruption: 0.0,
            })
            .collect();

        for custom in c.structures {
            if let Some(s) = structures.iter_mut().find(|s| s.kind == custom.kind) {
                s.funding_level = custom.funding_level;
                s.oversight = custom.oversight;
                if let Some(payer) = custom.payer {
                    s.payer = payer;
                }
            }
        }

        let economy = CountryEconomy {
            currency_code: c.economy.currency_code,
            currency_symbol: c.economy.currency_symbol,
            exchange_rate: c.economy.exchange_rate,
            treasury: c.economy.treasury,
            national_debt: c.economy.national_debt,
            nominal_gdp: c.economy.nominal_gdp,
            corporate_tax: c.economy.corporate_tax,
            income_tax: c.economy.income_tax,
            tariff_rate: c.economy.tariff_rate,
            bailout_policy: c.economy.bailout_policy,
            max_subsidy_budget_ratio: c.economy.max_subsidy_budget_ratio,
            welfare: c.economy.welfare,
            ..Default::default()
        };

        let market =
            NationalMarket::from_registry_scaled(&world.defs.resources, economy.exchange_rate);
        let mut country = Country::with_economy(tag, economy, market);
        country.structures = structures;

        for tech_key in &c.technologies {
            let tech_id = world.defs.technologies.get_id(tech_key).ok_or_else(|| {
                LoadError::Registration(format!(
                    "Country '{}' has unknown technology '{}'",
                    c.tag, tech_key
                ))
            })?;
            country.unlock_technology(tech_id);
        }

        if let Some(res_key) = &c.active_research {
            let tech_id = world.defs.technologies.get_id(res_key).ok_or_else(|| {
                LoadError::Registration(format!(
                    "Country '{}' has unknown active research '{}'",
                    c.tag, res_key
                ))
            })?;
            country.active_research = Some(tech_id);
        }

        world.add_country(country);
    }
    Ok(())
}

fn load_regions(scenario_dir: &Path, world: &mut World) -> Result<(), LoadError> {
    let regions_dto: Vec<RegionDto> = load_list_from_dir(scenario_dir.join("regions"), "ron")?;
    let resource_count = world.defs.resources.count();

    for r_dto in regions_dto {
        let mut region = build_region(&r_dto, resource_count)?;

        for f_dto in r_dto.facilities {
            let facility = build_facility(f_dto, &region, &world.defs)?;
            let fac_id = world.add_facility(facility);
            region.facilities.push(fac_id);
        }

        for s_dto in r_dto.structures {
            let structure = build_region_structure(s_dto, &world.defs)?;
            region.structures.push(structure);
        }

        for re_dto in r_dto.real_estate {
            let country_tag = region
                .owner
                .unwrap_or_else(|| parse_country_tag("USA").unwrap());
            let prop = RealEstateProperty::new(
                RealEstateId(0),
                re_dto.kind,
                re_dto.owner_company_id.map(CompanyId),
                region.id,
                country_tag,
                re_dto.total_units,
                re_dto.base_rent,
            );
            world.add_real_estate(prop);
        }

        if let Some(owner_tag) = region.owner {
            if let Some(country) = world.country_mut(owner_tag) {
                country.regions.push(region.id);
            } else {
                eprintln!(
                    "WARN: Region {:?} refers to undefined country {:?}",
                    region.id, owner_tag
                );
            }
        }

        world.regions.push(region);
    }
    Ok(())
}

fn load_provinces(scenario_dir: &Path, world: &mut World) -> Result<(), LoadError> {
    let provinces_dto: Vec<ProvinceDto> =
        load_list_from_dir(scenario_dir.join("provinces"), "ron")?;

    let mut region_pop_totals: HashMap<RegionId, BTreeMap<PopTypeId, u64>> = HashMap::new();
    let mut province_totals: HashMap<ProvinceId, u64> = HashMap::new();

    for p_dto in &provinces_dto {
        let prov_id = ProvinceId(p_dto.id);
        let reg_id = RegionId(p_dto.region_id);

        let mut prov_sum: u64 = 0;
        let reg_map = region_pop_totals.entry(reg_id).or_default();

        for (pop_type_key, &count) in &p_dto.population {
            let pop_type_id = world
                .defs
                .pop_types
                .get_id(pop_type_key)
                .ok_or_else(|| LoadError::UnknownPopType(pop_type_key.clone()))?;

            *reg_map.entry(pop_type_id).or_insert(0) += count;
            prov_sum += count;
        }

        province_totals.insert(prov_id, prov_sum);
    }

    for region in &mut world.regions {
        if let Some(pop_map) = region_pop_totals.get(&region.id) {
            let mut pops: Vec<Pop> = pop_map
                .iter()
                .map(|(&type_id, &reg_pop_count)| Pop::new(type_id, reg_pop_count))
                .collect();
            pops.sort_by_key(|p| p.pop_type_id);
            region.pops = pops;
        }
    }

    let region_indices: HashMap<RegionId, usize> = world
        .regions
        .iter()
        .enumerate()
        .map(|(idx, r)| (r.id, idx))
        .collect();

    for p_dto in provinces_dto {
        let controller = p_dto
            .controller
            .as_deref()
            .map(parse_country_tag)
            .transpose()?;
        let prov_id = ProvinceId(p_dto.id);
        let reg_id = RegionId(p_dto.region_id);

        let &reg_idx = region_indices
            .get(&reg_id)
            .ok_or(LoadError::OrphanProvince {
                province_id: p_dto.id,
                region_id: p_dto.region_id,
            })?;

        let region = &mut world.regions[reg_idx];

        region.provinces.push(prov_id);

        let reg_total: u64 = region_pop_totals
            .get(&reg_id)
            .map(|m| m.values().sum())
            .unwrap_or(0);

        let prov_total = *province_totals.get(&prov_id).unwrap_or(&0);

        let weight = if reg_total > 0 {
            prov_total as f64 / reg_total as f64
        } else {
            0.0
        };

        let prov = Province {
            id: prov_id,
            region_id: reg_id,
            terrain: p_dto.terrain,
            controller,
            population_weight: weight,
        };

        world.provinces.push(prov);
    }

    Ok(())
}

fn build_region(dto: &RegionDto, resource_count: usize) -> Result<Region, LoadError> {
    let reg_id = RegionId(dto.id);
    let owner_tag = dto.owner.as_deref().map(parse_country_tag).transpose()?;

    let mut region = Region::new(
        reg_id,
        owner_tag,
        Stockpile::new(resource_count),
        dto.infrastructure_level,
    );

    for &neighbor in &dto.neighbors {
        region.add_neighbor(RegionId(neighbor));
    }

    Ok(region)
}

fn build_facility(
    dto: FacilityDto,
    region: &Region,
    defs: &GameDefs,
) -> Result<FacilityState, LoadError> {
    let mut lines = Vec::with_capacity(dto.lines.len());

    for (line_idx, line_dto) in dto.lines.into_iter().enumerate() {
        let line = build_production_line(line_idx as u32, line_dto, defs)?;
        lines.push(line);
    }

    let country_tag = match dto.country_tag {
        Some(ref tag_str) => parse_country_tag(tag_str)?,
        None => region.owner.ok_or_else(|| {
            LoadError::InvalidCountryTag(format!(
                "Facility {} in region {:?} has no country_tag and region has no owner",
                dto.company_id, region.id
            ))
        })?,
    };

    let mut stockpile = Stockpile::new(defs.resources.count());
    if let Some(explicit_stock) = dto.stockpile {
        for (res_key, amount) in explicit_stock {
            if let Some(res_id) = defs.resources.get_id(&res_key) {
                stockpile.add(res_id, amount.max(0.0));
            }
        }
    } else {
        for line in &lines {
            if line.installed_capacity > 0 {
                if let Some(recipe) = defs.recipes.get(line.recipe_id) {
                    let capacity = line.installed_capacity as f64;
                    for &(res_id, req_per_cap) in &recipe.inputs {
                        let initial_amount = req_per_cap * capacity * recipe.safety_stock_days;
                        stockpile.add(res_id, initial_amount);
                    }
                }
            }
        }
    }

    Ok(FacilityState {
        id: FacilityId(0),
        country_tag,
        region_id: region.id,
        company_id: CompanyId(dto.company_id),
        sector_kind: dto.sector_kind,
        lines,
        is_priority_development_zone: dto.is_priority_development_zone,
        local_reserve: dto.local_reserve,
        pending_deliveries: Vec::new(),
        period_revenue: dto.period_revenue,
        period_material_cost: dto.period_material_cost,
        period_wage_cost: dto.period_wage_cost,
        stockpile,
    })
}

fn build_production_line(
    line_id: u32,
    dto: ProductionLineDto,
    defs: &GameDefs,
) -> Result<ProductionLineState, LoadError> {
    let recipe_id = defs
        .recipes
        .get_id(&dto.recipe_key)
        .ok_or(LoadError::UnknownRecipeKey(dto.recipe_key))?;

    Ok(ProductionLineState {
        id: ProductionLineId(line_id),
        recipe_id,
        installed_capacity: dto.installed_capacity,
        natural_max_capacity: dto.natural_max_capacity,
        target_capacity_cap: dto.target_capacity_cap,
        utilization: 0.0,
        potential_utilization: 0.0,
        material_utilization: 0.0,
        priority: dto.priority,
        expansion_progress: 0.0,
        last_day_output: 0.0,
        input_shortage: Vec::new(),
        employed_workers: Vec::new(),
        target_wage: dto.target_wage,
        consecutive_idle_days: 0,
    })
}

fn build_region_structure(
    dto: RegionStructureDto,
    defs: &GameDefs,
) -> Result<RegionStructure, LoadError> {
    let def_id = defs
        .structures
        .get_id(&dto.key)
        .ok_or_else(|| LoadError::UnknownStructureKey(dto.key.clone()))?;

    let def = defs.structures.get(def_id).unwrap();

    Ok(RegionStructure {
        def_id,
        kind: def.kind,
        level: dto.level as u32,
        wear: 0.0,
        efficiency: dto.efficiency,
        current_workforce: Vec::new(),
        wage_per_worker: dto.wage_per_worker,
        budget_weight: dto.budget_weight,
        required_budget: 0.0,
        allocated_budget: 0.0,
        period_wage_cost: 0.0,
        period_maintenance_cost: 0.0,
    })
}

fn parse_country_tag(tag_str: &str) -> Result<CountryTag, LoadError> {
    CountryTag::try_from(tag_str).map_err(|_| LoadError::InvalidCountryTag(tag_str.to_owned()))
}

fn parse_game_date(date_str: &str) -> Result<GameDate, LoadError> {
    let mut parts = date_str.split('.');
    let year = parts.next().and_then(|s| s.parse().ok());
    let month = parts.next().and_then(|s| s.parse().ok());
    let day = parts.next().and_then(|s| s.parse().ok());

    let hour = match parts.next() {
        Some(s) => s.parse().ok(),
        None => Some(0),
    };

    match (year, month, day, hour, parts.next()) {
        (Some(y), Some(m), Some(d), Some(h), None) => {
            GameDate::new(y, m, d, h).ok_or_else(|| LoadError::InvalidDate(date_str.to_owned()))
        }
        _ => Err(LoadError::InvalidDate(date_str.to_owned())),
    }
}

fn initialize_baseline_gdp(world: &mut World) {
    for country in &mut world.countries {
        if country.economy.nominal_gdp > 0.0 {
            continue;
        }

        let mut daily_added_value = 0.0;
        for facility in &world.facilities {
            if facility.country_tag != country.tag {
                continue;
            }

            for line in &facility.lines {
                let Some(recipe) = world.defs.recipes.get(line.recipe_id) else {
                    continue;
                };
                let cap = line.installed_capacity as f64;

                let output_val: f64 = recipe
                    .outputs
                    .iter()
                    .map(|&(res_id, amt)| {
                        let price = country.market.get(res_id).map(|m| m.price).unwrap_or(10.0);
                        amt * cap * price
                    })
                    .sum();

                let input_val: f64 = recipe
                    .inputs
                    .iter()
                    .map(|&(res_id, amt)| {
                        let price = country.market.get(res_id).map(|m| m.price).unwrap_or(10.0);
                        amt * cap * price
                    })
                    .sum();

                daily_added_value += (output_val - input_val).max(0.0);
            }
        }

        let mut monthly_structures = 0.0;
        for region in &world.regions {
            if region.owner == Some(country.tag) {
                for s in &region.structures {
                    monthly_structures += s.required_budget;
                }
            }
        }

        let monthly_gdp = daily_added_value * 30.0 + monthly_structures;
        country.economy.nominal_gdp = (monthly_gdp * 12.0).max(10_000.0);
    }
}

fn initialize_digital_platforms(world: &mut World) {
    let recipe_software_id = world.defs.recipes.get_id("recipe_software_development");
    let recipe_cloud_id = world.defs.recipes.get_id("recipe_cloud_hosting");
    let recipe_platforms_id = world.defs.recipes.get_id("recipe_digital_platforms");

    let mut next_id = 0u32;
    let mut initial_platforms = Vec::new();

    for fac in &world.facilities {
        if fac.sector_kind != SectorKind::InformationTechnology {
            continue;
        }

        let comp_name = world
            .companies
            .iter()
            .find(|c| c.id == fac.company_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Tech Enterprise".to_string());

        let ex_rate = world
            .country(fac.country_tag)
            .map(|c| c.economy.exchange_rate.max(0.0001))
            .unwrap_or(1.0);

        for line in &fac.lines {
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

            let name = format!("{comp_name} {suffix}");
            let max_subscribers = (line.installed_capacity as u64) * 1200;
            let monthly_fee = (fee_base * ex_rate).round().max(1.0);

            let platform = DigitalPlatform::new(
                DigitalPlatformId(next_id),
                fac.company_id,
                fac.id,
                fac.region_id,
                fac.country_tag,
                name,
                kind,
                max_subscribers,
                monthly_fee,
            );
            next_id += 1;
            initial_platforms.push(platform);
        }
    }

    for p in initial_platforms {
        world.add_digital_platform(p);
    }
}
