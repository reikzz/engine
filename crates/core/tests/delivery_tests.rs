use engine_core::economy::{PendingDelivery, ResourceId};
use engine_core::loader::DataLoader;
use engine_core::world::map::{RegionId, TransportConnectionType, TransportEdge, TransportNetwork};
use std::path::{Path, PathBuf};

fn find_data_dir() -> PathBuf {
    let candidates = [
        Path::new("data"),
        Path::new("../../data"),
        Path::new("../data"),
    ];

    for candidate in candidates {
        if candidate.join("common").exists() {
            return candidate.to_path_buf();
        }
    }
    panic!("Could not find 'data' directory in candidate paths");
}

#[test]
fn test_transport_network_routing_and_pipelines() {
    let edges = vec![
        TransportEdge {
            from: RegionId(0),
            to: RegionId(1),
            connection_type: TransportConnectionType::Road,
            distance_km: 1000.0,
        },
        TransportEdge {
            from: RegionId(1),
            to: RegionId(2),
            connection_type: TransportConnectionType::Pipeline,
            distance_km: 1500.0,
        },
        TransportEdge {
            from: RegionId(0),
            to: RegionId(2),
            connection_type: TransportConnectionType::Rail,
            distance_km: 1600.0,
        },
    ];

    let network = TransportNetwork::new(edges, 2);

    let rail_route = network.find_route(RegionId(0), RegionId(2), false);
    assert_eq!(rail_route.distance_km, 1600.0);
    assert_eq!(rail_route.lead_time_days, 2);
    assert_eq!(rail_route.fuel_cost_factor, 0.25);

    let pipeline_route = network.find_route(RegionId(1), RegionId(2), true);
    assert_eq!(pipeline_route.distance_km, 1500.0);
    assert_eq!(pipeline_route.lead_time_days, 1);
    assert_eq!(pipeline_route.fuel_cost_factor, 0.05);
}

#[test]
fn test_goods_in_transit_delayed_arrival() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let test_res_id = world
        .defs
        .resources
        .get_id("gold")
        .expect("Gold resource must exist");
    let initial_stock = world.facilities[0].stockpile.get(test_res_id);
    let arrival_date = world.date.add_days(2);
    let delivery_amount = 500.0;

    world.facilities[0]
        .pending_deliveries
        .push(PendingDelivery {
            resource_id: test_res_id,
            amount: delivery_amount,
            arrival_date,
        });

    for _ in 0..24 {
        world.tick();
    }

    assert_eq!(
        world.facilities[0].pending_deliveries.len(),
        1,
        "Delivery should still be in transit after 1 day"
    );
    assert_eq!(
        world.facilities[0].stockpile.get(test_res_id),
        initial_stock,
        "Stockpile should not increase while in transit"
    );

    for _ in 0..24 {
        world.tick();
    }

    assert_eq!(
        world.facilities[0].pending_deliveries.len(),
        0,
        "Delivery should have arrived after 2 days"
    );
    assert!(
        world.facilities[0].stockpile.get(test_res_id) >= initial_stock + delivery_amount,
        "Stockpile must receive the delivered goods"
    );
}

#[test]
fn test_pending_amount_calculation_and_protection() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let mut facility = world.facilities[0].clone();
    let res_a = ResourceId(0);
    let res_b = ResourceId(1);

    facility.pending_deliveries.push(PendingDelivery {
        resource_id: res_a,
        amount: 250.0,
        arrival_date: world.date.add_days(3),
    });
    facility.pending_deliveries.push(PendingDelivery {
        resource_id: res_a,
        amount: 150.0,
        arrival_date: world.date.add_days(4),
    });
    facility.pending_deliveries.push(PendingDelivery {
        resource_id: res_b,
        amount: 80.0,
        arrival_date: world.date.add_days(2),
    });

    assert_eq!(facility.pending_amount(res_a), 400.0);
    assert_eq!(facility.pending_amount(res_b), 80.0);
    assert_eq!(facility.pending_amount(ResourceId(99)), 0.0);
}

#[test]
fn test_neighbor_adjacency_and_auto_network_generation() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios").join("2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load defs");
    let world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    assert!(world.regions.len() >= 181);

    for region in &world.regions {
        assert!(
            !region.neighbors.is_empty(),
            "Region {:?} has no neighbors connected!",
            region.id
        );
        for &neighbor_id in &region.neighbors {
            let neighbor_region = world
                .region(neighbor_id)
                .unwrap_or_else(|| panic!("Neighbor region {:?} not found", neighbor_id));
            assert!(
                neighbor_region.is_neighbor(region.id),
                "Asymmetric neighbor connection between {:?} and {:?}",
                region.id,
                neighbor_id
            );
        }
    }

    let route_internal = world
        .transport_network
        .find_route(RegionId(12), RegionId(23), false);
    assert!(route_internal.distance_km > 0.0);
    assert!(route_internal.distance_km < 5000.0);

    let route_intercontinental =
        world
            .transport_network
            .find_route(RegionId(0), RegionId(12), false);
    assert!(route_intercontinental.distance_km > 1000.0);
    assert!(route_intercontinental.lead_time_days >= 3);
}
