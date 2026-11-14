mod profile;

use engine_core::loader::DataLoader;
use profile::{ProfileOptions, run_profiler};
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }

    let result = if args.iter().any(|a| a == "--profile" || a == "-p") {
        let options = parse_profile_options(&args);
        run_profiler(options)
    } else if args.iter().any(|a| a == "--sim") {
        let years = args
            .iter()
            .skip_while(|&a| a != "--sim")
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(50);
        run_interactive_simulation(years)
    } else if args.iter().any(|a| a == "--all") {
        let _ = run_interactive_simulation(5);
        run_benchmark()
    } else {
        run_benchmark()
    };

    if let Err(err) = result {
        eprintln!("\n[Error] {err}");

        let mut source = err.source();
        while let Some(cause) = source {
            eprintln!("  Caused by: {cause}");
            source = cause.source();
        }

        std::process::exit(1);
    }
}

fn parse_profile_options(args: &[String]) -> ProfileOptions {
    let mut options = ProfileOptions::default();

    if let Some(pos) = args.iter().position(|a| a == "--ticks") {
        if let Some(val) = args.get(pos + 1).and_then(|s| s.parse::<u64>().ok()) {
            options.ticks = Some(val);
            options.days = None;
            options.years = None;
            return options;
        }
    }

    if let Some(pos) = args.iter().position(|a| a == "--days") {
        if let Some(val) = args.get(pos + 1).and_then(|s| s.parse::<u64>().ok()) {
            options.days = Some(val);
            options.ticks = None;
            options.years = None;
            return options;
        }
    }

    if let Some(pos) = args.iter().position(|a| a == "--years") {
        if let Some(val) = args.get(pos + 1).and_then(|s| s.parse::<u64>().ok()) {
            options.years = Some(val);
            return options;
        }
    }

    let positional_val = args
        .iter()
        .skip_while(|&a| a != "--profile" && a != "-p")
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok());

    if let Some(val) = positional_val {
        options.years = Some(val);
    }

    options
}

fn print_usage() {
    println!("Использование симуляционного приложения Engine:");
    println!("  cli                             Запуск бенчмарка производительности (10-50 лет)");
    println!(
        "  cli --profile [years]           Профилирование систем симуляции (по умолчанию 1 год)"
    );
    println!("  cli --profile --days <N>        Профилирование на N дней");
    println!("  cli --profile --ticks <N>       Профилирование на N тиков (часов)");
    println!("  cli --sim [years]               Интерактивная симуляция с помесячным выводом");
    println!("  cli --all                       Запуск интерактивного режима и бенчмарка");
    println!("  cli --help, -h                  Вывод этой справки");
}

fn run_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    let defs_path = Path::new("data/common");
    let scenario_path = Path::new("data/scenarios/2020_default");

    let initial_defs = DataLoader::load_game_defs(defs_path)?;
    let initial_world = DataLoader::load_world(scenario_path, initial_defs)?;

    println!("================================================================================");
    println!("        БЕНЧМАРК ПРОИЗВОДИТЕЛЬНОСТИ СИМУЛЯЦИОННОГО ДВИЖКА (ZERO I/O)           ");
    println!("================================================================================");
    println!(
        "Сценарий: 2020_default ({} стран: USA, RUS, CHN, DEU, {} регионов, {} провинций, {} заводов, {} компаний)",
        initial_world.countries.len(),
        initial_world.regions.len(),
        initial_world.provinces.len(),
        initial_world.facilities.len(),
        initial_world.companies.len(),
    );
    println!("Включены: производство, зарплаты, рынки, налоги, глобальная торговля, демография\n");

    let benchmarks = [
        ("10 лет (86 400 часов)", 86_400u64),
        ("20 лет (172 800 часов)", 172_800u64),
        ("30 лет (259 200 часов)", 259_200u64),
        ("40 лет (345 600 часов)", 345_600u64),
        ("50 лет (432 000 часов)", 432_000u64),
    ];

    for &(title, target_ticks) in &benchmarks {
        let defs = DataLoader::load_game_defs(defs_path)?;
        let mut world = DataLoader::load_world(scenario_path, defs)?;

        let start = Instant::now();
        for _ in 0..target_ticks {
            world.tick();
        }
        let elapsed = start.elapsed();

        let tps = (target_ticks as f64) / elapsed.as_secs_f64().max(1e-9);
        let sim_days = (target_ticks as f64) / 24.0;
        let sim_years = sim_days / 360.0;
        let sim_years_per_sec = sim_years / elapsed.as_secs_f64().max(1e-9);

        let total_pops: u64 = world
            .regions
            .iter()
            .flat_map(|r| &r.pops)
            .map(|p| p.count())
            .sum();
        let employed_pops: u64 = world
            .regions
            .iter()
            .flat_map(|r| &r.pops)
            .map(|p| p.employed.count)
            .sum();
        let total_companies = world.companies.len();
        let total_facilities = world.facilities.len();

        println!(
            "┌── [Тест: {} | {} тиков] ──────────────────────────────────────────",
            title, target_ticks
        );
        println!("│ Время выполнения:  {:.3?}", elapsed);
        println!(
            "│ Скорость тиков:    {:.0} тиков/сек ({:.2} мкс/тик)",
            tps,
            (elapsed.as_micros() as f64) / (target_ticks as f64)
        );
        if sim_years >= 0.5 {
            println!(
                "│ Скорость симуляции: {:.1} игровых лет в секунду ({:.1} игровых дней/сек)",
                sim_years_per_sec,
                sim_days / elapsed.as_secs_f64().max(1e-9)
            );
        } else {
            println!(
                "│ Скорость симуляции: {:.1} игровых дней в секунду",
                sim_days / elapsed.as_secs_f64().max(1e-9)
            );
        }
        println!(
            "│ Население в конце:  {} чел. (занято: {}, {:.1}%)",
            total_pops,
            employed_pops,
            (employed_pops as f64 / total_pops.max(1) as f64) * 100.0
        );
        let treasuries: Vec<String> = world
            .countries
            .iter()
            .map(|c| {
                format!(
                    "{}: {:.0}{}",
                    c.tag.as_str(),
                    c.economy.treasury,
                    c.economy.currency_symbol
                )
            })
            .collect();
        println!("│ Казна стран:       {}", treasuries.join(" | "));
        println!(
            "│ Объектов в мире:   {} заводов, {} компаний",
            total_facilities, total_companies
        );
        println!("│ Дата в симуляции:  {}", world.date);
        println!(
            "└─────────────────────────────────────────────────────────────────────────────\n"
        );
    }

    println!(
        "Подсказка: запустите симуляцию с флагом `--sim` для интерактивного логирования каждого месяца."
    );
    Ok(())
}

fn run_interactive_simulation(years: u64) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    let defs_path = Path::new("data/common");
    let scenario_path = Path::new("data/scenarios/2020_default");

    let defs = DataLoader::load_game_defs(defs_path)?;
    let mut world = DataLoader::load_world(scenario_path, defs)?;

    let target_ticks = years * 360 * 24;

    println!(
        "=== Старт симуляции на {} лет ({} стран, {} регионов, {} заводов, {} компаний) ===",
        years,
        world.countries.len(),
        world.regions.len(),
        world.facilities.len(),
        world.companies.len()
    );

    let mut month_counter = 0;

    for tick_num in 1..=target_ticks {
        let time_tick = world.tick();

        let is_first_day = tick_num == 24;
        let is_monthly = time_tick.is_new_month();

        if is_first_day || is_monthly {
            if is_monthly {
                month_counter += 1;
            }

            let should_print = month_counter % 24 == 0 || tick_num == target_ticks;

            if !should_print {
                continue;
            }

            println!(
                "\n┌── [Тик {:>4} | {} | Месяц {:>2}] ──────────────────────────────────────",
                tick_num, world.date, month_counter
            );

            for c in &world.countries {
                let symbol = &c.economy.currency_symbol;
                let mut market_entries: Vec<String> = Vec::new();

                for (res_id, res_def) in world.defs.resources.iter() {
                    let Some(rm) = c.market.get(res_id) else {
                        continue;
                    };
                    if rm.last_demanded <= 0.05 && rm.last_supplied <= 0.05 {
                        continue;
                    }

                    let deficit_flag = if rm.buy_fulfillment < 0.9 && rm.last_demanded > 0.1 {
                        "!"
                    } else {
                        ""
                    };

                    market_entries.push(format!(
                        "{}{}: {:.1}{} ({:.0}/{:.0} | {:.0}%)",
                        deficit_flag,
                        res_def.key,
                        rm.price,
                        symbol,
                        rm.last_demanded,
                        rm.last_supplied,
                        rm.buy_fulfillment * 100.0
                    ));
                }

                if !market_entries.is_empty() {
                    println!(
                        "│ Рынок [{}] ({}): {}",
                        c.tag.as_str(),
                        c.economy.currency_code,
                        market_entries.join(" | ")
                    );
                }
            }

            let mut global_entries: Vec<String> = Vec::new();
            for (res_id, quote) in world.global_market.iter() {
                if quote.daily_volume > 0.05 {
                    let name = world
                        .defs
                        .resources
                        .get(res_id)
                        .map(|r| r.key.as_str())
                        .unwrap_or("?");
                    global_entries.push(format!(
                        "{name}: ${:.1} (объём {:.0})",
                        quote.benchmark_price, quote.daily_volume
                    ));
                }
            }
            if !global_entries.is_empty() {
                println!("│ Мировой рынок ($): {}", global_entries.join(" | "));
            }

            for facility in &world.facilities {
                let stock_str: Vec<String> = facility
                    .stockpile
                    .non_empty()
                    .map(|(res_id, amount)| {
                        let name = world
                            .defs
                            .resources
                            .get(res_id)
                            .map(|r| r.key.as_str())
                            .unwrap_or("?");
                        format!("{name}:{amount:.0}")
                    })
                    .collect();

                let lines_str: Vec<String> = facility
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let mut line_desc = format!("L{i}: {:.0}%", l.utilization * 100.0);

                        if l.utilization < 0.05 && l.installed_capacity > 0 {
                            let recipe = world.defs.recipes.get(l.recipe_id);
                            let mut missing_labor = Vec::new();
                            if let Some(r) = recipe {
                                for &(pop_type, req) in &r.workforce_requirements {
                                    let needed =
                                        ((req as f64) * l.installed_capacity as f64).ceil() as u32;
                                    let actual = l.get_employed(pop_type);
                                    if actual < needed {
                                        let name = world
                                            .defs
                                            .pop_types
                                            .get(pop_type)
                                            .map(|p| p.key.as_str())
                                            .unwrap_or("?");
                                        missing_labor.push(format!("{name}:{actual}/{needed}"));
                                    }
                                }
                            }

                            if !missing_labor.is_empty() {
                                line_desc
                                    .push_str(&format!(" [NO_LABOR: {}]", missing_labor.join(",")));
                            } else if !l.input_shortage.is_empty() {
                                line_desc.push_str(" [NO_INPUT]");
                            } else if l.potential_utilization < 0.05 {
                                line_desc.push_str(" [STOCK_FULL]");
                            }
                        } else if l.last_day_output > 0.0 {
                            line_desc.push_str(&format!(" (out:{:.1})", l.last_day_output));
                        }

                        line_desc
                    })
                    .collect();

                let stock_display = if stock_str.is_empty() {
                    "—".to_string()
                } else {
                    stock_str.join(",")
                };

                println!(
                    "│ [Завод #{:<2} Co:{:<2} ({})] Склад: [{}] | {}",
                    facility.id.0,
                    facility.company_id.0,
                    facility.country_tag.as_str(),
                    stock_display,
                    lines_str.join(" | ")
                );
            }

            let comp_str: Vec<String> = world
                .companies
                .iter()
                .map(|comp| {
                    let symbol = world
                        .countries
                        .iter()
                        .find(|c| c.tag == comp.country_tag)
                        .map(|c| c.economy.currency_symbol.as_str())
                        .unwrap_or("$");
                    format!(
                        "{}(#{}): {:.0}{}",
                        comp.name, comp.id.0, comp.balance, symbol
                    )
                })
                .collect();
            println!("│ Балансы: {}", comp_str.join(" | "));

            if !world.digital_platforms.is_empty() {
                let platforms_str: Vec<String> = world
                    .digital_platforms
                    .iter()
                    .filter(|p| {
                        p.is_active && (p.active_subscribers > 0 || p.monthly_revenue > 0.0)
                    })
                    .map(|p| {
                        let sym = world
                            .countries
                            .iter()
                            .find(|c| c.tag == p.country_tag)
                            .map(|c| c.economy.currency_symbol.as_str())
                            .unwrap_or("$");
                        format!(
                            "{}: {} subs | MRR {:.0}{} (Q:{:.1})",
                            p.name, p.active_subscribers, p.monthly_revenue, sym, p.quality
                        )
                    })
                    .collect();
                if !platforms_str.is_empty() {
                    println!("│ Цифровые платформы (SaaS): {}", platforms_str.join(" | "));
                }
            }

            let gdp_str: Vec<String> = world
                .countries
                .iter()
                .map(|c| {
                    let sym = &c.economy.currency_symbol;
                    let pop: u64 = world
                        .regions
                        .iter()
                        .filter(|r| r.owner == Some(c.tag))
                        .flat_map(|r| &r.pops)
                        .map(|p| p.count())
                        .sum();
                    let per_capita = if pop > 0 {
                        c.economy.nominal_gdp / pop as f64
                    } else {
                        0.0
                    };
                    format!(
                        "{}: ВВП {:.1}B{} (на душу: {:.0}{})",
                        c.tag.as_str(),
                        c.economy.nominal_gdp / 1_000_000_000.0,
                        sym,
                        per_capita,
                        sym
                    )
                })
                .collect();
            println!("│ ВВП (GDP): {}", gdp_str.join(" | "));

            let budget_str: Vec<String> = world
                .countries
                .iter()
                .map(|c| {
                    let sym = &c.economy.currency_symbol;
                    format!(
                        "{}: Казна {:.0}{} (налоги +{:.0}{}, пошлины +{:.0}{})",
                        c.tag.as_str(),
                        c.economy.treasury,
                        sym,
                        c.economy.last_period_tax_revenue,
                        sym,
                        c.economy.last_period_tariff_revenue,
                        sym
                    )
                })
                .collect();
            println!("│ Бюджеты: {}", budget_str.join(" | "));

            let mut total_pops = 0u64;
            let mut employed_pops = 0u64;
            let mut reg_summaries = Vec::new();

            for r in &world.regions {
                let mut reg_pops = 0u64;
                let mut reg_emp = 0u64;
                for p in &r.pops {
                    reg_pops += p.count();
                    reg_emp += p.employed.count;
                    total_pops += p.count();
                    employed_pops += p.employed.count;
                }
                let owner_str = r.owner.as_ref().map(|t| t.as_str()).unwrap_or("?");
                reg_summaries.push(format!(
                    "Регион #{}({}): {} (занято {})",
                    r.id.0, owner_str, reg_pops, reg_emp
                ));
            }
            let emp_pct = if total_pops > 0 {
                (employed_pops as f64 / total_pops as f64) * 100.0
            } else {
                0.0
            };

            let avg_sat = if total_pops > 0 {
                world
                    .regions
                    .iter()
                    .flat_map(|r| &r.pops)
                    .map(|p| p.satisfaction as f64 * p.count() as f64)
                    .sum::<f64>()
                    / total_pops as f64
            } else {
                1.0
            };

            println!(
                "│ Население: {}/{} ({:.1}%) | Счастье: {:.0}%",
                employed_pops,
                total_pops,
                emp_pct,
                avg_sat * 100.0
            );
            println!("│ Распределение: {}", reg_summaries.join(" | "));

            println!("└──────────────────────────────────────────────────────────");
        }
    }

    println!(
        "\n=== Интерактивная симуляция завершена за {:.2?} ===",
        start_time.elapsed()
    );
    Ok(())
}
