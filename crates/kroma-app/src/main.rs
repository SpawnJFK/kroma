//! Kroma desktop app entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod demo;
mod dialogs;
mod icon;
mod icons;
mod panels;
mod recovery;
mod tab;
mod theme;
mod tools;
mod widgets;

fn main() -> eframe::Result {
    let files: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kroma")
            .with_app_id("kroma")
            .with_icon(crate::icon::icon_data())
            .with_inner_size([1480.0, 920.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Kroma",
        options,
        Box::new(move |cc| Ok(Box::new(app::KromaApp::new(cc, files)))),
    )
}

#[cfg(test)]
mod tests;
