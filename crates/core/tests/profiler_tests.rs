use engine_core::loader::DataLoader;
use engine_core::profile::SystemProfiler;
use std::path::PathBuf;
use std::time::Duration;

fn find_data_dir() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap();
    loop {
        let candidate = dir.join("data");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            panic!("Could not find data directory");
        }
    }
}

#[test]
fn test_system_profiler_metrics_calculation() {
    let mut profiler = SystemProfiler::new();
    profiler.register_system("SystemA");
    profiler.register_system("SystemB");

    profiler.record_advance(Duration::from_micros(50));
    profiler.record_pipeline_tick();
    profiler.record_system_run(0, "SystemA", Duration::from_millis(10));
    profiler.record_system_skip(1, "SystemB");
    profiler.record_commands(Duration::from_micros(100));

    profiler.record_advance(Duration::from_micros(50));
    profiler.record_pipeline_tick();
    profiler.record_system_run(0, "SystemA", Duration::from_millis(20));
    profiler.record_system_run(1, "SystemB", Duration::from_millis(10));

    assert_eq!(profiler.total_ticks(), 2);
    assert_eq!(profiler.pipeline_ticks(), 2);
    assert_eq!(profiler.total_systems_duration(), Duration::from_millis(40));

    let report = profiler.build_report(Duration::from_millis(50));
    assert_eq!(report.items.len(), 2);

    let system_a = report.items.iter().find(|i| i.id == "SystemA").unwrap();
    assert_eq!(system_a.invocations, 2);
    assert_eq!(system_a.evaluations, 2);
    assert_eq!(system_a.total_duration, Duration::from_millis(30));
    assert_eq!(system_a.min_duration, Duration::from_millis(10));
    assert_eq!(system_a.max_duration, Duration::from_millis(20));
    assert_eq!(system_a.avg_duration, Duration::from_millis(15));
    assert!((system_a.percentage_of_systems - 75.0).abs() < 1e-4);
    assert!((system_a.percentage_of_total - 60.0).abs() < 1e-4);

    let system_b = report.items.iter().find(|i| i.id == "SystemB").unwrap();
    assert_eq!(system_b.invocations, 1);
    assert_eq!(system_b.evaluations, 2);
    assert_eq!(system_b.total_duration, Duration::from_millis(10));

    assert_eq!(report.top_bottlenecks(1)[0].id, "SystemA");
    assert_eq!(report.accounted_duration(), Duration::from_micros(40_200));
    assert_eq!(report.overhead_duration(), Duration::from_micros(9_800));
}

#[test]
fn test_world_tick_profiled_execution() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let mut profiler = world.create_profiler();

    for _ in 0..48 {
        world.tick_profiled(&mut profiler);
    }

    assert_eq!(profiler.total_ticks(), 48);
    assert_eq!(profiler.pipeline_ticks(), 2);

    let report = profiler.build_report(Duration::from_secs(1));
    assert!(!report.items.is_empty());

    let has_production = report
        .items
        .iter()
        .any(|i| i.id == "core:production_system");
    assert!(has_production);

    let bottlenecks = report.top_bottlenecks(3);
    assert!(!bottlenecks.is_empty());
}

#[test]
fn test_world_tick_day_profiled() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let mut profiler = world.create_profiler();

    for _ in 0..5 {
        world.tick_day_profiled(&mut profiler);
    }

    assert_eq!(profiler.total_ticks(), 5);
    assert_eq!(profiler.pipeline_ticks(), 5);

    let report = profiler.build_report(Duration::from_secs(1));
    for item in &report.items {
        if item.invocations > 0 {
            assert!(item.avg_duration <= item.max_duration);
            assert!(item.min_duration <= item.avg_duration);
        }
    }
}
