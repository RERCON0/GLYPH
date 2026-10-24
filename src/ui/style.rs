//! Terminal Native: the SNATCH/RAZE family, with GLYPH's ice-cyan accent.
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke};

pub const WINDOW_SIZE: [f32; 2] = [480.0, 700.0];
pub const MIN_SIZE: [f32; 2] = [420.0, 540.0];
pub const CONTENT_WIDTH: f32 = 580.0;
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const TITLE_HEIGHT: f32 = 34.0;
pub const TITLE_BUTTON_WIDTH: f32 = 34.0;
pub const LANG_BUTTON_WIDTH: f32 = 34.0;
pub const THEME_BUTTON_WIDTH: f32 = 54.0;
const MONO: &[u8] = include_bytes!("../../fonts/CascadiaMono-Light.ttf");

pub fn accent(dark: bool) -> Color32 {
    if dark {
        // Muted ice-cyan: the SNATCH palette's green sits at a similar
        // luminance, but pure cyan at #70D6DD read noticeably brighter.
        Color32::from_rgb(95, 179, 186)
    } else {
        Color32::from_rgb(0, 104, 116)
    }
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, offset) in [
        ("body", 0.15),
        // Explicitly select this face for buttons: the body's +0.15em offset
        // is for free-standing labels and puts button captions below centre.
        ("button", -0.05),
        ("field", 0.0),
        ("title", -0.08),
    ] {
        let key = format!("cascadia-{name}");
        fonts.font_data.insert(
            key.clone(),
            egui::FontData::from_static(MONO)
                .tweak(egui::FontTweak {
                    y_offset_factor: offset,
                    ..Default::default()
                })
                .into(),
        );
        // Keep egui's symbol fallbacks: Cascadia alone does not contain ↻.
        let mut family = vec![key];
        family.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
        fonts.families.insert(FontFamily::Name(name.into()), family);
    }
    for family in [FontFamily::Monospace, FontFamily::Proportional] {
        fonts
            .families
            .get_mut(&family)
            .unwrap()
            .insert(0, "cascadia-body".into());
    }
    ctx.set_fonts(fonts);
}

pub fn setup_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|style| {
        style.visuals = visuals(dark);
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 5.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.icon_width = 14.0;
        style.spacing.icon_width_inner = 8.0;
        style.spacing.window_margin = egui::Margin::symmetric(12, 14);
        style.spacing.scroll = egui::style::ScrollStyle {
            foreground_color: true,
            ..egui::style::ScrollStyle::solid()
        };
        style.spacing.scroll.bar_width = 5.0;
        style.interaction.selectable_labels = false;
        for (text, size, family) in [
            (egui::TextStyle::Body, 12.0, FontFamily::Proportional),
            (
                egui::TextStyle::Monospace,
                12.0,
                FontFamily::Name("field".into()),
            ),
            (
                egui::TextStyle::Button,
                11.5,
                FontFamily::Name("button".into()),
            ),
            (
                egui::TextStyle::Small,
                10.5,
                FontFamily::Name("button".into()),
            ),
            (
                egui::TextStyle::Heading,
                16.0,
                FontFamily::Name("title".into()),
            ),
        ] {
            style.text_styles.insert(text, FontId::new(size, family));
        }
    });
}

fn visuals(dark: bool) -> egui::Visuals {
    let rgb = Color32::from_rgb;
    let (bg, lift, field, line, bright, text, dim) = if dark {
        (
            rgb(11, 11, 14),
            rgb(14, 14, 17),
            rgb(17, 17, 20),
            rgb(30, 30, 36),
            rgb(51, 51, 60),
            rgb(214, 214, 218),
            rgb(131, 131, 140),
        )
    } else {
        (
            rgb(243, 243, 241),
            Color32::WHITE,
            rgb(251, 251, 250),
            rgb(218, 218, 214),
            rgb(181, 181, 176),
            rgb(28, 28, 30),
            rgb(92, 92, 98),
        )
    };
    let accent = accent(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    v.window_fill = lift;
    v.window_stroke = Stroke::new(1.0, line);
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.panel_fill = bg;
    v.extreme_bg_color = field;
    v.faint_bg_color = field;
    v.code_bg_color = field;
    v.hyperlink_color = accent;
    // No override_text_color: plain labels and field values then use
    // widgets.noninteractive.fg_stroke (text_dim), exactly like SNATCH.
    // `text` stays reserved for hover/active and explicit emphasis.
    v.warn_fg_color = if dark {
        rgb(224, 179, 77)
    } else {
        rgb(154, 103, 0)
    };
    v.error_fg_color = if dark {
        rgb(224, 101, 92)
    } else {
        rgb(177, 32, 45)
    };
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 45);
    v.selection.stroke = Stroke::new(1.0, accent);
    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
        widget.bg_fill = field;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.expansion = 0.0;
        widget.bg_stroke = Stroke::new(1.0, line);
        widget.fg_stroke = Stroke::new(1.0, dim);
    }
    v.widgets.noninteractive.weak_bg_fill = bg;
    v.widgets.hovered.bg_stroke.color = bright;
    v.widgets.hovered.fg_stroke.color = text;
    v.widgets.active.bg_stroke.color = accent;
    v.widgets.active.fg_stroke.color = text;
    v.widgets.open.bg_stroke.color = bright;
    v.widgets.open.fg_stroke.color = text;
    v
}

pub fn button(text: &str, accent: Color32) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .font(button_font(11.5))
            .color(accent),
    )
    .stroke(Stroke::new(1.0, accent))
    .fill(Color32::TRANSPARENT)
}

pub fn button_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("button".into()))
}

pub fn plain_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).font(button_font(11.5)))
}

pub fn small_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).font(button_font(10.5))).small()
}

/// A permanent, narrow gutter prevents an appearing scrollbar from changing
/// content width. The floating handle stays in that gutter, not over controls.
pub fn scroll_area<R>(
    ui: &mut egui::Ui,
    area: egui::ScrollArea,
    content: impl FnOnce(&mut egui::Ui, egui::Rect) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    ui.scope(|ui| {
        let content_style = ui.style().clone();
        let handle = ui.visuals().widgets.hovered.bg_stroke.color;
        let hovered = ui.visuals().weak_text_color();
        let active = ui.visuals().text_color();
        let scroll = &mut ui.style_mut().spacing.scroll;
        scroll.floating = true;
        scroll.floating_allocated_width = 0.0;
        scroll.floating_width = 5.0;
        scroll.bar_width = 5.0;
        scroll.bar_inner_margin = 0.0;
        scroll.bar_outer_margin = 0.0;
        ui.visuals_mut().widgets.inactive.fg_stroke.color = handle;
        ui.visuals_mut().widgets.hovered.fg_stroke.color = hovered;
        ui.visuals_mut().widgets.active.fg_stroke.color = active;
        area.content_margin(egui::Margin {
            right: 8,
            ..Default::default()
        })
        .show_viewport(ui, |inner, viewport| {
            inner.set_style(content_style);
            content(inner, viewport)
        })
    })
    .inner
}

pub fn tag(ui: &mut egui::Ui, text: &str) {
    ui.weak(RichText::new(format!("› {text}")).size(10.5));
}

pub fn rule(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let mut x = rect.left();
    while x < rect.right() {
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.center().y),
                egui::pos2((x + 4.0).min(rect.right()), rect.center().y),
            ],
            stroke,
        );
        x += 7.0;
    }
}

pub fn wordmark(ui: &mut egui::Ui, dark: bool) {
    let font = FontId::new(9.5, FontFamily::Name("field".into()));
    let galley = ui.painter().layout_no_wrap(
        crate::WORDMARK.to_owned(),
        font,
        ui.visuals().text_color().gamma_multiply(0.65),
    );
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), galley.size().y + 2.0),
        egui::Sense::hover(),
    );
    ui.painter().galley(
        egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top()),
        galley,
        ui.visuals().text_color(),
    );
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(
            RichText::new(crate::tr!(
                "passwords / tokens / keys",
                "пароли / токены / ключи"
            ))
            .size(10.0)
            .color(accent(dark)),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_ink_is_vertically_centered_at_common_display_scales() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for caption in [
                "PIN",
                "Пароль",
                "Копировать",
                "Создать",
                "↻ Создать",
                "32",
                "день",
            ] {
                let ctx = egui::Context::default();
                setup_fonts(&ctx);
                setup_theme(&ctx, true);
                ctx.set_pixels_per_point(scale);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        WINDOW_SIZE.into(),
                    )),
                    ..Default::default()
                };
                let mut button_rect = egui::Rect::NOTHING;
                let mut output = ctx.run_ui(input, |ui| {
                    button_rect = ui
                        .add_sized([120.0, CONTROL_HEIGHT], plain_button(caption))
                        .rect;
                });
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.text() == caption => {
                            Some(text)
                        }
                        _ => None,
                    })
                    .expect("button caption must be painted");
                let ink_center = text.pos.y + text.galley.mesh_bounds.center().y;
                let error = (ink_center - button_rect.center().y).abs();
                output.textures_delta.clear();
                assert!(
                    error <= 1.5,
                    "{caption}, {scale}x: visible ink is {error:.2}pt off centre"
                );
            }
        }
    }
}
