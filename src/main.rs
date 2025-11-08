// src/main.rs

mod app;

use app::state::App;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Showel - Database Client",
        native_options,
        Box::new(|_| Ok(Box::new(App::new()))),
    )
}
