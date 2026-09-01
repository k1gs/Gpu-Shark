//! Experimental egui front end. Telemetry remains on the dedicated worker in
//! `app`; the UI thread only consumes snapshots and paints immediate-mode UI.
#![windows_subsystem = "windows"]

mod app;
mod feedback;
mod gui_i18n;
mod gui_state;
mod sensor_model;
mod settings;

fn main() -> eframe::Result {
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title(format!("GPU Shark {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size([920.0, 600.0])
        .with_min_inner_size([760.0, 500.0])
        .with_resizable(true)
        .with_maximize_button(false)
        .with_minimize_button(true);
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        centered: true,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "GPU Shark",
        options,
        Box::new(|cc| Ok(Box::new(app::GpuSharkApp::new(cc)))),
    )
}
