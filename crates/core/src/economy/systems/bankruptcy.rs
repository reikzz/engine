use crate::economy::{Ownership, SectorKind};
use crate::system::{System, SystemContext};
use crate::world::WorldCommand;
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct BankruptcySystem {
    company_funds: Vec<f64>,
    company_profits: Vec<f64>,
    company_ownership: Vec<Option<Ownership>>,
}

impl System for BankruptcySystem {
    fn id(&self) -> &'static str {
        "core:bankruptcy_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let max_comp_id = ctx
            .companies
            .iter()
            .map(|c| c.id.0 as usize)
            .max()
            .unwrap_or(0);

        self.company_funds.clear();
        self.company_funds.resize(max_comp_id + 1, 0.0);
        self.company_profits.clear();
        self.company_profits.resize(max_comp_id + 1, 0.0);
        self.company_ownership.clear();
        self.company_ownership.resize_with(max_comp_id + 1, || None);

        for comp in ctx.companies.iter() {
            let idx = comp.id.0 as usize;
            self.company_funds[idx] = comp.balance;
            self.company_profits[idx] = comp.last_month_profit;
            self.company_ownership[idx] = Some(comp.ownership.clone());
        }

        for facility in ctx.facilities.iter() {
            if facility.sector_kind == SectorKind::Financial {
                continue;
            }

            let comp_id = facility.company_id.0 as usize;
            let comp_balance = if comp_id < self.company_funds.len() {
                self.company_funds[comp_id]
            } else {
                0.0
            };
            let comp_profit = if comp_id < self.company_profits.len() {
                self.company_profits[comp_id]
            } else {
                0.0
            };
            let is_state = self
                .company_ownership
                .get(comp_id)
                .and_then(|o| o.as_ref())
                .map(|o| matches!(o, Ownership::State))
                .unwrap_or(false);

            let has_installed_capacity = facility.lines.iter().any(|l| l.installed_capacity > 0);

            if !has_installed_capacity {
                ctx.commands.push(WorldCommand::CloseFacility(facility.id));
                continue;
            }

            let idle_threshold = if is_state { 720 } else { 360 };
            let completely_abandoned = facility
                .lines
                .iter()
                .filter(|l| l.installed_capacity > 0)
                .all(|l| l.consecutive_idle_days >= idle_threshold)
                && facility.local_reserve <= 0.0;

            let private_insolvent_idle = !is_state
                && comp_balance <= 0.0
                && comp_profit < 0.0
                && facility.local_reserve <= 0.0
                && facility
                    .lines
                    .iter()
                    .filter(|l| l.installed_capacity > 0)
                    .all(|l| l.consecutive_idle_days >= 180);

            if completely_abandoned || private_insolvent_idle {
                ctx.commands.push(WorldCommand::CloseFacility(facility.id));
            }
        }
    }
}
