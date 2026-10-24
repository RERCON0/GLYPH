//! Capture only this application's framebuffer, not the desktop. Demo data only.
//! cargo run --example snapshot --features preview -- docs/screenshot.png dark password 480 700
use eframe::egui;
use glyph::{
    generator::Mode,
    ui::{GlyphApp, style},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

struct Snapshot {
    app: GlyphApp,
    output: PathBuf,
    frames: usize,
    started: Instant,
}
impl eframe::App for Snapshot {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.render(ui);
        let screenshot = ui.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = screenshot {
            if let Some(parent) = self.output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let rgba: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            image::save_buffer(
                &self.output,
                &rgba,
                image.size[0] as u32,
                image.size[1] as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frames == 4 {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        self.frames += 1;
        if self.started.elapsed() > Duration::from_secs(15) {
            panic!("snapshot timed out");
        }
        ui.ctx().request_repaint_after(Duration::from_millis(60));
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let output = PathBuf::from(args.first().expect("output PNG path"));
    let dark = args.get(1).is_none_or(|s| s != "light");
    let mode = match args.get(2).map(String::as_str) {
        Some("pin") => Mode::Pin,
        Some("hex") => Mode::Hex,
        Some("url") => Mode::Url,
        Some("fernet") => Mode::Fernet,
        _ => Mode::Password,
    };
    let size = [
        args.get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(style::WINDOW_SIZE[0]),
        args.get(4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(style::WINDOW_SIZE[1]),
    ];
    let show_history = args.get(5).is_some_and(|arg| arg == "history");
    let seed_history = show_history || args.get(5).is_some_and(|arg| arg == "history-closed");
    let history_count = args.get(6).and_then(|arg| arg.parse().ok()).unwrap_or(6);
    eframe::run_native(
        "GLYPH preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("GLYPH preview — demo only")
                .with_inner_size(size)
                .with_decorations(false),
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = GlyphApp::preview(&cc.egui_ctx, mode, dark);
            if seed_history {
                app.prepare_history_preview(history_count, show_history);
            }
            Ok(Box::new(Snapshot {
                app,
                output,
                frames: 0,
                started: Instant::now(),
            }))
        }),
    )
}
