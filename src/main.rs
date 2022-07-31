/*
use clap::Parser;

#[derive(Parser)]
struct Cli {}
*/

use egui::Vec2;
use tracing_gui::App;

fn main() {
    // let _cli = Cli::parse();

    let egui_options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Some(Vec2::new(1000.0, 800.0)),
        ..Default::default()
    };

    // Launch the app
    eframe::run_native(
        "tracing-gui",
        egui_options,
        Box::new(|cc| Box::new(App::new(cc))),
    );
}
