use crate::world::map::RegionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportConnectionType {
    Road,
    Rail,
    Sea,
    Pipeline,
}

impl TransportConnectionType {
    #[inline]
    pub fn speed_km_per_day(self) -> f64 {
        match self {
            Self::Road => 500.0,
            Self::Rail => 800.0,
            Self::Sea => 600.0,
            Self::Pipeline => 1500.0,
        }
    }

    #[inline]
    pub fn fuel_consumption_factor(self) -> f64 {
        match self {
            Self::Pipeline => 0.05,
            Self::Sea => 0.15,
            Self::Rail => 0.25,
            Self::Road => 0.50,
        }
    }

    #[inline]
    pub fn is_pipeline(self) -> bool {
        matches!(self, Self::Pipeline)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportEdge {
    pub from: RegionId,
    pub to: RegionId,
    pub connection_type: TransportConnectionType,
    pub distance_km: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RouteMetrics {
    pub distance_km: f64,
    pub lead_time_days: u32,
    pub fuel_cost_factor: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportNetwork {
    pub edges: Vec<TransportEdge>,
    distance_matrix: Vec<Vec<Option<RouteMetrics>>>,
    pipeline_matrix: Vec<Vec<Option<RouteMetrics>>>,
}

impl TransportNetwork {
    pub fn new(edges: Vec<TransportEdge>, max_region_id: usize) -> Self {
        let mut network = Self {
            edges,
            distance_matrix: Vec::new(),
            pipeline_matrix: Vec::new(),
        };
        network.build_distance_matrices(max_region_id);
        network
    }

    pub fn build_distance_matrices(&mut self, max_region_id: usize) {
        let size = max_region_id + 1;
        self.distance_matrix = vec![vec![None; size]; size];
        self.pipeline_matrix = vec![vec![None; size]; size];

        for i in 0..size {
            let zero_route = RouteMetrics {
                distance_km: 0.0,
                lead_time_days: 0,
                fuel_cost_factor: 0.0,
            };
            self.distance_matrix[i][i] = Some(zero_route);
            self.pipeline_matrix[i][i] = Some(zero_route);
        }

        for edge in &self.edges {
            let u = edge.from.0 as usize;
            let v = edge.to.0 as usize;
            if u >= size || v >= size {
                continue;
            }

            let days = (edge.distance_km / edge.connection_type.speed_km_per_day()).ceil() as u32;
            let lead_time = match edge.connection_type {
                TransportConnectionType::Sea => days.max(1) + 1,
                _ => days.max(1),
            };

            let metrics = RouteMetrics {
                distance_km: edge.distance_km,
                lead_time_days: lead_time,
                fuel_cost_factor: edge.connection_type.fuel_consumption_factor(),
            };

            Self::update_edge_if_shorter(&mut self.distance_matrix[u][v], metrics);
            Self::update_edge_if_shorter(&mut self.distance_matrix[v][u], metrics);

            if edge.connection_type.is_pipeline() {
                Self::update_edge_if_shorter(&mut self.pipeline_matrix[u][v], metrics);
                Self::update_edge_if_shorter(&mut self.pipeline_matrix[v][u], metrics);
            }
        }

        Self::run_floyd_warshall(&mut self.distance_matrix, size);
        Self::run_floyd_warshall(&mut self.pipeline_matrix, size);
    }

    fn update_edge_if_shorter(entry: &mut Option<RouteMetrics>, new_metrics: RouteMetrics) {
        if let Some(existing) = entry {
            if new_metrics.distance_km < existing.distance_km {
                *existing = new_metrics;
            }
        } else {
            *entry = Some(new_metrics);
        }
    }

    fn run_floyd_warshall(matrix: &mut [Vec<Option<RouteMetrics>>], size: usize) {
        for k in 0..size {
            for i in 0..size {
                let Some(route_ik) = matrix[i][k] else {
                    continue;
                };
                for j in 0..size {
                    let Some(route_kj) = matrix[k][j] else {
                        continue;
                    };

                    let new_distance = route_ik.distance_km + route_kj.distance_km;
                    let new_days = route_ik.lead_time_days + route_kj.lead_time_days;
                    let new_fuel = route_ik.fuel_cost_factor.max(route_kj.fuel_cost_factor);

                    let new_route = RouteMetrics {
                        distance_km: new_distance,
                        lead_time_days: new_days,
                        fuel_cost_factor: new_fuel,
                    };

                    if let Some(existing) = &mut matrix[i][j] {
                        if new_distance < existing.distance_km {
                            *existing = new_route;
                        }
                    } else {
                        matrix[i][j] = Some(new_route);
                    }
                }
            }
        }
    }

    pub fn find_route(&self, from: RegionId, to: RegionId, is_hydrocarbon: bool) -> RouteMetrics {
        if from == to {
            return RouteMetrics {
                distance_km: 0.0,
                lead_time_days: 0,
                fuel_cost_factor: 0.0,
            };
        }

        let u = from.0 as usize;
        let v = to.0 as usize;

        if is_hydrocarbon {
            if let Some(row) = self.pipeline_matrix.get(u) {
                if let Some(Some(route)) = row.get(v) {
                    return *route;
                }
            }
        }

        if let Some(row) = self.distance_matrix.get(u) {
            if let Some(Some(route)) = row.get(v) {
                return *route;
            }
        }

        let estimated_distance = (u as f64 - v as f64).abs() * 80.0 + 300.0;
        let estimated_days = (estimated_distance / 600.0).ceil() as u32;

        RouteMetrics {
            distance_km: estimated_distance,
            lead_time_days: estimated_days.max(1),
            fuel_cost_factor: 0.40,
        }
    }
}
