use crate::Country;
use crate::defs::GameDefs;
use crate::economy::finance::BankLoan;
use crate::economy::{
    CompanyState, DigitalPlatform, FacilityState, GlobalMarket, RealEstateProperty,
};
use crate::world::WorldCommand;
use crate::world::map::{Province, Region, TransportNetwork};
use crate::world::time::{GameDate, TimeTick};
use std::collections::HashSet;
use std::sync::Arc;

pub struct SystemContext<'a> {
    pub tick: &'a TimeTick,
    pub date: &'a GameDate,
    pub defs: &'a Arc<GameDefs>,
    pub countries: &'a mut [Country],
    pub regions: &'a mut [Region],
    pub provinces: &'a mut [Province],
    pub facilities: &'a mut [FacilityState],
    pub companies: &'a mut [CompanyState],
    pub real_estate: &'a mut [RealEstateProperty],
    pub digital_platforms: &'a mut [DigitalPlatform],
    pub global_market: &'a mut GlobalMarket,
    pub loans: &'a mut Vec<BankLoan>,
    pub transport_network: &'a TransportNetwork,

    pub commands: &'a mut Vec<WorldCommand>,
}

pub trait System: Send + Sync {
    fn id(&self) -> &'static str;

    fn run_after(&self) -> &'static [&'static str] {
        &[]
    }

    fn should_run(&self, tick: &TimeTick) -> bool;

    fn update(&mut self, ctx: &mut SystemContext);
}

pub struct SystemPipeline {
    systems: Vec<Box<dyn System>>,
    disabled_systems: HashSet<&'static str>,
}

impl SystemPipeline {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            disabled_systems: HashSet::new(),
        }
    }

    pub fn systems(&self) -> &[Box<dyn System>] {
        &self.systems
    }

    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn set_enabled(&mut self, id: &'static str, enabled: bool) {
        if enabled {
            self.disabled_systems.remove(id);
        } else {
            self.disabled_systems.insert(id);
        }
    }

    pub fn is_enabled(&self, id: &'static str) -> bool {
        !self.disabled_systems.contains(id)
    }

    pub fn create_profiler(&self) -> crate::profile::SystemProfiler {
        let mut profiler = crate::profile::SystemProfiler::new();
        for system in &self.systems {
            profiler.register_system(system.id());
        }
        profiler
    }

    pub fn run_tick(&mut self, ctx: &mut SystemContext) {
        for system in &mut self.systems {
            if self.disabled_systems.contains(system.id()) {
                continue;
            }

            if system.should_run(ctx.tick) {
                system.update(ctx);
            }
        }
    }

    pub fn run_tick_profiled(
        &mut self,
        ctx: &mut SystemContext,
        profiler: &mut crate::profile::SystemProfiler,
    ) {
        profiler.record_pipeline_tick();
        for (index, system) in self.systems.iter_mut().enumerate() {
            if self.disabled_systems.contains(system.id()) {
                continue;
            }

            if system.should_run(ctx.tick) {
                let start = std::time::Instant::now();
                system.update(ctx);
                let elapsed = start.elapsed();
                profiler.record_system_run(index, system.id(), elapsed);
            } else {
                profiler.record_system_skip(index, system.id());
            }
        }
    }
}
