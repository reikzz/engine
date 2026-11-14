mod actions;
mod construction;
pub mod country;
mod inspection;
pub mod map;
mod pops;
mod structure;
mod systems;
pub mod tech;
pub mod time;

use crate::defs::GameDefs;
use crate::economy::finance::BankLoan;
use crate::economy::{
    CompanyId, CompanyState, FacilityId, FacilityState, GlobalMarket, Ownership, SectorKind,
};
pub use crate::economy::{
    DigitalPlatform, DigitalPlatformId, DigitalPlatformKind, RealEstateId, RealEstateKind,
    RealEstateProperty,
};
use crate::system::{SystemContext, SystemPipeline};
pub use crate::world::construction::{
    ConstructionMaterialEstimate, ConstructionProject, ConstructionTarget,
    FacilityConstructionEstimate, StructureUpgradeEstimate,
};
pub use crate::world::inspection::{
    RegionConstructionInspection, RegionDetailedInspection, RegionFacilityInspection,
    RegionFacilityLineInspection, RegionPopInspection, RegionStructureInspection,
};
pub use crate::world::map::{Province, Region, RegionId, TransportNetwork};
pub use crate::world::pops::{
    NeedCategory, Pop, PopNeed, PopSubgroup, PopType, PopTypeDef, PopTypeId,
};
pub use crate::world::structure::{
    RawStructureDef, RegionStructure, ResolvedStructureDef, StateStructure, StructureDef,
    StructureDefId, StructureKind,
};
pub use crate::world::tech::{
    RawTechnologyDef, TechnologyDef, TechnologyId, TechnologyOverviewItem, TechnologyResearchStatus,
};
pub use crate::world::time::GameDate;
use crate::world::time::TimeTick;
pub use country::{BailoutPolicy, Country, CountryEconomy, CountryTag, WelfarePolicy};
use std::sync::Arc;

pub struct World {
    pub defs: Arc<GameDefs>,

    pub date: GameDate,
    pub countries: Vec<Country>,
    pub regions: Vec<Region>,
    pub provinces: Vec<Province>,
    pub facilities: Vec<FacilityState>,
    pub companies: Vec<CompanyState>,
    pub real_estate: Vec<RealEstateProperty>,
    pub digital_platforms: Vec<DigitalPlatform>,
    pub global_market: GlobalMarket,
    pub loans: Vec<BankLoan>,
    pub transport_network: TransportNetwork,
    pub pipeline: SystemPipeline,
}

pub enum WorldCommand {
    SpawnFacility(FacilityState),
    SpawnCompany(CompanyState),
    SpawnRealEstate(RealEstateProperty),
    SpawnDigitalPlatform(DigitalPlatform),
    CloseFacility(FacilityId),
}

impl World {
    pub fn new(game_defs: GameDefs, start_date: GameDate) -> Self {
        let mut pipeline = SystemPipeline::new();

        Self::register_core_systems(&mut pipeline);

        let global_market = GlobalMarket::from_registry(&game_defs.resources);

        Self {
            defs: Arc::new(game_defs),
            countries: Vec::with_capacity(256),
            date: start_date,
            regions: Vec::new(),
            provinces: Vec::new(),
            facilities: Vec::new(),
            companies: Vec::new(),
            real_estate: Vec::new(),
            digital_platforms: Vec::new(),
            global_market,
            loans: Vec::new(),
            transport_network: TransportNetwork::default(),
            pipeline,
        }
    }

    fn register_core_systems(pipeline: &mut SystemPipeline) {
        pipeline.add_system(systems::LogisticsSystem::default());
        crate::economy::systems::register_systems(pipeline);
        systems::register_systems(pipeline);
    }

    pub fn add_country(&mut self, country: Country) {
        self.countries.push(country);
    }

    pub fn country(&self, tag: CountryTag) -> Option<&Country> {
        self.countries.iter().find(|c| c.tag == tag)
    }

    pub fn country_mut(&mut self, tag: CountryTag) -> Option<&mut Country> {
        self.countries.iter_mut().find(|c| c.tag == tag)
    }

    pub fn region(&self, id: RegionId) -> Option<&Region> {
        self.regions.iter().find(|r| r.id == id)
    }

    pub fn region_mut(&mut self, id: RegionId) -> Option<&mut Region> {
        self.regions.iter_mut().find(|r| r.id == id)
    }

    pub fn add_facility(&mut self, mut facility: FacilityState) -> FacilityId {
        let id = FacilityId(self.facilities.len() as u32);
        facility.id = id;
        if let Some(region) = self.regions.iter_mut().find(|r| r.id == facility.region_id) {
            if !region.facilities.contains(&id) {
                region.facilities.push(id);
            }
        }
        self.facilities.push(facility);
        id
    }

    pub fn add_real_estate(&mut self, mut property: RealEstateProperty) -> RealEstateId {
        let id = RealEstateId(self.real_estate.len() as u32);
        property.id = id;
        self.real_estate.push(property);
        id
    }

    pub fn add_digital_platform(&mut self, mut platform: DigitalPlatform) -> DigitalPlatformId {
        let id = DigitalPlatformId(self.digital_platforms.len() as u32);
        platform.id = id;
        self.digital_platforms.push(platform);
        id
    }

    pub fn add_company(&mut self, company: CompanyState) {
        self.companies.push(company);
    }

    pub fn found_state_company(
        &mut self,
        country_tag: CountryTag,
        name: String,
        sector: SectorKind,
        initial_capital: f64,
    ) -> Result<CompanyId, String> {
        let country = self
            .countries
            .iter_mut()
            .find(|c| c.tag == country_tag)
            .ok_or_else(|| format!("Country {:?} not found", country_tag))?;

        if country.economy.treasury < initial_capital {
            return Err(format!(
                "There are not enough funds in the treasury: available {:.1}, required {:.1}.",
                country.economy.treasury, initial_capital
            ));
        }

        country.economy.treasury -= initial_capital;

        let next_id = self
            .companies
            .iter()
            .map(|c| c.id.0)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        let comp_id = CompanyId(next_id);
        let company = CompanyState {
            id: comp_id,
            name,
            country_tag,
            ownership: Ownership::State,
            sector_kind: sector,
            balance: initial_capital,
            reinvestment_ratio: 0.5,
            monthly_revenue: 0.0,
            monthly_expenses: 0.0,
            last_month_profit: 0.0,
        };
        self.companies.push(company);
        Ok(comp_id)
    }

    pub fn create_profiler(&self) -> crate::profile::SystemProfiler {
        self.pipeline.create_profiler()
    }

    pub fn tick(&mut self) -> TimeTick {
        let time_tick = self.date.advance();

        if !time_tick.is_new_day() {
            return time_tick;
        }

        self.run_pipeline_for_tick(time_tick)
    }

    pub fn tick_day(&mut self) -> TimeTick {
        let time_tick = self.date.advance_day();
        self.run_pipeline_for_tick(time_tick)
    }

    pub fn tick_profiled(&mut self, profiler: &mut crate::profile::SystemProfiler) -> TimeTick {
        let start = std::time::Instant::now();
        let time_tick = self.date.advance();
        profiler.record_advance(start.elapsed());

        if !time_tick.is_new_day() {
            return time_tick;
        }

        self.run_pipeline_for_tick_profiled(time_tick, profiler)
    }

    pub fn tick_day_profiled(&mut self, profiler: &mut crate::profile::SystemProfiler) -> TimeTick {
        let start = std::time::Instant::now();
        let time_tick = self.date.advance_day();
        profiler.record_advance(start.elapsed());

        self.run_pipeline_for_tick_profiled(time_tick, profiler)
    }

    fn run_pipeline_for_tick(&mut self, time_tick: TimeTick) -> TimeTick {
        let mut commands = Vec::new();

        {
            let mut ctx = SystemContext {
                tick: &time_tick,
                date: &self.date,
                defs: &self.defs,
                countries: &mut self.countries,
                regions: &mut self.regions,
                provinces: &mut self.provinces,
                facilities: &mut self.facilities,
                companies: &mut self.companies,
                real_estate: &mut self.real_estate,
                digital_platforms: &mut self.digital_platforms,
                global_market: &mut self.global_market,
                loans: &mut self.loans,
                transport_network: &self.transport_network,
                commands: &mut commands,
            };

            self.pipeline.run_tick(&mut ctx);
        }

        self.apply_commands(commands);
        time_tick
    }

    fn run_pipeline_for_tick_profiled(
        &mut self,
        time_tick: TimeTick,
        profiler: &mut crate::profile::SystemProfiler,
    ) -> TimeTick {
        let mut commands = Vec::new();

        {
            let mut ctx = SystemContext {
                tick: &time_tick,
                date: &self.date,
                defs: &self.defs,
                countries: &mut self.countries,
                regions: &mut self.regions,
                provinces: &mut self.provinces,
                facilities: &mut self.facilities,
                companies: &mut self.companies,
                real_estate: &mut self.real_estate,
                digital_platforms: &mut self.digital_platforms,
                global_market: &mut self.global_market,
                loans: &mut self.loans,
                transport_network: &self.transport_network,
                commands: &mut commands,
            };

            self.pipeline.run_tick_profiled(&mut ctx, profiler);
        }

        let cmd_start = std::time::Instant::now();
        self.apply_commands(commands);
        profiler.record_commands(cmd_start.elapsed());

        time_tick
    }

    fn apply_commands(&mut self, commands: Vec<WorldCommand>) {
        for cmd in commands {
            match cmd {
                WorldCommand::SpawnFacility(f) => {
                    self.add_facility(f);
                }
                WorldCommand::SpawnCompany(c) => {
                    self.add_company(c);
                }
                WorldCommand::SpawnRealEstate(re) => {
                    self.add_real_estate(re);
                }
                WorldCommand::SpawnDigitalPlatform(dp) => {
                    self.add_digital_platform(dp);
                }
                WorldCommand::CloseFacility(fac_id) => {
                    self.close_facility(fac_id);
                }
            }
        }
    }

    pub fn close_facility(&mut self, facility_id: FacilityId) -> bool {
        let Some(fac_pos) = self.facilities.iter().position(|f| f.id == facility_id) else {
            return false;
        };

        let facility = self.facilities.remove(fac_pos);

        if let Some(region) = self.regions.iter_mut().find(|r| r.id == facility.region_id) {
            region.facilities.retain(|&id| id != facility_id);

            for line in &facility.lines {
                for &(pop_type_id, count) in &line.employed_workers {
                    if count > 0 {
                        if let Some(pop) = region
                            .pops
                            .iter_mut()
                            .find(|p| p.pop_type_id == pop_type_id)
                        {
                            pop.fire(count as u64);
                        }
                    }
                }
            }

            for (res_id, amt) in facility.stockpile.non_empty() {
                if amt > 0.0 {
                    region.stockpile.add(res_id, amt);
                }
            }
        }

        if facility.local_reserve > 0.0 {
            if let Some(comp) = self
                .companies
                .iter_mut()
                .find(|c| c.id == facility.company_id)
            {
                comp.balance += facility.local_reserve;
            }
        }

        self.digital_platforms
            .retain(|dp| dp.facility_id != facility_id);

        true
    }
}
