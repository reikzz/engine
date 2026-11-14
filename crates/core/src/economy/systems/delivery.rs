use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct DeliverySystem;

impl System for DeliverySystem {
    fn id(&self) -> &'static str {
        "core:delivery_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let current_date = *ctx.date;

        for facility in ctx.facilities.iter_mut() {
            if facility.pending_deliveries.is_empty() {
                continue;
            }

            let mut arrived = Vec::new();
            facility.pending_deliveries.retain(|delivery| {
                if delivery.arrival_date <= current_date {
                    arrived.push((delivery.resource_id, delivery.amount));
                    false
                } else {
                    true
                }
            });

            for (res_id, amount) in arrived {
                facility.stockpile.add(res_id, amount);
            }
        }
    }
}
