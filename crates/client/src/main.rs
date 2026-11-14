mod app;
mod bridge;
mod debug;
mod map;
mod render;
mod ui;

use app::App;
use bridge::SimulationWorker;
use crossbeam_channel::{bounded, unbounded};
use std::path::Path;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let defs_path = Path::new("data/common");
    let scenario_path = Path::new("data/scenarios/2020_default");

    let split_countries = &["USA", "RUS", "CHN"];

    let (command_tx, command_rx) = unbounded();
    let (snapshot_tx, snapshot_rx) = bounded(8);

    let _worker_handle =
        SimulationWorker::spawn(defs_path, scenario_path, command_rx, snapshot_tx)?;

    let event_loop = EventLoop::new()?;
    let mut app = App::new(command_tx, snapshot_rx, split_countries);

    event_loop.run_app(&mut app)?;

    Ok(())
}
