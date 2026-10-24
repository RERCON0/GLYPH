#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

use eframe::egui;
use glyph::ui::{GlyphApp, style};

fn main() -> eframe::Result {
    let image = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .expect("embedded GLYPH icon")
        .into_rgba8();
    let viewport = egui::ViewportBuilder::default()
        .with_title("GLYPH — rercon prod.")
        .with_inner_size(style::WINDOW_SIZE)
        .with_min_inner_size(style::MIN_SIZE)
        .with_decorations(false)
        .with_icon(egui::IconData {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        });
    let result = eframe::run_native(
        "GLYPH",
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(GlyphApp::new(&cc.egui_ctx)))),
    );
    #[cfg(windows)]
    if result.is_err() {
        use windows_sys::{
            Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
            core::w,
        };
        // No generated secrets or panic payloads in the fatal diagnostic.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                w!(
                    "Не удалось открыть GLYPH. Для интерфейса нужен драйвер с поддержкой OpenGL 2.1 или новее."
                ),
                w!("GLYPH"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    result
}
