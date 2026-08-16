mod app;
mod engine;
mod types;
mod util;

use anyhow::Result;

fn run_gui() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "MOQ Secure Chat",
        options,
        Box::new(|cc| Ok(Box::new(app::ChatApp::new(cc)))),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    run_gui()
}
