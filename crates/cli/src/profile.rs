use engine_core::World;
use engine_core::loader::DataLoader;
use engine_core::profile::SystemProfileReport;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct ProfileOptions {
    pub years: Option<u64>,
    pub days: Option<u64>,
    pub ticks: Option<u64>,
}

impl Default for ProfileOptions {
    fn default() -> Self {
        Self {
            years: Some(1),
            days: None,
            ticks: None,
        }
    }
}

pub fn run_profiler(options: ProfileOptions) -> Result<(), Box<dyn std::error::Error>> {
    let defs_path = Path::new("data/common");
    let scenario_path = Path::new("data/scenarios/2020_default");

    let defs = DataLoader::load_game_defs(defs_path)?;
    let mut world = DataLoader::load_world(scenario_path, defs)?;

    let target_ticks = if let Some(ticks) = options.ticks {
        ticks
    } else if let Some(days) = options.days {
        days.saturating_mul(24)
    } else {
        options.years.unwrap_or(1).saturating_mul(360 * 24)
    };

    let sim_days = (target_ticks as f64) / 24.0;
    let sim_years = sim_days / 360.0;

    println!("================================================================================");
    println!("          ПРОФИЛИРОВАНИЕ СИМУЛЯЦИОННОГО ДВИЖКА (SYSTEM PROFILER)                ");
    println!("================================================================================");
    println!(
        "Сценарий: 2020_default ({} стран, {} регионов, {} провинций, {} заводов, {} компаний)",
        world.countries.len(),
        world.regions.len(),
        world.provinces.len(),
        world.facilities.len(),
        world.companies.len(),
    );
    println!(
        "Целевая нагрузка: {} тиков ({:.1} игровых дней / {:.2} игровых лет)",
        target_ticks, sim_days, sim_years
    );
    println!("Профилирование систем симуляции запущено...\n");

    let mut profiler = world.create_profiler();

    let start_wall = Instant::now();
    for _ in 0..target_ticks {
        world.tick_profiled(&mut profiler);
    }
    let elapsed_wall = start_wall.elapsed();

    let report = profiler.build_report(elapsed_wall);
    print_profile_report(&world, &report, target_ticks, sim_days, sim_years);

    Ok(())
}

fn print_profile_report(
    world: &World,
    report: &SystemProfileReport,
    target_ticks: u64,
    sim_days: f64,
    sim_years: f64,
) {
    let elapsed_secs = report.wall_duration.as_secs_f64().max(1e-9);
    let tps = (target_ticks as f64) / elapsed_secs;
    let days_per_sec = sim_days / elapsed_secs;
    let years_per_sec = sim_years / elapsed_secs;

    println!(
        "┌── [СВОДКА ВЫПОЛНЕНИЯ | {} ТИКОВ] ──────────────────────────────────────────",
        target_ticks
    );
    println!(
        "│ Общее время выполнения: {:>10} ({:.0} тиков/сек)",
        format_duration(report.wall_duration),
        tps
    );
    println!(
        "│ Скорость симуляции:     {:>10.1} дней/сек ({:.2} игровых лет/сек)",
        days_per_sec, years_per_sec
    );
    println!(
        "│ Среднее время тика:     {:>10}",
        format_duration(report.wall_duration / target_ticks.max(1) as u32)
    );
    println!(
        "│ Финальная дата:         {} | Объектов: {} заводов, {} компаний",
        world.date,
        world.facilities.len(),
        world.companies.len()
    );
    println!(
        "├─────────────────────────────────────────────────────────────────────────────────────────────────────────────"
    );
    println!("│ ДЕТАЛЬНЫЙ ОТЧЕТ ПО СИСТЕМАМ ДВИЖКА (УПОРЯДОЧЕНО ПО ВРЕМЕНИ ВЫПОЛНЕНИЯ):");
    println!(
        "├────┬──────────────────────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬─────────┬────────┤"
    );
    println!(
        "│  # │ Система                          │   Вызовы │  Суммарно│   Среднее│       Мин│      Макс│ % Систем│ % Общее│"
    );
    println!(
        "├────┼──────────────────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼─────────┼────────┤"
    );

    for (idx, item) in report.items.iter().enumerate() {
        println!(
            "│ {:>2} │ {:<32} │ {:>8} │ {:>8} │ {:>8} │ {:>8} │ {:>8} │ {:>6.1}% │ {:>5.1}% │",
            idx + 1,
            item.id,
            item.invocations,
            format_duration(item.total_duration),
            format_duration(item.avg_duration),
            format_duration(item.min_duration),
            format_duration(item.max_duration),
            item.percentage_of_systems,
            item.percentage_of_total,
        );
    }

    println!(
        "├────┴──────────────────────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴─────────┴────────┤"
    );
    println!("│ ВИЗУАЛЬНОЕ РАСПРЕДЕЛЕНИЕ ВРЕМЕНИ МЕЖДУ СИСТЕМАМИ (ДОЛЯ В ПАЙПЛАЙНЕ):");

    for (idx, item) in report.items.iter().enumerate() {
        if item.invocations == 0 {
            continue;
        }
        let bar = format_ascii_bar(item.percentage_of_systems, 24);
        println!(
            "│ {:>2}. {:<32} │ {} │ {:>5.1}% │ {:>8} │",
            idx + 1,
            item.id,
            bar,
            item.percentage_of_systems,
            format_duration(item.total_duration)
        );
    }

    println!(
        "├─────────────────────────────────────────────────────────────────────────────────────────────────────────────"
    );
    println!("│ ФАЗЫ ВЫПОЛНЕНИЯ СИМУЛЯЦИИ:");

    let systems_pct = (report.total_systems_duration.as_secs_f64() / elapsed_secs) * 100.0;
    let commands_pct = (report.commands_duration.as_secs_f64() / elapsed_secs) * 100.0;
    let advance_pct = (report.advance_duration.as_secs_f64() / elapsed_secs) * 100.0;
    let overhead_pct = (report.overhead_duration().as_secs_f64() / elapsed_secs) * 100.0;

    println!(
        "│ • Системы симуляции (pipeline): {:>10} ({:>5.1}% от общего времени, {} вызовов пайплайна)",
        format_duration(report.total_systems_duration),
        systems_pct,
        report.pipeline_ticks
    );
    println!(
        "│ • Команды мира (spawn commands):{:>10} ({:>5.1}% от общего времени)",
        format_duration(report.commands_duration),
        commands_pct
    );
    println!(
        "│ • Инкремент времени (date advance):{:>7} ({:>5.1}% от общего времени)",
        format_duration(report.advance_duration),
        advance_pct
    );
    println!(
        "│ • Накладные расходы (overhead): {:>10} ({:>5.1}% от общего времени)",
        format_duration(report.overhead_duration()),
        overhead_pct
    );

    println!(
        "├─────────────────────────────────────────────────────────────────────────────────────────────────────────────"
    );
    println!("│ АНАЛИЗ УЗКИХ МЕСТ (TOP BOTTLENECKS):");

    let top_items = report.top_bottlenecks(3);
    if top_items.is_empty() {
        println!("│ Системы не выполнялись во время профилирования.");
    } else {
        let top_total_pct: f64 = top_items.iter().map(|i| i.percentage_of_systems).sum();
        for (i, item) in top_items.iter().enumerate() {
            println!(
                "│ {}. {:<32} — {:>5.1}% времени систем ({}, в среднем {}/вызов)",
                i + 1,
                item.id,
                item.percentage_of_systems,
                format_duration(item.total_duration),
                format_duration(item.avg_duration)
            );
        }
        println!(
            "│ Топ-{} систем суммарно потребляют {:.1}% всего времени выполнения систем.",
            top_items.len(),
            top_total_pct
        );
    }

    println!(
        "└─────────────────────────────────────────────────────────────────────────────────────────────────────────────\n"
    );
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs_f64();
    if total_secs >= 60.0 {
        let mins = (total_secs / 60.0).floor();
        let rem_secs = total_secs - mins * 60.0;
        format!("{:.0}m {:.1}s", mins, rem_secs)
    } else if total_secs >= 1.0 {
        format!("{:.3}s", total_secs)
    } else if total_secs >= 0.001 {
        format!("{:.2}ms", total_secs * 1_000.0)
    } else if total_secs >= 0.000_001 {
        format!("{:.1}µs", total_secs * 1_000_000.0)
    } else {
        format!("{:.0}ns", total_secs * 1_000_000_000.0)
    }
}

fn format_ascii_bar(percentage: f64, width: usize) -> String {
    let clamped = percentage.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);

    let mut bar = String::with_capacity(width * 3);
    for _ in 0..filled {
        bar.push('█');
    }
    for _ in 0..empty {
        bar.push('░');
    }
    bar
}
