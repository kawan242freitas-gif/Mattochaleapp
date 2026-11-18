#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod api;
#[allow(dead_code)]
mod app;
mod dashboard;
mod icons;
#[allow(dead_code)]
mod model;
mod updater;

fn main() -> eframe::Result {
    let app_icon = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .expect("ícone do aplicativo inválido")
        .to_rgba8();
    let (icon_width, icon_height) = app_icon.dimensions();
    let app_icon = eframe::egui::IconData {
        rgba: app_icon.into_raw(),
        width: icon_width,
        height: icon_height,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Matto Chalés")
            .with_icon(std::sync::Arc::new(app_icon))
            .with_inner_size([1536.0, 760.0])
            .with_min_inner_size([1080.0, 620.0])
            .with_maximized(true)
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Matto Chalés",
        options,
        Box::new(|cc| Ok(Box::new(dashboard::DashboardApp::new(cc)))),
    )
}
