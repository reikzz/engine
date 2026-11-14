use engine_core::loader::DataLoader;
use engine_core::world::CountryTag;
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
fn test_long_term_dynamic_wage_and_macro_evolution() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(data_dir.join("scenarios/2020_default"), defs).unwrap();

    let rus_tag = CountryTag::new("RUS");
    let initial_rus_line_wages: Vec<f64> = world
        .facilities
        .iter()
        .filter(|f| f.country_tag == rus_tag)
        .flat_map(|f| f.lines.iter().map(|l| l.target_wage))
        .collect();

    assert!(!initial_rus_line_wages.is_empty());

    for _ in 0..8640 {
        world.tick();
    }

    let rus_country = world
        .countries
        .iter()
        .find(|c| c.tag == rus_tag)
        .expect("RUS country must exist");
    assert!(rus_country.economy.treasury > 0.0);
    assert!(!rus_country.economy.treasury.is_nan());

    let final_rus_line_wages: Vec<f64> = world
        .facilities
        .iter()
        .filter(|f| f.country_tag == rus_tag)
        .flat_map(|f| f.lines.iter().map(|l| l.target_wage))
        .collect();

    let mut has_wage_growth = false;
    for (init, current) in initial_rus_line_wages
        .iter()
        .zip(final_rus_line_wages.iter())
    {
        assert!(!current.is_nan());
        assert!(*current >= 11.2 - 1e-5);
        if *current > *init + 1e-4 {
            has_wage_growth = true;
        }
    }

    assert!(
        has_wage_growth,
        "Expected at least some profitable production lines in RUS to grow target wages"
    );

    for region in &world.regions {
        if region.owner == Some(rus_tag) {
            for pop in &region.pops {
                assert!(!pop.employed.savings.is_nan());
                assert!(pop.employed.savings >= 0.0);
                assert!(!pop.unemployed.savings.is_nan());
                assert!(pop.unemployed.savings >= 0.0);
            }
            for structure in &region.structures {
                assert!(!structure.wage_per_worker.is_nan());
                assert!(structure.wage_per_worker > 0.0);
            }
        }
    }
}

#[test]
fn test_russia_wages_in_2013() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(data_dir.join("scenarios/2020_default"), defs).unwrap();

    let rus_tag = CountryTag::new("RUS");

    let total_ticks = 13 * 365 * 24;
    for _ in 0..total_ticks {
        world.tick();
    }

    let mut line_wages: Vec<f64> = Vec::new();
    for facility in &world.facilities {
        if facility.country_tag == rus_tag {
            for line in &facility.lines {
                if line.installed_capacity > 0 {
                    line_wages.push(line.target_wage);
                }
            }
        }
    }

    line_wages.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg_daily_line = line_wages.iter().sum::<f64>() / (line_wages.len().max(1) as f64);
    let avg_monthly_line = avg_daily_line * 30.0;
    let min_daily_line = line_wages.first().copied().unwrap_or(0.0);
    let max_daily_line = line_wages.last().copied().unwrap_or(0.0);

    let mut pop_wages: Vec<f64> = Vec::new();
    for region in &world.regions {
        if region.owner == Some(rus_tag) {
            for pop in &region.pops {
                if pop.employed.count > 0 && pop.average_wage > 0.0 {
                    pop_wages.push(pop.average_wage);
                }
            }
        }
    }
    pop_wages.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg_pop_daily = pop_wages.iter().sum::<f64>() / (pop_wages.len().max(1) as f64);
    let avg_pop_monthly = avg_pop_daily * 30.0;
    let min_pop_daily = pop_wages.first().copied().unwrap_or(0.0);
    let max_pop_daily = pop_wages.last().copied().unwrap_or(0.0);
    let max_pop_monthly = max_pop_daily * 30.0;

    assert!(avg_monthly_line > 2000.0);
    assert!(avg_pop_monthly > 0.0);
    assert!(max_pop_monthly > 10000.0);
    assert!(!min_pop_daily.is_nan());
    assert!(!avg_pop_daily.is_nan());
    assert!(!min_daily_line.is_nan());
    assert!(!max_daily_line.is_nan());
}

#[test]
fn test_countries_gdp_evolution_2000_to_2013() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(data_dir.join("scenarios/2020_default"), defs).unwrap();

    let mut initial_stats = Vec::new();
    for c in &world.countries {
        let ex = c.economy.exchange_rate.max(0.001);
        let pop_count: u64 = world
            .regions
            .iter()
            .filter(|r| r.owner == Some(c.tag))
            .flat_map(|r| r.pops.iter().map(|p| p.count() as u64))
            .sum();
        initial_stats.push((
            c.tag,
            c.economy.nominal_gdp,
            c.economy.nominal_gdp / ex,
            pop_count,
        ));
    }

    let total_ticks = 13 * 365 * 24;
    for _ in 0..total_ticks {
        world.tick();
    }

    for (tag, init_local, init_usd, init_pop) in initial_stats {
        let country = world
            .countries
            .iter()
            .find(|c| c.tag == tag)
            .expect("Country exists");
        let ex = country.economy.exchange_rate.max(0.001);
        let final_local = country.economy.nominal_gdp;
        let final_usd = final_local / ex;
        let final_pop: u64 = world
            .regions
            .iter()
            .filter(|r| r.owner == Some(tag))
            .flat_map(|r| r.pops.iter().map(|p| p.count() as u64))
            .sum();

        let init_capita_usd = init_usd / (init_pop.max(1) as f64);
        let final_capita_usd = final_usd / (final_pop.max(1) as f64);
        let init_capita_local = init_local / (init_pop.max(1) as f64);
        let final_capita_local = final_local / (final_pop.max(1) as f64);

        let capita_growth =
            ((final_capita_usd - init_capita_usd) / init_capita_usd.max(0.001)) * 100.0;

        println!(
            "PER_CAPITA [{}]: 2000: ${:.1} ({:.1} local, pop: {}) -> 2013: ${:.1} ({:.1} local, pop: {}) | Growth: {:+.1}%",
            tag.as_str(),
            init_capita_usd,
            init_capita_local,
            init_pop,
            final_capita_usd,
            final_capita_local,
            final_pop,
            capita_growth
        );
        assert!(!final_local.is_nan());
        assert!(final_local > 0.0);
    }
}
