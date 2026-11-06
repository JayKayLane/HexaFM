#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use hexa_fm::app;
use std::path::PathBuf;

struct Standalone(app::App);

impl eframe::App for Standalone {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.0.ui(ctx);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.0.shutdown();
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut tab: Option<String> = None;
    let mut text_size: Option<f32> = None;
    let mut theme: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--theme" {
            theme = args.get(i + 1).cloned();
            i += 1;
        } else if args[i] == "--text-size" {
            text_size = args.get(i + 1).and_then(|v| v.parse::<f32>().ok());
            i += 1;
        } else if args[i] == "--tab" {
            tab = args.get(i + 1).cloned();
            i += 1;
        } else if args[i] == "--screenshot" {
            if let Some(p) = args.get(i + 1) {
                screenshot = Some(PathBuf::from(p));
                i += 1;
            }
        }
        i += 1;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Hexa FM")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("Hexa FM", options, Box::new(move |cc| { let mut a = app::App::new(&cc.egui_ctx, screenshot); a.set_start_tab(tab.as_deref()); if let Some(t) = text_size { a.settings.text_size = t; } match theme.as_deref() { Some("dx") => a.settings.theme = hexa_fm::theme::ThemeId::DxClassic, Some("graphite") => a.settings.theme = hexa_fm::theme::ThemeId::Graphite, Some("daylight") => a.settings.theme = hexa_fm::theme::ThemeId::Daylight, _ => {} } Ok(Box::new(Standalone(a))) }))
}
