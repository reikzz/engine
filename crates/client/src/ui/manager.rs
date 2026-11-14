use crate::bridge::commands::ClientCommand;
use crate::bridge::snapshot::WorldSnapshot;
use crate::map::camera::Camera2D;
use crate::map::country_names::CountryLabelAnchor;
use crate::map::modes::MapMode;
use crate::render::OverlayPipeline;
use crate::ui::platform::CustomSlintPlatform;
use crossbeam_channel::Sender;
use engine_core::world::CountryTag;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{LogicalPosition, PhysicalSize, SharedString};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

slint::include_modules!();

pub struct UiManager {
    main_window: MainWindow,
    software_window: Rc<MinimalSoftwareWindow>,
    pixel_buffer: Vec<PremultipliedRgbaColor>,
    width: u32,
    height: u32,
    last_camera_pos: glam::Vec2,
    last_camera_zoom: f32,
    last_camera_rot: f32,
    labels_were_empty: bool,
    country_menu_closed: Rc<Cell<bool>>,
    region_panel_closed: Rc<Cell<bool>>,
    active_region_id: Rc<Cell<Option<u16>>>,
    active_country_tag: Rc<RefCell<String>>,
    active_central_bank_rate: Rc<Cell<f64>>,
    last_inspected_region_id: Option<u16>,
    last_region_ticks: u64,
    last_market_modal_open: bool,
    last_market_ticks: u64,
    last_companies_modal_open: bool,
    last_companies_ticks: u64,
    last_construction_modal_open: bool,
    last_construction_ticks: u64,
    last_tech_modal_open: bool,
    last_tech_ticks: u64,
    last_active_country: String,
    last_region_update: Instant,
    last_market_update: Instant,
    last_companies_update: Instant,
    last_tech_update: Instant,
}

impl UiManager {
    pub fn new(
        command_sender: Sender<ClientCommand>,
        mode_sender: Sender<MapMode>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let software_window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        software_window.set_size(PhysicalSize::new(width, height));

        let platform = CustomSlintPlatform::new(software_window.clone());
        slint::platform::set_platform(Box::new(platform))?;

        let main_window = MainWindow::new()?;

        let closed_country_cell = Rc::new(Cell::new(false));
        let closed_country_clone = closed_country_cell.clone();
        main_window.on_country_menu_closed(move || {
            closed_country_clone.set(true);
        });

        let closed_region_cell = Rc::new(Cell::new(false));
        let closed_region_clone = closed_region_cell.clone();
        main_window.on_region_panel_closed(move || {
            closed_region_clone.set(true);
        });

        let active_region_id = Rc::new(Cell::new(None));
        let active_country_tag = Rc::new(RefCell::new("USA".to_string()));
        let active_central_bank_rate = Rc::new(Cell::new(0.05));

        let cmd_tx_rate = command_sender.clone();
        let country_rate = active_country_tag.clone();
        let rate_cell_clone = active_central_bank_rate.clone();
        main_window.on_adjust_central_bank_rate_requested(move |delta| {
            let tag_str = country_rate.borrow().clone();
            if let Ok(country) = CountryTag::try_from(tag_str.as_str()) {
                let cur = rate_cell_clone.get();
                let new_rate = (cur + delta as f64).clamp(0.0, 0.30);
                rate_cell_clone.set(new_rate);
                let _ = cmd_tx_rate.send(ClientCommand::SetCentralBankRate {
                    country,
                    target_rate: new_rate,
                });
            }
        });

        let cmd_tx = command_sender.clone();
        main_window.on_speed_changed(move |speed_val| {
            let speed = match speed_val {
                0 => crate::bridge::commands::SimulationSpeed::Paused,
                1 => crate::bridge::commands::SimulationSpeed::Speed1x,
                2 => crate::bridge::commands::SimulationSpeed::Speed2x,
                5 => crate::bridge::commands::SimulationSpeed::Speed5x,
                _ => crate::bridge::commands::SimulationSpeed::MaxSpeed,
            };
            let _ = cmd_tx.send(ClientCommand::ChangeSpeed(speed));
        });

        let mode_tx = mode_sender;
        main_window.on_map_mode_changed(move |mode_code| {
            let mode = MapMode::from_index(mode_code);
            let _ = mode_tx.send(mode);
        });

        let cmd_tx_comm = command_sender.clone();
        let country_comm = active_country_tag.clone();
        let region_comm = active_region_id.clone();
        main_window.on_commission_construction_requested(move |recipe_id| {
            let tag_str = country_comm.borrow().clone();
            if let Ok(country) = CountryTag::try_from(tag_str.as_str()) {
                if let Some(rid) = region_comm.get() {
                    let _ = cmd_tx_comm.send(ClientCommand::CommissionFacilityConstruction {
                        country,
                        region_id: rid,
                        recipe_id: recipe_id as u16,
                    });
                }
            }
        });

        let cmd_tx_infra = command_sender.clone();
        let country_infra = active_country_tag.clone();
        main_window.on_upgrade_infrastructure_requested(move |region_id| {
            let tag_str = country_infra.borrow().clone();
            if let Ok(country) = CountryTag::try_from(tag_str.as_str()) {
                let _ = cmd_tx_infra.send(ClientCommand::UpgradeInfrastructure {
                    country,
                    region_id: region_id as u16,
                });
            }
        });

        let cmd_tx_tech = command_sender;
        let country_tech = active_country_tag.clone();
        main_window.on_select_research_requested(move |tech_id| {
            let tag_str = country_tech.borrow().clone();
            if let Ok(country) = CountryTag::try_from(tag_str.as_str()) {
                let _ = cmd_tx_tech.send(ClientCommand::StartResearch {
                    country,
                    tech_id: tech_id as u16,
                });
            }
        });

        let pixel_buffer = vec![PremultipliedRgbaColor::default(); (width * height) as usize];
        let past_instant = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);

        Ok(Self {
            main_window,
            software_window,
            pixel_buffer,
            width,
            height,
            last_camera_pos: glam::Vec2::splat(f32::NAN),
            last_camera_zoom: f32::NAN,
            last_camera_rot: f32::NAN,
            labels_were_empty: true,
            country_menu_closed: closed_country_cell,
            region_panel_closed: closed_region_cell,
            active_region_id,
            active_country_tag,
            active_central_bank_rate,
            last_inspected_region_id: None,
            last_region_ticks: u64::MAX,
            last_market_modal_open: false,
            last_market_ticks: u64::MAX,
            last_companies_modal_open: false,
            last_companies_ticks: u64::MAX,
            last_construction_modal_open: false,
            last_construction_ticks: u64::MAX,
            last_tech_modal_open: false,
            last_tech_ticks: u64::MAX,
            last_active_country: String::new(),
            last_region_update: past_instant,
            last_market_update: past_instant,
            last_companies_update: past_instant,
            last_tech_update: past_instant,
        })
    }

    pub fn take_country_menu_closed(&self) -> bool {
        self.country_menu_closed.replace(false)
    }

    pub fn take_region_panel_closed(&self) -> bool {
        self.region_panel_closed.replace(false)
    }

    #[allow(dead_code)]
    pub fn set_region_panel_open(&self, open: bool) {
        self.main_window.set_region_panel_open(open);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.software_window
            .set_size(PhysicalSize::new(width, height));
        self.pixel_buffer
            .resize((width * height) as usize, PremultipliedRgbaColor::default());
        self.last_camera_pos = glam::Vec2::splat(f32::NAN);
    }

    pub fn update_country_labels(&mut self, camera: &Camera2D, anchors: &[CountryLabelAnchor]) {
        let opacity = ((0.70 - camera.zoom) / 0.20).clamp(0.0, 1.0);
        if opacity <= 0.01 || anchors.is_empty() {
            if !self.labels_were_empty {
                let empty: Vec<CountryLabelItem> = Vec::new();
                self.main_window
                    .set_country_labels(std::rc::Rc::new(slint::VecModel::from(empty)).into());
                self.labels_were_empty = true;
            }
            return;
        }

        let pos_diff = (camera.position - self.last_camera_pos).length_squared();
        let zoom_diff = (camera.zoom - self.last_camera_zoom).abs();
        let rot_diff = (camera.rotation - self.last_camera_rot).abs();

        if pos_diff < 0.01 && zoom_diff < 1e-4 && rot_diff < 1e-4 && !self.labels_were_empty {
            return;
        }

        self.last_camera_pos = camera.position;
        self.last_camera_zoom = camera.zoom;
        self.last_camera_rot = camera.rotation;
        self.labels_were_empty = false;

        let mut labels = Vec::new();
        let offsets = [-5400.0f32, 0.0f32, 5400.0f32];

        for anchor in anchors {
            let screen_width = anchor.world_width * camera.zoom;
            let text_char_count = anchor.name.chars().count().max(1) as f32;
            let min_required_width = text_char_count * 7.0;

            if screen_width < min_required_width {
                continue;
            }

            let max_font_for_box =
                (screen_width * 0.75 / (text_char_count * 0.65)).clamp(8.0, 16.0);
            let font_size = (10.0 + (camera.zoom * 10.0)).clamp(8.0, max_font_for_box);
            let box_width = (screen_width * 0.85).clamp(min_required_width, 600.0);

            for &off_x in &offsets {
                let screen = camera
                    .world_to_screen(glam::Vec2::new(anchor.center.x + off_x, anchor.center.y));
                if screen.x >= -box_width
                    && screen.x <= (self.width as f32 + box_width)
                    && screen.y >= 50.0
                    && screen.y <= (self.height as f32 + 50.0)
                {
                    labels.push(CountryLabelItem {
                        name: SharedString::from(anchor.name.as_str()),
                        pos_x: screen.x,
                        pos_y: screen.y,
                        font_size,
                        box_width,
                        opacity,
                        visible: true,
                    });
                    break;
                }
            }
        }

        self.main_window
            .set_country_labels(std::rc::Rc::new(slint::VecModel::from(labels)).into());
    }

    pub fn update_state(
        &mut self,
        snapshot: &WorldSnapshot,
        _selected_province: Option<u16>,
        active_country: &str,
        active_country_name: &str,
        inspected_country: Option<(&str, &str)>,
    ) {
        *self.active_country_tag.borrow_mut() = active_country.to_string();

        self.main_window
            .set_date_text(SharedString::from(&snapshot.date_display));
        self.main_window
            .set_country_name(SharedString::from(active_country_name));

        if let Some(country) = snapshot.countries.get(active_country) {
            self.active_central_bank_rate.set(country.central_bank_rate);
            let treasury_formatted =
                format_compact_currency(&country.currency_symbol, country.treasury);
            let gdp_formatted =
                format_compact_currency(&country.currency_symbol, country.nominal_gdp);

            self.main_window
                .set_treasury_text(SharedString::from(treasury_formatted));
            self.main_window
                .set_gdp_text(SharedString::from(gdp_formatted));

            let (infl_text, infl_color) = format_inflation(country.inflation_rate);
            self.main_window
                .set_inflation_text(SharedString::from(infl_text));
            self.main_window.set_inflation_color(infl_color);
            self.main_window
                .set_central_bank_rate_text(SharedString::from(format!(
                    "{:.2}%",
                    country.central_bank_rate * 100.0
                )));
        }

        if let Some((tag, display_name)) = inspected_country {
            self.main_window.set_country_menu_open(true);
            self.main_window
                .set_inspected_country_tag(SharedString::from(tag));
            self.main_window
                .set_inspected_country_name(SharedString::from(display_name));

            if let Some(country) = snapshot.countries.get(tag) {
                let treasury_formatted =
                    format_compact_currency(&country.currency_symbol, country.treasury);
                let gdp_formatted =
                    format_compact_currency(&country.currency_symbol, country.nominal_gdp);
                let debt_formatted =
                    format_compact_currency(&country.currency_symbol, country.national_debt);
                let pop_formatted = format_compact_number(country.total_population as f64);
                let emp_pct = (country.employed_population as f64
                    / country.total_population.max(1) as f64)
                    * 100.0;
                let emp_formatted = format!(
                    "{} ({:.1}%)",
                    format_compact_number(country.employed_population as f64),
                    emp_pct
                );

                let regions_count = snapshot
                    .regions
                    .values()
                    .filter(|r| r.owner.as_deref() == Some(tag))
                    .count();

                self.main_window
                    .set_inspected_treasury(SharedString::from(treasury_formatted));
                self.main_window
                    .set_inspected_gdp(SharedString::from(gdp_formatted));
                self.main_window
                    .set_inspected_debt(SharedString::from(debt_formatted));
                self.main_window
                    .set_inspected_pop(SharedString::from(pop_formatted));
                self.main_window
                    .set_inspected_employment(SharedString::from(emp_formatted));
                self.main_window
                    .set_inspected_regions(SharedString::from(format!("{}", regions_count)));

                let (inspected_infl_text, inspected_infl_color) =
                    format_inflation(country.inflation_rate);
                let real_rate_val = country.real_interest_rate;
                let real_prefix = if real_rate_val >= 0.0 { "+" } else { "" };
                let real_rate_text = format!("{}{:.2}%", real_prefix, real_rate_val * 100.0);

                self.main_window
                    .set_inspected_cpi(SharedString::from(format!("{:.1}", country.cpi)));
                self.main_window
                    .set_inspected_inflation(SharedString::from(inspected_infl_text));
                self.main_window
                    .set_inspected_inflation_color(inspected_infl_color);
                self.main_window
                    .set_inspected_central_bank_rate(SharedString::from(format!(
                        "{:.2}%",
                        country.central_bank_rate * 100.0
                    )));
                self.main_window
                    .set_inspected_real_rate(SharedString::from(real_rate_text));
            }
        } else {
            self.main_window.set_country_menu_open(false);
        }

        if let Some(ref detail) = snapshot.inspected_region_detail {
            self.active_region_id.set(Some(detail.region_id));
            self.main_window
                .set_region_name(SharedString::from(&detail.name));
            self.main_window.set_region_id(detail.region_id as i32);
            self.main_window
                .set_region_country_tag(SharedString::from(&detail.country_tag));
            self.main_window
                .set_region_terrain(SharedString::from(&detail.terrain));
            self.main_window
                .set_region_pop_text(SharedString::from(format_compact_number(
                    detail.total_pop as f64,
                )));

            let emp_ratio =
                (detail.employed_pop as f64 / (detail.total_pop.max(1) as f64)).clamp(0.0, 1.0);
            self.main_window
                .set_region_emp_text(SharedString::from(format!(
                    "{} ({:.1}%)",
                    format_compact_number(detail.employed_pop as f64),
                    emp_ratio * 100.0
                )));
            self.main_window.set_region_emp_ratio(emp_ratio as f32);
            self.main_window
                .set_region_satisfaction_ratio(detail.satisfaction);
            self.main_window
                .set_region_radicalism_ratio(detail.radicalism);
            self.main_window
                .set_region_infra_level(SharedString::from(format!(
                    "Tier {:.0}",
                    detail.infrastructure_level
                )));
            self.main_window
                .set_region_infra_cap_text(SharedString::from(format!(
                    "{:.0} / {:.0}",
                    detail.infrastructure_load, detail.infrastructure_capacity
                )));
            let util_ratio = (detail.infrastructure_load / detail.infrastructure_capacity.max(1.0))
                .clamp(0.0, 1.0) as f32;
            self.main_window.set_region_infra_util(util_ratio);
            self.main_window
                .set_region_market_access_ratio(detail.market_access as f32);
            self.main_window
                .set_region_market_access_text(SharedString::from(format!(
                    "{:.1}%",
                    detail.market_access * 100.0
                )));
            self.main_window
                .set_region_avg_price_text(SharedString::from(format!(
                    "${:.2}",
                    detail.average_price
                )));

            let region_changed = self.last_inspected_region_id != Some(detail.region_id);
            let time_elapsed = self.last_region_update.elapsed() >= Duration::from_millis(1500);
            if region_changed || time_elapsed {
                let mut facility_items = Vec::new();
                for f in &detail.facilities {
                    facility_items.push(LocalFacilityItem {
                        name: SharedString::from(&f.name),
                        sector: SharedString::from(&f.sector),
                        workforce_text: SharedString::from(format!(
                            "{}/{}",
                            f.workforce_count, f.workforce_capacity
                        )),
                        output_text: SharedString::from(format!("+{:.1}/day", f.daily_output)),
                    });
                }
                self.main_window
                    .set_region_facilities(Rc::new(slint::VecModel::from(facility_items)).into());
                self.last_inspected_region_id = Some(detail.region_id);
                self.last_region_ticks = snapshot.total_ticks;
                self.last_region_update = Instant::now();
            }

            self.main_window.set_region_panel_open(true);
        } else {
            self.active_region_id.set(None);
            self.last_inspected_region_id = None;
            self.main_window.set_region_panel_open(false);
        }

        let construction_modal_open = self.main_window.get_construction_modal_open();
        if construction_modal_open {
            let opened_now = !self.last_construction_modal_open;
            let country_changed = self.last_active_country != active_country;
            if opened_now || country_changed {
                let mut construction_items = Vec::new();
                for opt in &snapshot.construction_options {
                    construction_items.push(ConstructionRecipeItem {
                        id: opt.recipe_id as i32,
                        key: SharedString::from(&opt.key),
                        name: SharedString::from(&opt.name),
                        sector: SharedString::from(&opt.sector),
                        build_time_days: opt.build_time_days as i32,
                        cost_text: SharedString::from(&opt.cost_text),
                        materials_text: SharedString::from(&opt.materials_summary),
                        tech_unlocked: opt.tech_unlocked,
                        required_tech_name: SharedString::from(&opt.required_tech_name),
                    });
                }
                self.main_window.set_construction_recipes(
                    Rc::new(slint::VecModel::from(construction_items)).into(),
                );
                self.last_construction_ticks = snapshot.total_ticks;
            }
        }
        self.last_construction_modal_open = construction_modal_open;

        let tech_modal_open = self.main_window.get_tech_modal_open();
        if tech_modal_open {
            let opened_now = !self.last_tech_modal_open;
            let time_elapsed = self.last_tech_update.elapsed() >= Duration::from_millis(1000);
            if opened_now || time_elapsed {
                let mut tech_model_items = Vec::new();
                for t in &snapshot.tech_tree {
                    tech_model_items.push(TechTreeItem {
                        id: t.id as i32,
                        key: SharedString::from(&t.key),
                        name: SharedString::from(&t.name),
                        category: SharedString::from(&t.category),
                        cost_points: t.cost_points as i32,
                        progress_ratio: t.progress_ratio,
                        status: SharedString::from(&t.status),
                        is_active_research: t.is_active,
                        is_completed: t.is_completed,
                        can_research: t.can_research,
                        unlocks_summary: SharedString::from(&t.unlocks_summary),
                    });
                }
                self.main_window
                    .set_tech_items(Rc::new(slint::VecModel::from(tech_model_items)).into());
                self.last_tech_ticks = snapshot.total_ticks;
                self.last_tech_update = Instant::now();
            }
        }
        self.last_tech_modal_open = tech_modal_open;

        if let Some(ref name) = snapshot.active_tech_name {
            self.main_window
                .set_active_tech_name(SharedString::from(name));
            self.main_window
                .set_active_tech_category(SharedString::from(
                    snapshot
                        .active_tech_category
                        .as_deref()
                        .unwrap_or("Applied"),
                ));
            self.main_window
                .set_active_tech_progress(snapshot.active_tech_progress);
            self.main_window
                .set_active_tech_progress_text(SharedString::from(format!(
                    "{:.0}%",
                    snapshot.active_tech_progress * 100.0
                )));
            self.main_window.set_has_active_research(true);
        } else {
            self.main_window
                .set_active_tech_name(SharedString::from("NONE"));
            self.main_window.set_has_active_research(false);
            self.main_window.set_active_tech_progress(0.0);
            self.main_window
                .set_active_tech_progress_text(SharedString::from("0%"));
        }

        let market_modal_open = self.main_window.get_market_modal_open();
        if market_modal_open {
            let opened_now = !self.last_market_modal_open;
            let country_changed = self.last_active_country != active_country;
            let time_elapsed = self.last_market_update.elapsed() >= Duration::from_millis(800);
            if opened_now || country_changed || time_elapsed {
                let mut local_items = Vec::new();
                if let Some(list) = snapshot.local_market_prices.get(active_country) {
                    let symbol = snapshot
                        .countries
                        .get(active_country)
                        .map(|c| c.currency_symbol.as_str())
                        .unwrap_or("$");
                    for r in list {
                        local_items.push(ResourcePriceItem {
                            name: SharedString::from(&r.name),
                            price_text: SharedString::from(format!(
                                "{}{:.1}",
                                symbol, r.local_price
                            )),
                            base_price_text: SharedString::from(format!(
                                "{}{:.1}",
                                symbol, r.base_price
                            )),
                            demand_text: SharedString::from(format_compact_number(r.demand)),
                            supply_text: SharedString::from(format_compact_number(r.supply)),
                            volume_text: SharedString::from(format_compact_number(r.global_volume)),
                        });
                    }
                }
                self.main_window
                    .set_local_market_items(Rc::new(slint::VecModel::from(local_items)).into());

                let mut global_items = Vec::new();
                for r in &snapshot.global_market_prices {
                    global_items.push(ResourcePriceItem {
                        name: SharedString::from(&r.name),
                        price_text: SharedString::from(format!("${:.1}", r.global_price)),
                        base_price_text: SharedString::from(format!("${:.1}", r.base_price)),
                        demand_text: SharedString::from("-"),
                        supply_text: SharedString::from("-"),
                        volume_text: SharedString::from(format_compact_number(r.global_volume)),
                    });
                }
                self.main_window
                    .set_global_market_items(Rc::new(slint::VecModel::from(global_items)).into());
                self.last_market_ticks = snapshot.total_ticks;
                self.last_market_update = Instant::now();
            }
        }
        self.last_market_modal_open = market_modal_open;

        let companies_modal_open = self.main_window.get_companies_modal_open();
        if companies_modal_open {
            let opened_now = !self.last_companies_modal_open;
            let time_elapsed = self.last_companies_update.elapsed() >= Duration::from_millis(1000);
            if opened_now || time_elapsed {
                let mut company_items = Vec::new();
                for c in &snapshot.companies {
                    let profit_prefix = if c.last_month_profit >= 0.0 { "+" } else { "" };
                    let profit_str = format!(
                        "{}{}",
                        profit_prefix,
                        format_compact_currency("$", c.last_month_profit)
                    );
                    company_items.push(CompanyItem {
                        name: SharedString::from(&c.name),
                        sector: SharedString::from(&c.sector),
                        country_tag: SharedString::from(&c.country_tag),
                        balance_text: SharedString::from(format_compact_currency("$", c.balance)),
                        profit_text: SharedString::from(profit_str),
                        is_positive_profit: c.last_month_profit >= 0.0,
                    });
                }
                self.main_window
                    .set_company_items(Rc::new(slint::VecModel::from(company_items)).into());
                self.last_companies_ticks = snapshot.total_ticks;
                self.last_companies_update = Instant::now();
            }
        }
        self.last_companies_modal_open = companies_modal_open;

        self.last_active_country = active_country.to_string();
    }

    #[allow(dead_code)]
    pub fn is_country_menu_open(&self) -> bool {
        self.main_window.get_country_menu_open()
    }

    pub fn render(&mut self, overlay: &OverlayPipeline, queue: &wgpu::Queue) {
        let width = self.width;
        let height = self.height;

        self.software_window.draw_if_needed(|renderer| {
            self.pixel_buffer.fill(PremultipliedRgbaColor::default());
            renderer.render(&mut self.pixel_buffer, width as usize);
            overlay.update_pixels(queue, &self.pixel_buffer, width, height);
        });
    }

    pub fn is_point_over_ui(&self, x: f32, y: f32) -> bool {
        if y <= 44.0 {
            return true;
        }

        if self.main_window.get_market_modal_open()
            || self.main_window.get_companies_modal_open()
            || self.main_window.get_construction_modal_open()
            || self.main_window.get_tech_modal_open()
        {
            return true;
        }

        let nav_x = 14.0f32;
        let nav_y = 50.0f32;
        let nav_width = 380.0f32;
        let nav_height = if self.main_window.get_market_menu_open() {
            104.0f32
        } else {
            32.0f32
        };
        if x >= nav_x && x <= nav_x + nav_width && y >= nav_y && y <= nav_y + nav_height {
            return true;
        }

        if self.main_window.get_region_panel_open() {
            let panel_x = 14.0f32;
            let panel_y = 88.0f32;
            let panel_width = 380.0f32;
            let panel_height = 600.0f32;
            if x >= panel_x
                && x <= panel_x + panel_width
                && y >= panel_y
                && y <= panel_y + panel_height
            {
                return true;
            }
        }

        if self.main_window.get_country_menu_open() {
            let panel_x = 14.0f32;
            let panel_y = 88.0f32;
            let panel_width = 320.0f32;
            let panel_height = 490.0f32;
            if x >= panel_x
                && x <= panel_x + panel_width
                && y >= panel_y
                && y <= panel_y + panel_height
            {
                return true;
            }
        }

        let is_open = self.main_window.get_map_menu_open();
        let width = if is_open { 180.0f32 } else { 42.0f32 };
        let height = if is_open { 240.0f32 } else { 42.0f32 };
        let bar_x = (self.width as f32) - width - 16.0f32;
        let bar_y = (self.height as f32) - height - 16.0f32;

        if x >= bar_x && x <= bar_x + width && y >= bar_y && y <= bar_y + height {
            return true;
        }

        false
    }

    pub fn forward_pointer_move(&self, x: f32, y: f32) {
        let position = LogicalPosition::new(x, y);
        self.software_window
            .dispatch_event(WindowEvent::PointerMoved { position });
    }

    pub fn forward_pointer_press(&self, x: f32, y: f32) {
        let position = LogicalPosition::new(x, y);
        self.software_window
            .dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
    }

    pub fn forward_pointer_release(&self, x: f32, y: f32) {
        let position = LogicalPosition::new(x, y);
        self.software_window
            .dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
    }
}

fn format_compact_currency(symbol: &str, value: f64) -> String {
    let sign = if value < 0.0 { "-" } else { "" };
    let abs_val = value.abs();

    let (scaled, suffix) = if abs_val >= 1_000_000_000_000.0 {
        (abs_val / 1_000_000_000_000.0, "T")
    } else if abs_val >= 1_000_000_000.0 {
        (abs_val / 1_000_000_000.0, "B")
    } else if abs_val >= 1_000_000.0 {
        (abs_val / 1_000_000.0, "M")
    } else if abs_val >= 1_000.0 {
        (abs_val / 1_000.0, "K")
    } else {
        return format!("{sign}{symbol}{:.0}", abs_val);
    };

    if (scaled * 10.0).fract() == 0.0 && scaled.fract() == 0.0 {
        format!("{sign}{symbol}{:.0}{suffix}", scaled)
    } else {
        format!("{sign}{symbol}{:.1}{suffix}", scaled)
    }
}

fn format_compact_number(value: f64) -> String {
    let sign = if value < 0.0 { "-" } else { "" };
    let abs_val = value.abs();

    let (scaled, suffix) = if abs_val >= 1_000_000_000_000.0 {
        (abs_val / 1_000_000_000_000.0, "T")
    } else if abs_val >= 1_000_000_000.0 {
        (abs_val / 1_000_000_000.0, "B")
    } else if abs_val >= 1_000_000.0 {
        (abs_val / 1_000_000.0, "M")
    } else if abs_val >= 1_000.0 {
        (abs_val / 1_000.0, "K")
    } else {
        return format!("{sign}{:.0}", abs_val);
    };

    if (scaled * 10.0).fract() == 0.0 && scaled.fract() == 0.0 {
        format!("{sign}{:.0}{suffix}", scaled)
    } else {
        format!("{sign}{:.1}{suffix}", scaled)
    }
}

fn format_inflation(rate: f64) -> (String, slint::Color) {
    let pct = rate * 100.0;
    let prefix = if pct >= 0.0 { "+" } else { "" };
    let text = format!("{prefix}{:.1}%", pct);
    let color = if rate < 0.0 {
        slint::Color::from_rgb_u8(56, 189, 248)
    } else if rate <= 0.04 {
        slint::Color::from_rgb_u8(52, 211, 153)
    } else if rate <= 0.08 {
        slint::Color::from_rgb_u8(251, 191, 36)
    } else {
        slint::Color::from_rgb_u8(248, 113, 113)
    };
    (text, color)
}
