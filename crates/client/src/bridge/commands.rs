use engine_core::world::CountryTag;
use engine_core::world::country::WelfarePolicy;

#[derive(Debug, Clone)]
pub enum SimulationSpeed {
    Paused,
    Speed1x,
    Speed2x,
    Speed5x,
    MaxSpeed,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ClientCommand {
    ChangeSpeed(SimulationSpeed),
    StepOnce,
    InspectRegion(Option<u16>),
    CommissionFacilityConstruction {
        country: CountryTag,
        region_id: u16,
        recipe_id: u16,
    },
    UpgradeInfrastructure {
        country: CountryTag,
        region_id: u16,
    },
    StartResearch {
        country: CountryTag,
        tech_id: u16,
    },
    SetTaxRates {
        country: CountryTag,
        corporate_tax: f64,
        income_tax: f64,
        tariff_rate: f64,
    },
    SetWelfarePolicy {
        country: CountryTag,
        policy: WelfarePolicy,
    },
    IssueEmergencyWelfare {
        country: CountryTag,
        amount: f64,
    },
    TakeNationalLoan {
        country: CountryTag,
        amount: f64,
    },
    RepayNationalDebt {
        country: CountryTag,
        amount: f64,
    },
}
