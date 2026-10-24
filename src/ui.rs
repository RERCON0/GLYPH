use std::{borrow::Cow, collections::VecDeque, rc::Rc};

use eframe::egui::{self, Align, FontId, RichText, Stroke};
use zeroize::Zeroizing;

use crate::{
    TELEGRAM_HANDLE, VERSION,
    clipboard::{Clipboard, ClipboardError, PollResult},
    generator::{self, Generated, Mode, PasswordOptions},
    i18n, tr, tr_format,
};

pub mod style;
const MAX_HISTORY: usize = 80;

// Never Debug/Serialize session actions: Copy owns a reference to a secret.
enum HistoryAction {
    Copy(Rc<Generated>),
    Clear,
}

pub struct GlyphApp {
    mode: Mode,
    options: PasswordOptions,
    pin_length: usize,
    hex_bytes: usize,
    url_bytes: usize,
    hidden_enabled: bool,
    hidden_amount: usize,
    result: Option<Rc<Generated>>,
    representation: Zeroizing<String>,
    history: VecDeque<Rc<Generated>>,
    clipboard: Clipboard,
    dark: bool,
    masked: bool,
    maximized: bool,
    status: String,
    status_error: bool,
    dirty: bool,
    generation: u64,
    pixel_generation: u64,
    pixel_started: f64,
    #[cfg(any(test, feature = "preview"))]
    preview_history_open: bool,
}

impl GlyphApp {
    pub fn new(ctx: &egui::Context) -> Self {
        i18n::set_language(i18n::initial_language());
        style::setup_fonts(ctx);
        style::setup_theme(ctx, true);
        let mut app = Self {
            mode: Mode::Password,
            options: PasswordOptions::default(),
            pin_length: 4,
            hex_bytes: 32,
            url_bytes: 48,
            hidden_enabled: false,
            hidden_amount: 6,
            result: None,
            representation: Zeroizing::new(String::new()),
            history: VecDeque::new(),
            clipboard: Clipboard::default(),
            dark: true,
            masked: false,
            maximized: false,
            status: String::new(),
            status_error: false,
            dirty: false,
            generation: 0,
            pixel_generation: 0,
            pixel_started: 0.0,
            #[cfg(any(test, feature = "preview"))]
            preview_history_open: false,
        };
        app.generate();
        app
    }

    fn generate(&mut self) {
        self.dirty = false;
        self.options.hidden_count = if self.hidden_enabled && !self.options.argon2_safe {
            self.hidden_amount
        } else {
            0
        };
        let result = match self.mode {
            Mode::Password => generator::password(&self.options),
            Mode::Pin => generator::pin(self.pin_length, self.options.exclude_similar),
            Mode::Hex => generator::token(self.mode, self.hex_bytes),
            Mode::Url => generator::token(self.mode, self.url_bytes),
            Mode::Fernet => generator::token(self.mode, 32),
        };
        match result {
            Ok(result) => {
                self.representation = result.representation();
                let result = Rc::new(result);
                self.history.push_front(Rc::clone(&result));
                self.history.truncate(MAX_HISTORY);
                self.result = Some(result);
                self.generation = self.generation.wrapping_add(1);
                self.status = tr!(
                    "Created locally. Nothing is saved to disk.",
                    "Создано локально. Ничего не сохраняется на диск."
                )
                .into();
                self.status_error = false;
            }
            Err(error) => {
                self.result = None;
                self.representation = Zeroizing::new(String::new());
                self.status = error.to_string();
                self.status_error = true;
            }
        }
    }

    fn copy(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.status = tr!(
                "Parameters changed. Press “Create” first.",
                "Параметры изменены. Сначала нажмите «Создать»."
            )
            .into();
            self.status_error = false;
            return;
        }
        let Some(result) = &self.result else {
            return;
        };
        let copied = self.clipboard.copy(&result.secret);
        self.finish_copy(ctx, copied);
    }

    fn finish_copy(&mut self, ctx: &egui::Context, copied: Result<(), ClipboardError>) {
        match copied {
            Ok(()) => {
                self.status = tr!(
                    "Copied · auto-clear in 30 seconds.",
                    "Скопировано · автоочистка через 30 секунд."
                )
                .into();
                self.status_error = false;
                ctx.request_repaint_after(crate::clipboard::LIFETIME);
            }
            Err(error) => {
                self.status = error.to_string();
                self.status_error = true;
            }
        }
    }

    fn apply_history_action(&mut self, action: HistoryAction, ctx: &egui::Context) {
        match action {
            HistoryAction::Copy(entry) => {
                // Copy this exact archived value, even if current parameters
                // changed. Never regenerate, strip Unicode or copy an ellipsis.
                let copied = self.clipboard.copy(&entry.secret);
                self.finish_copy(ctx, copied);
            }
            HistoryAction::Clear => {
                self.history.clear();
                self.status = tr!(
                    "Session history cleared. The current result is kept.",
                    "История сеанса очищена. Текущий результат сохранён."
                )
                .into();
                self.status_error = false;
            }
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            self.generate();
        }
    }

    fn set_argon2(&mut self, enabled: bool) {
        self.options.argon2_safe = enabled;
        if enabled {
            self.hidden_enabled = false;
            self.options.hidden_count = 0;
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        match self.clipboard.poll() {
            PollResult::Cleared => {
                self.status = tr!("Clipboard cleared.", "Буфер обмена очищен.").into();
                self.status_error = false;
            }
            PollResult::Superseded => {
                self.status = tr!(
                    "The clipboard already contains something else — it was not changed.",
                    "В буфере уже другое содержимое — оно не изменено."
                )
                .into();
                self.status_error = false;
            }
            PollResult::Failed => {
                self.status = tr!(
                    "Couldn't clear the busy clipboard. Clear it manually.",
                    "Не удалось очистить занятый буфер. Очистите его вручную."
                )
                .into();
                self.status_error = true;
            }
            PollResult::Idle => {}
        }
        if let Some(delay) = self.clipboard.next_check() {
            ctx.request_repaint_after(delay);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let generate = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Enter);
        let copy = egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            egui::Key::C,
        );
        if ctx.input_mut(|i| i.consume_shortcut(&generate)) {
            self.generate();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&copy)) {
            self.copy(ctx);
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        // Current UI input is available here. App::logic may run while hidden,
        // where egui intentionally retains the last UI frame's key state.
        self.shortcuts(ui.ctx());
        self.title_bar(ui);
        let ctx = ui.ctx().clone();
        let drawn_status = self.status.clone();
        egui::Panel::bottom("footer")
            .frame(
                egui::Frame::NONE
                    .fill(ui.visuals().window_fill)
                    .inner_margin(egui::Margin::symmetric(22, 8)),
            )
            .show_separator_line(true)
            .show(ui, |ui| {
                let color = if self.status_error {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().text_color()
                };
                ui.add(egui::Label::new(RichText::new(&self.status).size(10.5).color(color)).truncate())
                    .on_hover_text(&self.status);
                ui.horizontal(|ui| {
                    ui.weak(RichText::new(format!("glyph {VERSION} / rercon prod.")).size(10.0))
                        .on_hover_text(tr!(
                            "Secrets are created and stay local. GLYPH makes no network requests; the browser opens only when you click the channel link.",
                            "Секреты создаются и остаются локально. GLYPH сам не обращается к сети; браузер открывается только по вашему клику на ссылку канала."
                        ));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let link = egui::Button::new(
                            RichText::new(TELEGRAM_HANDLE)
                                .font(style::button_font(10.0))
                                .color(style::accent(self.dark)),
                        )
                        .frame(false);
                        if ui
                            .add(link)
                            .on_hover_text(tr!(
                                "Open the rercon channel in a browser",
                                "Открыть канал rercon в браузере"
                            ))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            if crate::system::open_telegram() {
                                self.status = tr!(
                                    "Opening t.me/rercon in the browser.",
                                    "Открываю t.me/rercon в браузере."
                                )
                                .into();
                                self.status_error = false;
                            } else {
                                self.status = tr!(
                                    "Couldn't open the browser. Address: t.me/rercon",
                                    "Не удалось открыть браузер. Адрес: t.me/rercon"
                                )
                                .into();
                                self.status_error = true;
                            }
                        }
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(ctx.global_style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(22, 12)),
            )
            .show(ui, |ui| {
                let width = ui.available_width().min(style::CONTENT_WIDTH);
                ui.with_layout(egui::Layout::top_down(Align::Center), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, ui.available_height()),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            let pixels_per_point = ui.ctx().pixels_per_point();
                            // egui rounds the content rectangle to physical pixels.
                            // A half-pixel viewport can otherwise acquire a phantom
                            // subpixel scroll range and move the whole form on wheel.
                            let available_height = (ui.available_height() * pixels_per_point)
                                .floor()
                                / pixels_per_point;
                            let main = style::scroll_area(
                                ui,
                                egui::ScrollArea::vertical()
                                    .id_salt("main")
                                    .max_height(available_height)
                                    .auto_shrink([false, true]),
                                |ui, _| {
                                    // Use the host's finite budget rather than a
                                    // scrolled/rounded child rectangle, even while
                                    // the closed area shrinks to current contents.
                                    let content_bottom = ui.max_rect().top() + available_height;
                                    style::wordmark(ui, self.dark);
                                    ui.add_space(10.0);
                                    self.modes(ui);
                                    ui.add_space(10.0);
                                    self.result_panel(ui);
                                    ui.add_space(12.0);
                                    style::rule(ui);
                                    ui.add_space(8.0);
                                    let before = (
                                        self.options.clone(),
                                        self.pin_length,
                                        self.hex_bytes,
                                        self.url_bytes,
                                        self.hidden_enabled,
                                        self.hidden_amount,
                                    );
                                    self.settings(ui);
                                    let after = (
                                        self.options.clone(),
                                        self.pin_length,
                                        self.hex_bytes,
                                        self.url_bytes,
                                        self.hidden_enabled,
                                        self.hidden_amount,
                                    );
                                    if before != after {
                                        self.dirty = true;
                                        self.status = tr!(
                                            "Parameters changed. Press “Create”.",
                                            "Параметры изменены. Нажмите «Создать»."
                                        )
                                        .into();
                                        self.status_error = false;
                                    }
                                    ui.add_space(4.0);
                                    if let Some(action) = self.activity(ui, content_bottom) {
                                        self.apply_history_action(action, ui.ctx());
                                    }
                                },
                            );
                            #[cfg(test)]
                            record_scroll(ui.ctx(), "main", &main);
                            #[cfg(not(test))]
                            let _ = main;
                        },
                    );
                });
            });
        if self.status != drawn_status {
            ctx.request_repaint();
        }
        self.resize_edges(&ctx);
    }

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("titlebar")
            .exact_size(style::TITLE_HEIGHT)
            .show_separator_line(true)
            .frame(
                egui::Frame::NONE
                    .fill(ui.visuals().window_fill)
                    .inner_margin(egui::Margin::symmetric(14, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let width = (ui.available_width()
                        - style::LANG_BUTTON_WIDTH
                        - style::THEME_BUTTON_WIDTH
                        - 3.0 * style::TITLE_BUTTON_WIDTH)
                        .max(20.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(width, style::TITLE_HEIGHT),
                        egui::Sense::click_and_drag(),
                    );
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        "GLYPH / rercon prod.",
                        FontId::new(10.5, egui::FontFamily::Name("title".into())),
                        ui.visuals().weak_text_color(),
                    );
                    if response.double_clicked() {
                        self.toggle_maximized(ui.ctx());
                    } else if response.drag_started() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if ui
                        .add_sized(
                            [style::LANG_BUTTON_WIDTH, style::TITLE_HEIGHT - 2.0],
                            style::plain_button(tr!("RU", "EN")).frame(false),
                        )
                        .on_hover_text(tr!("Switch to Russian", "Переключить на английский"))
                        .clicked()
                    {
                        let next = i18n::language().toggled();
                        let saved = i18n::save_language(next);
                        i18n::set_language(next);
                        if saved {
                            self.status =
                                tr!("Interface language: English.", "Язык интерфейса: русский.")
                                    .into();
                            self.status_error = false;
                        } else {
                            self.status = tr!(
                                "Language was not saved; it will reset on restart.",
                                "Язык не сохранён; после перезапуска сбросится."
                            )
                            .into();
                            self.status_error = true;
                        }
                        ui.ctx().request_repaint();
                    }
                    if ui
                        .add_sized(
                            [style::THEME_BUTTON_WIDTH, style::TITLE_HEIGHT - 2.0],
                            style::plain_button(if self.dark {
                                tr!("light", "день")
                            } else {
                                tr!("dark", "ночь")
                            })
                            .frame(false),
                        )
                        .on_hover_text(tr!("Switch theme", "Сменить тему"))
                        .clicked()
                    {
                        self.dark = !self.dark;
                        style::setup_theme(ui.ctx(), self.dark);
                    }
                    for (text, tooltip, action) in [
                        ("─", tr!("Minimize", "Свернуть"), 0),
                        (
                            "□",
                            tr!("Maximize / restore", "Развернуть / восстановить"),
                            1,
                        ),
                        ("×", tr!("Close", "Закрыть"), 2),
                    ] {
                        if ui
                            .add_sized(
                                [style::TITLE_BUTTON_WIDTH, style::TITLE_HEIGHT - 2.0],
                                style::plain_button(text).frame(false),
                            )
                            .on_hover_text(tooltip)
                            .clicked()
                        {
                            match action {
                                0 => ui
                                    .ctx()
                                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                                1 => self.toggle_maximized(ui.ctx()),
                                _ => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                            }
                        }
                    }
                });
            });
    }

    fn toggle_maximized(&mut self, ctx: &egui::Context) {
        self.maximized = !ctx.input(|i| i.viewport().maximized.unwrap_or(self.maximized));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
    }

    fn modes(&mut self, ui: &mut egui::Ui) {
        let width = (ui.available_width() - ui.spacing().item_spacing.x * 4.0) / 5.0;
        ui.horizontal(|ui| {
            for mode in Mode::ALL {
                let selected = self.mode == mode;
                let color = if selected {
                    style::accent(self.dark)
                } else {
                    ui.visuals().weak_text_color()
                };
                let stroke = if selected {
                    Stroke::new(1.0, color)
                } else {
                    ui.visuals().widgets.inactive.bg_stroke
                };
                if ui
                    .add_sized(
                        [width, 28.0],
                        egui::Button::new(
                            RichText::new(mode.label())
                                .font(style::button_font(11.0))
                                .color(color),
                        )
                        .stroke(stroke),
                    )
                    .on_hover_text(mode_description(mode))
                    .clicked()
                {
                    self.set_mode(mode);
                }
            }
        });
    }

    fn result_panel(&mut self, ui: &mut egui::Ui) {
        let accent = style::accent(self.dark);
        ui.horizontal(|ui| {
            style::tag(
                ui,
                if self.dirty {
                    tr!(
                        "result / previous settings",
                        "результат / прежние параметры"
                    )
                } else {
                    tr!("result", "результат")
                },
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        style::small_button(if self.masked {
                            tr!("show", "показать")
                        } else {
                            tr!("hide", "скрыть")
                        })
                        .frame(false),
                    )
                    .clicked()
                {
                    self.masked = !self.masked;
                }
            });
        });
        let frame = egui::Frame::NONE
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .inner_margin(12);
        frame.show(ui, |ui| {
            ui.set_min_width((ui.available_width()).max(0.0));
            ui.set_min_height(34.0);
            if self.masked && self.result.is_some() {
                self.masked_pixels(ui);
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("secret")
                    .max_height(84.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let text = self.result.as_ref().map_or(
                            tr!("Secret not created", "Секрет не создан"),
                            |result| result.secret.as_str(),
                        );
                        // The sole copy path is the timed clipboard, not egui's generic
                        // Ctrl+C/context-menu path. No additional secret in widget state.
                        ui.add(
                            egui::Label::new(
                                RichText::new(text)
                                    .font(FontId::new(13.0, egui::FontFamily::Name("field".into())))
                                    .color(ui.visuals().text_color()),
                            )
                            .wrap()
                            .selectable(false),
                        );
                    });
            }
        });
        if let Some(result) = &self.result {
            let meta = &result.metadata;
            ui.horizontal_wrapped(|ui| {
                ui.weak(RichText::new(format!(
                    "{} · {}",
                    meta.mode.label(),
                    char_count(meta.visible_length)
                ))
                .size(11.0));
                let bits = tr!("bits", "бит");
                ui.label(
                    RichText::new(format!(
                        "{} {:.0} {bits}",
                        if meta.source_bytes.is_some() { "=" } else { "≥" },
                        meta.entropy_bits
                    ))
                    .size(11.0)
                    .color(accent),
                )
                .on_hover_text(tr!(
                    "For a password this is a conservative lower-bound estimate. Hidden characters are not included.",
                    "Для пароля показана консервативная нижняя оценка энтропии. Скрытые символы в неё не включены."
                ));
                ui.weak(RichText::new(generator::strength(meta.entropy_bits)).size(11.0));
            });
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                0.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            );
            let mut fill = rect;
            fill.set_width(rect.width() * (meta.entropy_bits / 256.0).min(1.0) as f32);
            let color = if meta.entropy_bits < 60.0 {
                ui.visuals().warn_fg_color
            } else {
                accent
            };
            ui.painter().rect_filled(fill, 0.0, color);
            if meta.hidden_count > 0 {
                ui.label(
                    RichText::new(tr_format!(
                        "+ {} hidden · sites may strip them on paste",
                        "+ {} скрытых · сайты могут удалить их при вставке",
                        meta.hidden_count
                    ))
                    .size(11.0)
                    .color(ui.visuals().warn_fg_color),
                );
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            if ui
                .add_sized(
                    [width, style::CONTROL_HEIGHT],
                    style::button(tr!("↻ Create", "↻ Создать"), accent),
                )
                .on_hover_text(tr!(
                    "Create a new secret · Ctrl+Enter",
                    "Создать новый секрет · Ctrl+Enter"
                ))
                .clicked()
            {
                self.generate();
            }
            if ui
                .add_enabled(
                    self.result.is_some() && !self.dirty,
                    style::plain_button(tr!("Copy", "Копировать"))
                        .min_size(egui::vec2(width, style::CONTROL_HEIGHT)),
                )
                .on_hover_text(tr!(
                    "Ctrl+Shift+C · the clipboard clears after 30 seconds or on exit",
                    "Ctrl+Shift+C · буфер очистится через 30 секунд или при выходе"
                ))
                .clicked()
            {
                self.copy(ui.ctx());
            }
        });
    }

    /// A fixed pixel-redaction pattern. It depends only on cell coordinates and
    /// time, never on the secret, so the mask cannot leak length, entropy or
    /// characters. The short settle animation makes a regeneration obvious
    /// while the value stays hidden.
    fn masked_pixels(&mut self, ui: &mut egui::Ui) {
        const ROWS: usize = 3;
        const CELL: f32 = 5.0;
        const GAP: f32 = 1.0;
        const DURATION: f64 = 0.45;

        let now = ui.input(|i| i.time);
        if self.pixel_generation != self.generation {
            self.pixel_generation = self.generation;
            self.pixel_started = now;
        }
        let progress = ((now - self.pixel_started) / DURATION).clamp(0.0, 1.0) as f32;
        let animating = progress < 1.0;
        let settle = progress * progress * (3.0 - 2.0 * progress);
        let accent = style::accent(self.dark);
        let base = ui.visuals().text_color();
        let ppp = ui.ctx().pixels_per_point();
        let snap = |value: f32| (value * ppp).round() / ppp;

        let available = ui.available_width();
        // One column per visible character, capped only by the field width.
        // A 16-character password therefore paints half the strip of a
        // 32-character one; the secret itself never influences the pattern.
        let max_columns = (((available + GAP) / (CELL + GAP)).floor() as usize).max(1);
        let columns = self
            .result
            .as_ref()
            .map_or(1, |result| result.metadata.visible_length)
            .clamp(1, max_columns);
        let height = ROWS as f32 * CELL + (ROWS - 1) as f32 * GAP;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(available, 19.0), egui::Sense::hover());
        response.on_hover_text(tr!(
            "Hidden value. Press “show” above the result to display it.",
            "Скрытое значение. Нажмите «показать» над результатом, чтобы увидеть его."
        ));
        let origin = egui::pos2(snap(rect.left()), snap(rect.center().y - height / 2.0));
        let frame = (now * 30.0) as u64;
        let painter = ui.painter();
        for row in 0..ROWS {
            for col in 0..columns {
                let index = (col * ROWS + row) as u64;
                let static_level = (pixel_hash(index ^ 0xA5A5_5A5A_D3C1_B7E9) % 4) as u8;
                let level = if animating {
                    let flicker =
                        (pixel_hash(index ^ frame.wrapping_mul(0x9E37_79B9_7F4A_7C15)) % 5) as u8;
                    if (pixel_hash(index ^ frame) & 0x0f) == 0 {
                        4 // a brief accent spark
                    } else {
                        (flicker as f32 * (1.0 - settle) + static_level as f32 * settle).round()
                            as u8
                    }
                } else {
                    static_level
                };
                let color = match level {
                    0 => base.gamma_multiply(0.30),
                    1 => base.gamma_multiply(0.46),
                    2 => base.gamma_multiply(0.64),
                    3 => base.gamma_multiply(0.84),
                    _ => accent.gamma_multiply(0.90),
                };
                let x = origin.x + col as f32 * (CELL + GAP);
                let y = origin.y + row as f32 * (CELL + GAP);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(snap(x), snap(y)), egui::vec2(CELL, CELL)),
                    0.0,
                    color,
                );
            }
        }
        if animating {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        style::tag(ui, tr!("settings", "параметры"));
        match self.mode {
            Mode::Password | Mode::Pin => {
                let pin = self.mode == Mode::Pin;
                ui.horizontal(|ui| {
                    ui.label(tr!("Length", "Длина"));
                    let length = if pin {
                        &mut self.pin_length
                    } else {
                        &mut self.options.length
                    };
                    ui.add(egui::DragValue::new(length).range(1..=256).speed(1.0))
                        .on_hover_text(tr!(
                            "1 to 256 visible characters. You can type a number.",
                            "От 1 до 256 видимых символов. Можно ввести число с клавиатуры."
                        ));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let presets: &[usize] = if pin { &[8, 6, 4] } else { &[64, 32, 16] };
                        for &preset in presets {
                            if ui.add(style::small_button(&preset.to_string())).clicked() {
                                *length = preset;
                            }
                        }
                    });
                });
                if !pin {
                    self.options.min_digits = self.options.min_digits.min(self.options.length);
                    ui.horizontal(|ui| {
                        ui.label(tr!("Min digits", "Минимум цифр"));
                        ui.add(
                            egui::DragValue::new(&mut self.options.min_digits)
                                .range(0..=self.options.length),
                        );
                    });
                    ui.checkbox(
                        &mut self.options.special,
                        tr!("Special characters", "Спецсимволы"),
                    );
                }
                ui.checkbox(
                    &mut self.options.exclude_similar,
                    tr!(
                        "Exclude look-alikes: 0 O o I l 1",
                        "Без похожих: 0 O o I l 1"
                    ),
                );
                if !pin {
                    self.advanced(ui);
                } else {
                    ui.weak(
                        RichText::new(tr!(
                            "Digits only. No hidden characters.",
                            "Только цифры. Без скрытых символов."
                        ))
                        .size(11.0),
                    );
                }
            }
            Mode::Hex | Mode::Url => {
                ui.horizontal(|ui| {
                    ui.label(tr!("Random bytes", "Случайных байт"));
                    let bytes = if self.mode == Mode::Hex {
                        &mut self.hex_bytes
                    } else {
                        &mut self.url_bytes
                    };
                    ui.add(egui::DragValue::new(bytes).range(1..=1024));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        for preset in [64, 48, 32, 16] {
                            if ui.add(style::small_button(&preset.to_string())).clicked() {
                                *bytes = preset;
                            }
                        }
                    });
                });
                ui.weak(RichText::new(mode_description(self.mode)).size(11.5));
            }
            Mode::Fernet => {
                ui.label(tr!(
                    "32 random bytes → 44 base64url characters with “=”.",
                    "32 случайных байта → 44 символа base64url с «=»."
                ));
                ui.weak(
                    RichText::new(tr!(
                        "For Python cryptography.fernet.Fernet. Keep the key separate from the encrypted data.",
                        "Для Python cryptography.fernet.Fernet. Храните ключ отдельно от зашифрованных данных."
                    ))
                    .size(11.5),
                );
            }
        }
    }

    fn advanced(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(tr!(
            "Compatibility & hidden characters",
            "Совместимость и скрытые символы"
        ))
        .id_salt("advanced")
        .show(ui, |ui| {
            let mut argon2 = self.options.argon2_safe;
            if ui
                .checkbox(&mut argon2, "Argon2 / docker / bash / python")
                .on_hover_text(tr!(
                    "Filter for the legacy nested PasswordHasher().hash(...) command — not hashing and not general shell escaping. Excludes \" ' \\ $ ` ! and disables hidden characters.",
                    "Фильтр для прежней вложенной команды PasswordHasher().hash(...), а не хеширование и не универсальное экранирование shell. Исключает \" ' \\ $ ` ! и отключает скрытые символы."
                ))
                .changed()
            {
                self.set_argon2(argon2);
            }
            ui.add_enabled_ui(!argon2, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.hidden_enabled,
                        tr!("Hidden characters", "Скрытые символы"),
                    );
                    ui.add_enabled(
                        self.hidden_enabled,
                        egui::DragValue::new(&mut self.hidden_amount).range(1..=256),
                    );
                });
            });
            ui.weak(
                RichText::new(tr!(
                    "Hidden characters are off by default: not every site keeps them. The number of insertions is limited by available positions.",
                    "Скрытые символы выключены по умолчанию: не все сайты их сохраняют. Число вставок ограничено доступными позициями."
                ))
                .size(11.0),
            );
        });
        if self
            .result
            .as_ref()
            .is_some_and(|result| result.metadata.hidden_count > 0)
        {
            egui::CollapsingHeader::new(tr!(
                "Show hidden as \\u200B",
                "Показать скрытые как \\u200B"
            ))
            .id_salt("representation")
            .show(ui, |ui| {
                if self.masked {
                    ui.weak(tr!(
                        "Press “show” above the result first.",
                        "Сначала нажмите «показать» над результатом."
                    ));
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("repr-scroll")
                        .max_height(100.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(self.representation.as_str())
                                        .monospace()
                                        .size(12.0),
                                )
                                .wrap()
                                .selectable(false),
                            );
                        });
                }
            });
        }
    }

    fn activity(&self, ui: &mut egui::Ui, content_bottom: f32) -> Option<HistoryAction> {
        let header = egui::CollapsingHeader::new(tr_format!(
            "Session / {}",
            "Сеанс / {}",
            self.history.len()
        ))
        .id_salt("activity");
        #[cfg(any(test, feature = "preview"))]
        let header = header.default_open(self.preview_history_open);
        header
            .show(ui, |ui| self.history_contents(ui, content_bottom))
            .body_returned
            .flatten()
    }

    fn history_contents(&self, ui: &mut egui::Ui, content_bottom: f32) -> Option<HistoryAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            let hint_width = (ui.available_width() - 76.0 - ui.spacing().item_spacing.x).max(20.0);
            ui.add_sized(
                [hint_width, 20.0],
                egui::Label::new(
                    RichText::new(tr_format!(
                        "Up to {} values · memory only",
                        "До {} значений · только до закрытия",
                        MAX_HISTORY
                    ))
                    .size(10.0)
                    .weak(),
                )
                .truncate()
                .halign(Align::Min),
            );
            if ui
                .add_enabled(
                    !self.history.is_empty(),
                    style::small_button(tr!("Clear", "Очистить")),
                )
                .on_hover_text(tr!(
                    "Delete the history from memory. The current result stays.",
                    "Удалить историю из памяти. Текущий результат останется."
                ))
                .clicked()
            {
                action = Some(HistoryAction::Clear);
            }
        });
        if self.history.is_empty() {
            ui.weak(RichText::new(tr!("History is empty.", "История пуста.")).size(10.5));
        }
        // One compact row remains reachable in a genuinely undersized window.
        // Otherwise the session consumes only the space left under the form.
        let height = (content_bottom - ui.cursor().top() - ui.spacing().item_spacing.y - 2.0)
            .clamp(24.0, 160.0);
        let history = style::scroll_area(
            ui,
            egui::ScrollArea::vertical()
                .id_salt("history")
                .max_height(height)
                .min_scrolled_height(0.0)
                .auto_shrink([false, true]),
            |ui, _| {
                for entry in &self.history {
                    // A new generation prepends a row. Keep a pressed button
                    // bound to its value, not to a row index that can shift.
                    ui.push_id(Rc::as_ptr(entry) as usize, |ui| {
                        let meta = &entry.metadata;
                        egui::Frame::NONE.stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .inner_margin(egui::Margin::symmetric(6, 0)).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let value_width = (ui.available_width() - 84.0 - ui.spacing().item_spacing.x).max(20.0);
                                    // The row placeholder has one block per
                                    // visible character, so a 4-digit PIN and
                                    // a 64-character token look different.
                                    let value: Cow<'_, str> = if self.masked {
                                        Cow::Owned(mask_placeholder(meta.visible_length))
                                    } else {
                                        Cow::Borrowed(entry.secret.as_str())
                                    };
                                    let mut text = egui::text::LayoutJob::default();
                                    text.append(&format!("{}  ", meta.mode.label()), 0.0, egui::TextFormat {
                                        font_id: FontId::new(10.0, egui::FontFamily::Name("field".into())),
                                        color: ui.visuals().weak_text_color(), ..Default::default()
                                    });
                                    text.append(value.as_ref(), 0.0, egui::TextFormat {
                                        font_id: FontId::new(12.0, egui::FontFamily::Name("field".into())),
                                        color: ui.visuals().text_color(), ..Default::default()
                                    });
                                    // The tooltip body is built only when it is
                                    // actually shown: with 80 rows this used to
                                    // allocate two strings per row on every frame.
                                    ui.add_sized([value_width, 20.0], egui::Label::new(text)
                                        .truncate().halign(Align::Min).selectable(false))
                                        .on_hover_ui(|ui| {
                                            let hidden = if meta.hidden_count > 0 {
                                                tr_format!(
                                                    " · +{} hidden",
                                                    " · +{} скрытых",
                                                    meta.hidden_count
                                                )
                                            } else {
                                                String::new()
                                            };
                                            let bits = tr!("bits", "бит");
                                            ui.label(format!(
                                                "{} · {} · {:.0} {bits}{hidden}",
                                                meta.mode.label(),
                                                char_count(meta.visible_length),
                                                meta.entropy_bits
                                            ));
                                        });
                                    if ui.add_sized([84.0, 20.0], style::small_button(tr!("Copy", "Копировать")))
                                        .on_hover_text(tr!(
                                            "Copy this value in full, including hidden characters. Clipboard auto-clear after 30 seconds.",
                                            "Скопировать это значение целиком, включая скрытые символы. Автоочистка буфера — через 30 секунд."
                                        ))
                                        .clicked() && action.is_none() {
                                        action = Some(HistoryAction::Copy(Rc::clone(entry)));
                                    }
                                });
                            });
                    });
                }
            },
        );
        #[cfg(test)]
        record_scroll(ui.ctx(), "history", &history);
        #[cfg(not(test))]
        let _ = history;
        action
    }

    fn resize_edges(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let rect = ctx.input(|i| i.viewport_rect());
        let Some(pos) = ctx.pointer_hover_pos() else {
            return;
        };
        let margin = 5.0;
        let left = pos.x < rect.left() + margin;
        let right = pos.x > rect.right() - margin;
        let top = pos.y < rect.top() + margin;
        let bottom = pos.y > rect.bottom() - margin;
        use egui::viewport::ResizeDirection as D;
        let direction = match (left, right, top, bottom) {
            (true, _, true, _) => Some(D::NorthWest),
            (_, true, true, _) => Some(D::NorthEast),
            (true, _, _, true) => Some(D::SouthWest),
            (_, true, _, true) => Some(D::SouthEast),
            (true, _, _, _) => Some(D::West),
            (_, true, _, _) => Some(D::East),
            (_, _, true, _) => Some(D::North),
            (_, _, _, true) => Some(D::South),
            _ => None,
        };
        if let Some(direction) = direction {
            let cursor = match direction {
                D::North | D::South => egui::CursorIcon::ResizeVertical,
                D::East | D::West => egui::CursorIcon::ResizeHorizontal,
                D::NorthEast | D::SouthWest => egui::CursorIcon::ResizeNeSw,
                D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
            };
            ctx.set_cursor_icon(cursor);
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
    }

    /// Only for the separate screenshot example: no real secret in screenshots.
    #[cfg(any(test, feature = "preview"))]
    pub fn preview(ctx: &egui::Context, mode: Mode, dark: bool) -> Self {
        let mut app = Self::new(ctx);
        app.mode = mode;
        app.dark = dark;
        style::setup_theme(ctx, dark);
        let secret = match mode {
            Mode::Password => "m7#Qx2@Lp9!Vn4$Rt8&Yb6+Df3=Kz5?W".to_owned(),
            Mode::Pin => "7042".to_owned(),
            Mode::Hex => "0123456789abcdef".repeat(4),
            Mode::Url => "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".to_owned(),
            Mode::Fernet => "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=".to_owned(),
        };
        app.result = Some(Rc::new(Generated {
            metadata: generator::Metadata {
                mode,
                visible_length: secret.len(),
                hidden_count: 0,
                source_bytes: if matches!(mode, Mode::Password | Mode::Pin) {
                    None
                } else {
                    Some(32)
                },
                entropy_bits: if mode == Mode::Pin {
                    4.0 * 10_f64.log2()
                } else if mode == Mode::Password {
                    190.0
                } else {
                    256.0
                },
            },
            secret: Zeroizing::new(secret),
        }));
        app.representation = app.result.as_ref().unwrap().representation();
        app.history.clear();
        app.history
            .push_front(Rc::clone(app.result.as_ref().unwrap()));
        app.status = tr!(
            "Interface demo / do not use this secret.",
            "Демонстрация интерфейса / не используйте этот секрет."
        )
        .into();
        app
    }

    /// Only the screenshot example calls this: it selects the language after
    /// `preview` has created the demo state.
    #[cfg(feature = "preview")]
    pub fn set_preview_language(&mut self, language: crate::i18n::Language) {
        i18n::set_language(language);
        self.status = tr!(
            "Interface demo / do not use this secret.",
            "Демонстрация интерфейса / не используйте этот секрет."
        )
        .into();
    }

    /// Screenshots show the settled mask, not a mid-animation frame.
    #[cfg(feature = "preview")]
    pub fn set_preview_masked(&mut self, masked: bool) {
        self.masked = masked;
        self.pixel_generation = self.generation;
        self.pixel_started = -10.0;
    }

    #[cfg(feature = "preview")]
    pub fn prepare_history_preview(&mut self, count: usize, expanded: bool) {
        self.preview_history_open = expanded;
        let current = self.result.as_ref().unwrap();
        self.history.clear();
        self.history.push_front(Rc::clone(current));
        for index in 1..count.clamp(1, MAX_HISTORY) {
            let mut secret = current.secret.clone();
            // Demo values can be one byte long (e.g. a 1-character password);
            // never split a char boundary or index past the string.
            if secret.len() >= 2 && secret.is_char_boundary(2) {
                secret.replace_range(0..2, &format!("{index:02}"));
            }
            self.history.push_back(Rc::new(Generated {
                secret,
                metadata: current.metadata.clone(),
            }));
        }
    }
}

impl eframe::App for GlyphApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}

fn mode_description(mode: Mode) -> &'static str {
    match mode {
        Mode::Password => tr!(
            "Letters, digits and special characters. Adjustable length and compatibility.",
            "Буквы, цифры, спецсимволы. Настраиваемая длина и совместимость."
        ),
        Mode::Pin => tr!(
            "Numeric PIN. Digits only, no hidden characters.",
            "Числовой PIN. Только цифры, без скрытых символов."
        ),
        Mode::Hex => tr!(
            "Equivalent of openssl rand -hex N. Each byte becomes two characters 0–9, a–f.",
            "Аналог openssl rand -hex N. Каждый байт записывается двумя символами 0–9, a–f."
        ),
        Mode::Url => tr!(
            "Equivalent of secrets.token_urlsafe(N). Base64url without “=” for links and API tokens.",
            "Аналог secrets.token_urlsafe(N). Base64url без «=» — для ссылок и API-токенов."
        ),
        Mode::Fernet => tr!(
            "Python cryptography.fernet key: 32 random bytes in base64url with “=”.",
            "Ключ Python cryptography.fernet: 32 случайных байта в base64url с «=»."
        ),
    }
}

fn mask_placeholder(length: usize) -> String {
    const SHADES: [char; 3] = ['▓', '▒', '░'];
    (0..length.clamp(1, 48))
        .map(|index| SHADES[index % SHADES.len()])
        .collect()
}

fn pixel_hash(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^= value >> 33;
    value
}

fn char_count(count: usize) -> String {
    if !i18n::is_russian() {
        let word = if count == 1 { "char" } else { "chars" };
        return format!("{count} {word}");
    }
    let word = if (11..=14).contains(&(count % 100)) {
        "символов"
    } else {
        match count % 10 {
            1 => "символ",
            2..=4 => "символа",
            _ => "символов",
        }
    };
    format!("{count} {word}")
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct ScrollMetrics {
    viewport: egui::Rect,
    content: egui::Vec2,
    offset: egui::Vec2,
}

#[cfg(test)]
fn record_scroll<R>(
    ctx: &egui::Context,
    name: &'static str,
    output: &egui::scroll_area::ScrollAreaOutput<R>,
) {
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(("scroll-metrics", name)),
            ScrollMetrics {
                viewport: output.inner_rect,
                content: output.content_size,
                offset: output.state.offset,
            },
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Language, with_language};

    #[test]
    fn switching_modes_generates_the_actual_selected_format() {
        let mut app = GlyphApp::new(&egui::Context::default());
        for mode in Mode::ALL {
            app.set_mode(mode);
            assert_eq!(app.result.as_ref().unwrap().metadata.mode, mode);
        }
    }
    #[test]
    fn argon2_disables_hidden_and_does_not_restore_them_implicitly() {
        let mut app = GlyphApp::new(&egui::Context::default());
        app.hidden_enabled = true;
        app.generate();
        assert!(app.result.as_ref().unwrap().metadata.hidden_count > 0);
        app.set_argon2(true);
        app.generate();
        assert!(!app.hidden_enabled);
        assert_eq!(app.result.as_ref().unwrap().metadata.hidden_count, 0);
        app.set_argon2(false);
        assert!(!app.hidden_enabled);
    }
    #[test]
    fn history_is_bounded_and_evicted_secrets_are_released() {
        let mut app = GlyphApp::new(&egui::Context::default());
        let original = Rc::downgrade(app.result.as_ref().unwrap());
        for _ in 0..100 {
            app.generate();
        }
        assert_eq!(app.history.len(), MAX_HISTORY);
        assert_eq!(app.history[0].metadata.visible_length, 32);
        assert!(Rc::ptr_eq(&app.history[0], app.result.as_ref().unwrap()));
        assert!(original.upgrade().is_none());
    }

    fn history_frame(
        app: &GlyphApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> (Vec<egui::epaint::ClippedShape>, Option<HistoryAction>) {
        let mut action = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                [480.0, 600.0].into(),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let bottom = ui.max_rect().bottom();
                action = app.history_contents(ui, bottom);
            });
        });
        output.textures_delta.clear();
        (output.shapes, action)
    }

    fn history_caption_position(
        app: &GlyphApp,
        ctx: &egui::Context,
        caption: &str,
        index: usize,
    ) -> egui::Pos2 {
        let (shapes, _) = history_frame(app, ctx, Vec::new());
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == caption => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .nth(index)
            .expect("requested history button must be painted")
    }

    fn click_history_caption(
        app: &GlyphApp,
        ctx: &egui::Context,
        caption: &str,
        index: usize,
    ) -> HistoryAction {
        let target = history_caption_position(app, ctx, caption, index);
        let pointer = |pressed| egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        history_frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(target), pointer(true)],
        );
        history_frame(app, ctx, vec![pointer(false)])
            .1
            .expect("history button must return an action")
    }

    #[test]
    fn history_buttons_recover_exact_previous_values_in_every_mode() {
        for mode in Mode::ALL {
            let ctx = egui::Context::default();
            let mut app = GlyphApp::new(&ctx);
            app.mode = mode;
            app.hex_bytes = 1024;
            app.url_bytes = 1024;
            app.hidden_enabled = true;
            app.generate();
            let archived = Rc::clone(app.result.as_ref().unwrap());
            app.generate();
            app.dirty = true; // Explicit history copies do not depend on current parameters.
            let current = Rc::clone(app.result.as_ref().unwrap());
            let history_len = app.history.len();
            let action = click_history_caption(&app, &ctx, "Копировать", 1);
            let HistoryAction::Copy(value) = action else {
                panic!("wrong history action");
            };
            assert!(Rc::ptr_eq(&value, &archived));
            assert!(value.secret.as_bytes() == archived.secret.as_bytes());
            assert!(Rc::ptr_eq(app.result.as_ref().unwrap(), &current));
            assert_eq!(app.history.len(), history_len);
            if mode == Mode::Password {
                assert_eq!(value.metadata.hidden_count, 6);
                assert!(
                    value
                        .secret
                        .chars()
                        .any(|ch| matches!(ch, '\u{200b}' | '\u{200c}' | '\u{200d}'))
                );
            }
            if mode == Mode::Hex {
                assert_eq!(value.secret.len(), 2048);
            }
            assert!(
                app.clipboard.next_check().is_none(),
                "widget tests must not modify the system clipboard"
            );
        }
    }

    #[test]
    fn generation_during_a_history_click_never_substitutes_another_secret() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::new(&ctx);
        app.generate();
        let intended = Rc::clone(&app.history[1]);
        let target = history_caption_position(&app, &ctx, "Копировать", 1);
        let pointer = |pressed| egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        history_frame(
            &app,
            &ctx,
            vec![egui::Event::PointerMoved(target), pointer(true)],
        );
        app.generate(); // Ctrl+Enter can prepend a row while a mouse button is held.
        let (_, action) = history_frame(&app, &ctx, vec![pointer(false)]);
        match action {
            Some(HistoryAction::Copy(value)) => assert!(Rc::ptr_eq(&value, &intended)),
            None => {} // A moved button may cancel the click instead.
            Some(HistoryAction::Clear) => panic!("history copy changed into clear"),
        }
    }

    #[test]
    fn clearing_session_releases_old_values_without_removing_current_result() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::new(&ctx);
        let old = Rc::downgrade(app.result.as_ref().unwrap());
        app.generate();
        let current = Rc::downgrade(app.result.as_ref().unwrap());
        let action = click_history_caption(&app, &ctx, "Очистить", 0);
        assert!(matches!(&action, HistoryAction::Clear));
        app.apply_history_action(action, &ctx);
        assert!(app.history.is_empty());
        assert!(old.upgrade().is_none());
        assert!(current.upgrade().is_some());
        assert!(app.result.is_some());
        assert!(!app.status_error);
    }

    #[test]
    fn hidden_mode_masks_all_session_values_but_copy_still_uses_original() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
        app.generate();
        app.masked = true;
        let (shapes, _) = history_frame(&app, &ctx, Vec::new());
        for shape in shapes {
            if let egui::epaint::Shape::Text(text) = shape.shape {
                for entry in &app.history {
                    assert!(!text.galley.text().contains(entry.secret.as_str()));
                }
            }
        }
        let HistoryAction::Copy(value) = click_history_caption(&app, &ctx, "Копировать", 1)
        else {
            panic!("expected history copy");
        };
        assert!(Rc::ptr_eq(&value, &app.history[1]));
    }

    #[test]
    fn new_session_starts_without_previous_session_history() {
        let ctx = egui::Context::default();
        let previous = {
            let mut app = GlyphApp::new(&ctx);
            app.generate();
            Rc::downgrade(app.result.as_ref().unwrap())
        };
        assert!(previous.upgrade().is_none());
        let next = GlyphApp::new(&egui::Context::default());
        assert_eq!(
            next.history.len(),
            1,
            "only the new startup result may be retained"
        );
        assert!(Rc::ptr_eq(&next.history[0], next.result.as_ref().unwrap()));
    }
    #[test]
    fn invalid_generation_discards_old_result() {
        let mut app = GlyphApp::new(&egui::Context::default());
        app.options.length = 0;
        app.generate();
        assert!(app.status_error);
        assert!(app.result.is_none());
        assert!(app.representation.is_empty());
    }

    #[test]
    fn previous_parameters_cannot_be_copied_as_a_new_result() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::new(&ctx);
        app.dirty = true;
        app.copy(&ctx);
        assert!(app.clipboard.next_check().is_none());
        assert!(
            app.status.contains("Создать"),
            "a blocked copy must explain why nothing happened"
        );
        assert!(!app.status_error);
        app.generate();
        assert!(!app.dirty);
    }
    #[test]
    fn russian_character_counts() {
        for (count, expected) in [
            (1, "1 символ"),
            (4, "4 символа"),
            (11, "11 символов"),
            (21, "21 символ"),
            (32, "32 символа"),
        ] {
            assert_eq!(char_count(count), expected);
        }
    }

    #[test]
    fn english_character_counts() {
        with_language(Language::En, || {
            assert_eq!(char_count(1), "1 char");
            assert_eq!(char_count(0), "0 chars");
            assert_eq!(char_count(32), "32 chars");
        });
    }

    #[test]
    fn english_interface_has_no_russian_controls_or_local_badge() {
        with_language(Language::En, || {
            let ctx = egui::Context::default();
            let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
            app.generate();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    style::WINDOW_SIZE.into(),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.render(ui));
            // Clear before asserting: epaint aborts the process if a failed
            // test drops unapplied texture deltas.
            output.textures_delta.clear();
            let text: Vec<String> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            for expected in [
                "Password",
                "↻ Create",
                "Copy",
                "Length",
                "Min digits",
                "RU",
                "light",
            ] {
                assert!(
                    text.iter().any(|line| line == expected),
                    "missing English control {expected:?}: {text:?}"
                );
            }
            assert!(
                text.iter().any(|line| line.contains("settings")),
                "settings tag must be translated"
            );
            assert!(
                text.iter().any(|line| line.contains("Session /")),
                "session header must be translated"
            );
            for forbidden in ["Пароль", "Создать", "Копировать", "локально", "бит"]
            {
                assert!(
                    !text.iter().any(|line| line.contains(forbidden)),
                    "Russian text {forbidden:?} leaked into the English UI"
                );
            }
            assert!(
                !text.iter().any(|line| line.trim() == "● локально"),
                "the removed local badge must not be painted"
            );
        });
    }

    #[test]
    fn shortcut_generates_once_and_hidden_logic_does_not_repeat_it() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::new(&ctx);
        let initial = app.history.len();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                style::WINDOW_SIZE.into(),
            )),
            events: vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        output.textures_delta.clear();
        assert_eq!(app.history.len(), initial + 1);
        for _ in 0..3 {
            let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| app.tick(ctx));
        }
        assert_eq!(app.history.len(), initial + 1);
    }

    #[test]
    fn masking_never_paints_the_original_secret() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
        app.masked = true;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                style::WINDOW_SIZE.into(),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        let secret = &app.result.as_ref().unwrap().secret;
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                assert!(!text.galley.job.text.contains(secret.as_str()));
            }
        }
        output.textures_delta.clear();
    }

    #[test]
    fn masked_pixels_animate_then_settle_without_encoding_the_secret() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
        app.masked = true;
        let render = |app: &mut GlyphApp, time: f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        style::WINDOW_SIZE.into(),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            output.shapes
        };
        let pixels = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Rect(rect)
                        if (rect.rect.width() - 5.0).abs() < 0.01
                            && (rect.rect.height() - 5.0).abs() < 0.01 =>
                    {
                        Some((rect.rect.min, rect.fill))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let first = render(&mut app, 0.0);
        let first_pixels = pixels(&first);
        assert!(
            first_pixels.len() >= 4 * 8 * 3 && first_pixels.len() % 24 == 0,
            "expected whole pixel groups, got {} cells",
            first_pixels.len()
        );
        let mid = render(&mut app, 0.10);
        let mid_pixels = pixels(&mid);
        assert_eq!(mid_pixels.len(), first_pixels.len());
        assert_ne!(first_pixels, mid_pixels, "the mask must animate");
        let settled_a = render(&mut app, 1.0);
        let settled_b = render(&mut app, 1.1);
        assert_eq!(
            pixels(&settled_a),
            pixels(&settled_b),
            "the mask must settle"
        );
        for shape in &settled_a {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                assert!(
                    !text.galley.text().contains("m7#Qx2"),
                    "the pixel mask must not reveal the secret"
                );
            }
        }

        // A shorter value paints a shorter strip: one column per character.
        let mut metadata = app.result.as_ref().unwrap().metadata.clone();
        metadata.visible_length = 16;
        app.result = Some(Rc::new(Generated {
            secret: Zeroizing::new("x".repeat(16)),
            metadata,
        }));
        app.generation = app.generation.wrapping_add(1);
        let shorter = render(&mut app, 1.2);
        assert_eq!(
            pixels(&shorter).len(),
            16 * 3,
            "one pixel column per visible character"
        );
    }

    #[test]
    fn history_placeholder_follows_visible_length() {
        assert_eq!(mask_placeholder(4).chars().count(), 4);
        assert_eq!(mask_placeholder(32).chars().count(), 32);
        assert_eq!(mask_placeholder(0).chars().count(), 1);
        assert_eq!(mask_placeholder(256).chars().count(), 48);
    }

    #[test]
    fn actual_controls_use_the_button_face_not_the_lowered_body_face() {
        let ctx = egui::Context::default();
        let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                style::WINDOW_SIZE.into(),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        let captions = [
            "Пароль",
            "PIN",
            "HEX",
            "URL-safe",
            "Fernet",
            "↻ Создать",
            "Копировать",
            "скрыть",
            "день",
            "EN",
        ];
        let mut checked = 0;
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && captions.contains(&text.galley.text())
            {
                for section in &text.galley.job.sections {
                    assert_eq!(
                        section.format.font_id.family,
                        egui::FontFamily::Name("button".into()),
                        "{}",
                        text.galley.text()
                    );
                }
                checked += 1;
            }
        }
        output.textures_delta.clear();
        assert_eq!(checked, captions.len());
    }

    fn app_frame(
        app: &mut GlyphApp,
        ctx: &egui::Context,
        size: [f32; 2],
        frame: usize,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
                time: Some(frame as f64 * 0.05),
                events,
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        output.textures_delta.clear();
        output.shapes
    }

    fn primary_positions(shapes: &[egui::epaint::ClippedShape]) -> Vec<(&'static str, egui::Pos2)> {
        [
            "Пароль",
            "PIN",
            "HEX",
            "URL-safe",
            "Fernet",
            "↻ Создать",
            "Копировать",
            "Длина",
            "Минимум цифр",
        ]
        .into_iter()
        .map(|caption| {
            let position = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == caption => {
                        Some(text.pos)
                    }
                    _ => None,
                })
                .expect("primary control must be painted");
            (caption, position)
        })
        .collect()
    }

    #[test]
    fn opening_session_never_reflows_primary_controls() {
        for size in [style::WINDOW_SIZE, [544.0, 780.0], [960.0, 900.0]] {
            for scale in [1.0, 1.25, 1.5, 2.0] {
                for count in [6, MAX_HISTORY] {
                    let ctx = egui::Context::default();
                    let mut app = GlyphApp::preview(&ctx, Mode::Password, true);
                    for _ in 1..count {
                        app.generate();
                    }
                    ctx.set_pixels_per_point(scale);
                    let mut shapes = Vec::new();
                    for frame in 0..8 {
                        shapes = app_frame(&mut app, &ctx, size, frame, Vec::new());
                    }
                    let baseline = primary_positions(&shapes);
                    let verify = |shapes: &[egui::epaint::ClippedShape]| {
                        let main: ScrollMetrics = ctx
                            .data(|data| data.get_temp(egui::Id::new(("scroll-metrics", "main"))))
                            .unwrap();
                        assert!(
                            main.content.y <= main.viewport.height() + 0.5,
                            "session caused outer scrolling: {} > {} at {size:?}, {scale}x, {count} values",
                            main.content.y,
                            main.viewport.height()
                        );
                        assert!(
                            main.offset.y.abs() <= 0.01,
                            "outer offset {:?}, content {:?}, viewport {:?} at {size:?}, {scale}x, {count} values",
                            main.offset,
                            main.content,
                            main.viewport
                        );
                        for ((caption, before), (_, after)) in
                            baseline.iter().zip(primary_positions(shapes))
                        {
                            assert!(
                                before.distance(after) <= 0.01,
                                "{caption} moved from {before:?} to {after:?} at {size:?}, {scale}x, {count} values"
                            );
                        }
                    };
                    let header = format!("Сеанс / {count}");
                    let target = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(text) if text.galley.text() == header => {
                                Some(text.pos + text.galley.rect.center().to_vec2())
                            }
                            _ => None,
                        })
                        .expect("session header must be painted");
                    let pointer = |pressed| egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    };
                    app_frame(
                        &mut app,
                        &ctx,
                        size,
                        8,
                        vec![egui::Event::PointerMoved(target), pointer(true)],
                    );
                    app_frame(&mut app, &ctx, size, 9, vec![pointer(false)]);
                    for frame in 10..24 {
                        let shapes = app_frame(&mut app, &ctx, size, frame, Vec::new());
                        verify(&shapes);
                    }
                    let history: ScrollMetrics = ctx
                        .data(|data| data.get_temp(egui::Id::new(("scroll-metrics", "history"))))
                        .unwrap();
                    assert!(
                        history.viewport.height() >= 23.5,
                        "the session must show a complete copy row at {size:?}, {scale}x: {:?}",
                        history.viewport
                    );
                    app_frame(
                        &mut app,
                        &ctx,
                        size,
                        24,
                        vec![
                            egui::Event::PointerMoved(history.viewport.center()),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(0.0, -400.0),
                                phase: egui::TouchPhase::Move,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                    for frame in 25..36 {
                        verify(&app_frame(&mut app, &ctx, size, frame, Vec::new()));
                    }
                    let history: ScrollMetrics = ctx
                        .data(|data| data.get_temp(egui::Id::new(("scroll-metrics", "history"))))
                        .unwrap();
                    assert!(history.offset.y > 0.0, "history must remain scrollable");
                    app_frame(
                        &mut app,
                        &ctx,
                        size,
                        36,
                        vec![egui::Event::PointerMoved(target), pointer(true)],
                    );
                    app_frame(&mut app, &ctx, size, 37, vec![pointer(false)]);
                    for frame in 38..52 {
                        verify(&app_frame(&mut app, &ctx, size, frame, Vec::new()));
                    }
                }
            }
        }
    }
    #[test]
    fn renders_every_mode_at_small_default_and_wide_sizes_in_both_themes_and_languages() {
        for language in [Language::En, Language::Ru] {
            with_language(language, || {
                for size in [style::MIN_SIZE, style::WINDOW_SIZE, [960.0, 900.0]] {
                    for dark in [true, false] {
                        for mode in Mode::ALL {
                            let ctx = egui::Context::default();
                            let mut app = GlyphApp::preview(&ctx, mode, dark);
                            let input = egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    size.into(),
                                )),
                                ..Default::default()
                            };
                            let mut output = ctx.run_ui(input, |ui| app.render(ui));
                            assert!(!output.shapes.is_empty());
                            assert!(
                                output.platform_output.commands.is_empty(),
                                "render must not copy secrets or open URLs"
                            );
                            output.textures_delta.clear();
                        }
                    }
                }
            });
        }
    }
}
