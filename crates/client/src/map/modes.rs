use crate::bridge::snapshot::WorldSnapshot;
use crate::map::country_names::CountryNamesDef;

const NON_SCENARIO_COLOR: [f32; 4] = [0.22, 0.24, 0.26, 0.85];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapMode {
    #[default]
    Political,
    Population,
    Infrastructure,
    MarketPrices,
    Unemployment,
    InfrastructureStress,
}

impl MapMode {
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Population,
            2 => Self::Infrastructure,
            3 => Self::MarketPrices,
            4 => Self::Unemployment,
            5 => Self::InfrastructureStress,
            _ => Self::Political,
        }
    }

    pub fn calculate_province_color(
        self,
        province_id: u16,
        country_tag: &str,
        snapshot: &WorldSnapshot,
        country_names: &CountryNamesDef,
        is_selected_province: bool,
        is_selected_country: bool,
    ) -> [f32; 4] {
        let province = snapshot.provinces.get(&province_id);
        let region_id = province.map(|p| p.region_id);
        let region = region_id.and_then(|rid| snapshot.regions.get(&rid));

        let base_color = match self {
            Self::Political => {
                let owner = region
                    .and_then(|r| r.owner.as_deref())
                    .or_else(|| province.and_then(|p| p.controller.as_deref()))
                    .unwrap_or(country_tag);

                if !owner.is_empty() && snapshot.countries.contains_key(owner) {
                    country_names.resolve_color(owner, province_id as usize)
                } else {
                    NON_SCENARIO_COLOR
                }
            }
            Self::Population => {
                if let Some(r) = region {
                    let pop = r.total_population;
                    let normalized = ((pop as f64) / 15_000_000.0).clamp(0.0, 1.0) as f32;
                    lerp_color([0.15, 0.25, 0.45, 1.0], [0.95, 0.45, 0.12, 1.0], normalized)
                } else {
                    NON_SCENARIO_COLOR
                }
            }
            Self::Infrastructure => {
                if let Some(r) = region {
                    let access = r.market_access as f32;
                    lerp_color(
                        [0.65, 0.15, 0.15, 1.0],
                        [0.15, 0.75, 0.35, 1.0],
                        access.clamp(0.0, 1.0),
                    )
                } else {
                    NON_SCENARIO_COLOR
                }
            }
            Self::MarketPrices => {
                if let Some(r) = region {
                    let price = r.average_price;
                    let normalized = ((price - 20.0) / 80.0).clamp(0.0, 1.0) as f32;
                    lerp_color([0.15, 0.65, 0.28, 1.0], [0.85, 0.18, 0.18, 1.0], normalized)
                } else {
                    NON_SCENARIO_COLOR
                }
            }
            Self::Unemployment => {
                if let Some(r) = region {
                    let emp_rate = (r.employed_population as f64
                        / r.total_population.max(1) as f64)
                        .clamp(0.0, 1.0);
                    let unemp_rate = (1.0 - emp_rate) as f32;
                    let normalized = (unemp_rate / 0.25).clamp(0.0, 1.0);
                    lerp_color([0.10, 0.65, 0.35, 1.0], [0.85, 0.15, 0.20, 1.0], normalized)
                } else {
                    NON_SCENARIO_COLOR
                }
            }
            Self::InfrastructureStress => {
                if let Some(r) = region {
                    let load = (r.facility_count as f64 * 35.0 / r.infrastructure_capacity.max(1.0))
                        .clamp(0.0, 1.0) as f32;
                    lerp_color([0.10, 0.50, 0.70, 1.0], [0.90, 0.25, 0.10, 1.0], load)
                } else {
                    NON_SCENARIO_COLOR
                }
            }
        };

        if is_selected_country {
            [
                (base_color[0] * 1.30 + 0.12).min(1.0),
                (base_color[1] * 1.30 + 0.12).min(1.0),
                (base_color[2] * 1.30 + 0.12).min(1.0),
                1.0,
            ]
        } else if is_selected_province {
            [
                (base_color[0] * 1.15 + 0.05).min(1.0),
                (base_color[1] * 1.15 + 0.05).min(1.0),
                (base_color[2] * 1.15 + 0.05).min(1.0),
                1.0,
            ]
        } else {
            base_color
        }
    }
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
