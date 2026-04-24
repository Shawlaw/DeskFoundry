#![cfg_attr(
    all(target_os = "windows", not(feature = "console")),
    windows_subsystem = "windows"
)]

use eframe::egui;

fn main() -> eframe::Result<()> {
    let console_mode = cfg!(feature = "console") || std::env::args().any(|arg| arg == "--console");
    let paths = desktop_config::resolve_portable_app_paths(
        "com",
        "DeskFoundry",
        "DesktopAppTemplate",
        "config.json",
        ".desktop-app-template.log",
    )
    .unwrap_or_else(|err| fatal_error(&err));

    desktop_logger::init(&paths.app_log_path, console_mode, 2).unwrap_or_else(|err| fatal_error(&err));
    desktop_logger::set_panic_hook(&paths.app_log_path);

    log::info!("========== Desktop App Template v{} startup ==========", env!("CARGO_PKG_VERSION"));
    log::info!("System language: {}", desktop_i18n::detect_system_language());
    log::info!("Config path: {}", desktop_fs::display_path(&paths.config_path));
    log::info!("App log path: {}", desktop_fs::display_path(&paths.app_log_path));

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icons/icon_256.png"))
        .unwrap_or_default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 520.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Desktop App Template",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(TemplateApp {
                paths,
            }))
        }),
    )
}

struct TemplateApp {
    paths: desktop_config::PortableAppPaths,
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Desktop App Template");
            ui.label("Use this project as a starting point for a new Windows-first desktop tool.");
            ui.add_space(12.0);
            ui.label(format!("Portable mode: {}", self.paths.portable_mode));
            ui.label(format!(
                "Config path: {}",
                desktop_fs::display_path(&self.paths.config_path)
            ));
            ui.label(format!(
                "App log path: {}",
                desktop_fs::display_path(&self.paths.app_log_path)
            ));
        });
    }
}

fn fatal_error(message: &str) -> ! {
    let _ = rfd::MessageDialog::new()
        .set_title("Desktop App Template")
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .show();
    std::process::exit(1);
}
