use crate::system::{System, SystemContext};
use crate::world::map::RegionModifier;
use crate::world::structure::StructureKind;
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct LogisticsSystem {
    province_capacities: Vec<f64>,
    facility_loads: Vec<f64>,
}

impl System for LogisticsSystem {
    fn id(&self) -> &'static str {
        "core:logistics_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let max_region_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);

        let required_len = max_region_id + 1;
        if self.province_capacities.len() < required_len {
            self.province_capacities.resize(required_len, 0.0);
        } else {
            self.province_capacities.fill(0.0);
        }

        if self.facility_loads.len() < required_len {
            self.facility_loads.resize(required_len, 0.0);
        } else {
            self.facility_loads.fill(0.0);
        }

        for province in ctx.provinces.iter() {
            let reg_idx = province.region_id.0 as usize;
            if reg_idx < self.province_capacities.len() {
                self.province_capacities[reg_idx] +=
                    province.terrain.base_infrastructure_capacity();
            }
        }

        for fac in ctx.facilities.iter() {
            let reg_idx = fac.region_id.0 as usize;
            if reg_idx < self.facility_loads.len() {
                let weight = fac.sector_kind.infrastructure_weight();
                let capacity_sum: u32 = fac.lines.iter().map(|l| l.installed_capacity).sum();
                self.facility_loads[reg_idx] += capacity_sum as f64 * weight;
            }
        }

        let resource_count = ctx.defs.resources.len();

        for region in ctx.regions.iter_mut() {
            region.reset_local_market_tracking(resource_count);

            let reg_idx = region.id.0 as usize;
            let terrain_base = if reg_idx < self.province_capacities.len() {
                let cap = self.province_capacities[reg_idx];
                if cap > 0.0 { cap } else { 50.0 }
            } else {
                50.0
            };

            let mut transport_bonus = 0.0;
            for struct_state in &region.structures {
                if struct_state.kind == StructureKind::Infrastructure {
                    let health = (1.0 - struct_state.wear).clamp(0.0, 1.0);
                    let effective_level =
                        struct_state.level as f64 * struct_state.efficiency * health;
                    transport_bonus += effective_level * 25.0;
                }
            }

            let infra_mod = region.modifiers.get(RegionModifier::InfrastructureCapacity);
            let capacity_multiplier = (1.0 + infra_mod).max(0.1);

            let total_capacity =
                (terrain_base + region.infrastructure_level * 10.0 + transport_bonus)
                    * capacity_multiplier;
            region.infrastructure_capacity = total_capacity.max(10.0);

            let pop_load = region.population() as f64 * 0.00005;
            let fac_load = if reg_idx < self.facility_loads.len() {
                self.facility_loads[reg_idx]
            } else {
                0.0
            };

            let total_usage = pop_load + fac_load;
            region.infrastructure_usage = total_usage;

            let raw_market_access = if total_usage <= 0.01 {
                1.0
            } else {
                (region.infrastructure_capacity / total_usage).clamp(0.05, 1.0)
            };

            let access_mod = region.modifiers.get(RegionModifier::MarketAccess);
            region.market_access = (raw_market_access + access_mod).clamp(0.05, 1.0);
        }
    }
}
