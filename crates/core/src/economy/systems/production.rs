use crate::economy::{FacilityState, ProductionLineState, Recipe, Resource, ResourceId, Stockpile};
use crate::registry::Registry;
use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct ProductionSystem {
    region_productivity: Vec<f64>,
}

const MAX_OUTPUT_BUFFER_DAYS: f64 = 60.0;

impl System for ProductionSystem {
    fn id(&self) -> &'static str {
        "core:production_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let recipes = &ctx.defs.recipes;

        let max_reg_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);
        if self.region_productivity.len() <= max_reg_id {
            self.region_productivity.resize(max_reg_id + 1, 0.0);
        }
        self.region_productivity.fill(0.0);
        for region in ctx.regions.iter() {
            let prod = region
                .modifiers
                .get(crate::world::map::RegionModifier::LaborProductivity);
            self.region_productivity[region.id.0 as usize] = prod;
        }

        for facility in ctx.facilities.iter_mut() {
            let r_id = facility.region_id.0 as usize;
            let prod_bonus = if r_id < self.region_productivity.len() {
                self.region_productivity[r_id]
            } else {
                0.0
            };
            let FacilityState {
                lines, stockpile, ..
            } = facility;

            for line in lines.iter_mut() {
                if let Some(recipe) = recipes.get(line.recipe_id) {
                    Self::process_line(line, recipe, stockpile, &ctx.defs.resources, prod_bonus);
                }
            }
        }
    }
}

impl ProductionSystem {
    fn process_line(
        line: &mut ProductionLineState,
        recipe: &Recipe,
        stock: &mut Stockpile,
        resources: &Registry<ResourceId, Resource>,
        productivity_bonus: f64,
    ) {
        line.last_day_output = 0.0;
        line.input_shortage.clear();

        if line.installed_capacity == 0 {
            line.utilization = 0.0;
            return;
        }

        let capacity = line.installed_capacity as f64;

        let mut workforce_ratio = 1.0_f64;

        for &(pop_type_id, req_per_cap) in &recipe.workforce_requirements {
            let demanded = (req_per_cap as f64) * capacity;
            if demanded > 0.0 {
                let employed = line.get_employed(pop_type_id) as f64;
                let ratio = employed / demanded;
                workforce_ratio = workforce_ratio.min(ratio);
            }
        }

        let workforce_ratio = workforce_ratio.clamp(0.0, 1.0);

        if workforce_ratio <= 0.0 {
            line.utilization = 0.0;
            return;
        }

        let mut input_bottleneck_ratio = 1.0_f64;

        if recipe.inputs.is_empty() {
            input_bottleneck_ratio = 1.0
        } else {
            for &(res_id, req_per_cap) in &recipe.inputs {
                let demanded = req_per_cap * capacity;
                if demanded > 0.0 {
                    let available = stock.get(res_id);
                    let ratio = available / demanded;
                    input_bottleneck_ratio = input_bottleneck_ratio.min(ratio);
                }
            }
        }

        let mut output_storage_ratio = 1.0_f64;
        for &(res_id, prod_per_cap) in &recipe.outputs {
            let is_storable = resources.get(res_id).map(|r| r.is_storable).unwrap_or(true);
            if !is_storable {
                continue;
            }

            let daily_target_output = prod_per_cap * capacity;
            if daily_target_output > 0.0 {
                let max_stock = daily_target_output * MAX_OUTPUT_BUFFER_DAYS;
                let current_stock = stock.get(res_id);

                if current_stock >= max_stock {
                    output_storage_ratio = 0.0;
                    break;
                } else {
                    let available_space = max_stock - current_stock;
                    let space_ratio = available_space / daily_target_output;
                    output_storage_ratio = output_storage_ratio.min(space_ratio);
                }
            }
        }

        let material_bottleneck = input_bottleneck_ratio
            .min(output_storage_ratio)
            .clamp(0.0, 1.0);
        line.material_utilization = material_bottleneck;

        let potential_utilization = input_bottleneck_ratio
            .min(output_storage_ratio)
            .clamp(0.0, 1.0);

        line.potential_utilization = potential_utilization;

        let utilization = input_bottleneck_ratio
            .min(workforce_ratio)
            .min(output_storage_ratio)
            .clamp(0.0, 1.0);

        line.utilization = utilization;

        if line.last_day_output <= 0.0 {
            line.consecutive_idle_days += 1;
        } else {
            line.consecutive_idle_days = 0;
        }

        let effective_capacity = capacity * utilization;
        let output_capacity = effective_capacity * (1.0 + productivity_bonus.clamp(-0.5, 2.0));

        for &(res_id, req_per_cap) in &recipe.inputs {
            let demanded = req_per_cap * capacity;
            let consumed = req_per_cap * effective_capacity;

            if consumed > 0.0 {
                stock.try_consume(res_id, consumed);
            }

            let available = stock.get(res_id);
            if available < demanded {
                line.input_shortage.push((res_id, demanded - available));
            }
        }

        let mut total_output = 0.0;
        for &(res_id, prod_per_cap) in &recipe.outputs {
            let produced = prod_per_cap * output_capacity;
            if produced > 0.0 {
                stock.add(res_id, produced);
                total_output += produced;
            }
        }

        line.last_day_output = total_output;
    }
}
