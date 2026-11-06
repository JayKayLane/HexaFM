//! Flat, calm colour schemes. "DX Classic" takes its cues from the 1983 hardware livery:
//! warm near-black body, cream lettering, teal and burgundy switch colours, green-lit display.

use egui::{self, Color32, CornerRadius, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeId {
    DxClassic,
    Graphite,
    Daylight,
}

impl ThemeId {
    pub const ALL: [ThemeId; 3] = [ThemeId::DxClassic, ThemeId::Graphite, ThemeId::Daylight];
    pub fn label(self) -> &'static str {
        match self {
            ThemeId::DxClassic => "DX Classic",
            ThemeId::Graphite => "Graphite",
            ThemeId::Daylight => "Daylight",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: Color32,       // window body
    pub panel: Color32,    // side panels
    pub surface: Color32,  // cards
    pub raised: Color32,   // controls
    pub hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,   // teal switch colour
    pub accent2: Color32,  // burgundy switch colour
    pub accent_text: Color32,
    pub lcd_bg: Color32,
    pub lcd_text: Color32,
    pub lcd_dim: Color32,
    pub star: Color32,
    pub danger: Color32,
    pub key_white: Color32,
    pub key_black: Color32,
}

impl Theme {
    pub fn get(id: ThemeId) -> Theme {
        match id {
            ThemeId::DxClassic => Theme {
                dark: true,
                bg: hex(0x1b1917),
                panel: hex(0x161412),
                surface: hex(0x221f1c),
                raised: hex(0x2c2824),
                hover: hex(0x36312c),
                border: hex(0x3a342f),
                text: hex(0xebe4d3),
                text_dim: hex(0xa79e8d),
                text_faint: hex(0x716a5e),
                accent: hex(0x4a9e8f),
                accent2: hex(0x9a3a48),
                accent_text: hex(0x101010),
                lcd_bg: hex(0x1a2214),
                lcd_text: hex(0xbcd47a),
                lcd_dim: hex(0x6c7d45),
                star: hex(0xd9a441),
                danger: hex(0xc0504d),
                key_white: hex(0xe9e3d4),
                key_black: hex(0x24201d),
            },
            ThemeId::Graphite => Theme {
                dark: true,
                bg: hex(0x17191c),
                panel: hex(0x131518),
                surface: hex(0x1e2125),
                raised: hex(0x272b30),
                hover: hex(0x30353b),
                border: hex(0x33383e),
                text: hex(0xe6e8eb),
                text_dim: hex(0x9aa1a9),
                text_faint: hex(0x666d75),
                accent: hex(0xd9a441),
                accent2: hex(0x5b8fb9),
                accent_text: hex(0x15120c),
                lcd_bg: hex(0x101214),
                lcd_text: hex(0xe8c27a),
                lcd_dim: hex(0x7a6a48),
                star: hex(0xd9a441),
                danger: hex(0xd0605a),
                key_white: hex(0xdfe2e6),
                key_black: hex(0x1d2024),
            },
            ThemeId::Daylight => Theme {
                dark: false,
                bg: hex(0xf2efe8),
                panel: hex(0xe8e4db),
                surface: hex(0xfaf8f3),
                raised: hex(0xe2ddd2),
                hover: hex(0xd8d2c5),
                border: hex(0xcfc8ba),
                text: hex(0x24211d),
                text_dim: hex(0x6b6458),
                text_faint: hex(0x9a9284),
                accent: hex(0x2f7f72),
                accent2: hex(0x8f2f3d),
                accent_text: hex(0xffffff),
                lcd_bg: hex(0x2a3320),
                lcd_text: hex(0xc8de85),
                lcd_dim: hex(0x75864c),
                star: hex(0xb7801f),
                danger: hex(0xb4433f),
                key_white: hex(0xfdfcf9),
                key_black: hex(0x2b2723),
            },
        }
    }

    pub fn apply(&self, ctx: &egui::Context, scale: f32) {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = self.bg;
        v.window_fill = self.surface;
        v.extreme_bg_color = self.panel;
        v.faint_bg_color = self.surface;
        v.code_bg_color = self.panel;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = CornerRadius::same(6);
        v.menu_corner_radius = CornerRadius::same(5);
        v.window_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) };
        v.popup_shadow = egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(70) };
        v.selection.bg_fill = self.accent.linear_multiply(0.45);
        v.selection.stroke = Stroke::new(1.0, self.text);
        v.hyperlink_color = self.accent;
        v.override_text_color = Some(self.text);
        v.striped = false;
        v.slider_trailing_fill = true;
        let r = CornerRadius::same(4);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.surface;
        w.noninteractive.weak_bg_fill = self.surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text_dim);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = self.raised;
        w.inactive.weak_bg_fill = self.raised;
        w.inactive.bg_stroke = Stroke::new(1.0, self.border);
        w.inactive.fg_stroke = Stroke::new(1.0, self.text);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = self.hover;
        w.hovered.weak_bg_fill = self.hover;
        w.hovered.bg_stroke = Stroke::new(1.0, self.text_faint);
        w.hovered.fg_stroke = Stroke::new(1.0, self.text);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = self.accent.linear_multiply(0.6);
        w.active.weak_bg_fill = self.accent.linear_multiply(0.6);
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        w.active.fg_stroke = Stroke::new(1.0, self.text);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;
        w.open.bg_fill = self.hover;
        w.open.weak_bg_fill = self.hover;
        w.open.corner_radius = r;
        let which = if self.dark { egui::Theme::Dark } else { egui::Theme::Light };
        ctx.options_mut(|o| o.theme_preference = if self.dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        ctx.set_theme(which);

        let mut style = (*ctx.style_of(which)).clone();
        style.visuals = v;
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = fs(24.0);
        style.spacing.slider_width = 140.0;
        style.spacing.combo_width = 120.0;
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(fs(11.0), FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(fs(13.5), FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(fs(13.0), FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(fs(18.0), FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(fs(13.0), FontFamily::Monospace)),
        ]
        .into();
        ctx.set_style_of(egui::Theme::Dark, style.clone());
        ctx.set_style_of(egui::Theme::Light, style);
        ctx.set_zoom_factor(scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX));
    }
}

pub const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn category_color(t: &Theme, c: crate::library::Category) -> Color32 {
    use crate::library::Category::*;
    // muted, evenly spaced hues so tags read as labels, not decoration
    let base = match c {
        Piano => 0x8fa3b8,
        Keys => 0x6fa8a0,
        Organ => 0xb59a6a,
        Bass => 0x9a7fb5,
        Brass => 0xc4925a,
        Strings => 0xb57f7f,
        Pad => 0x7f9fc4,
        Lead => 0xc47f9a,
        Bells => 0xa8b56f,
        Mallets => 0x8fb58a,
        Plucked => 0xb5a36f,
        Wind => 0x7fb5b0,
        Voice => 0xb58fb0,
        Percussion => 0xa0a0a0,
        Effects => 0x8a8fb5,
        Other => 0x8a857c,
    };
    let col = hex(base);
    if t.dark { col } else { col.gamma_multiply(0.75) }
}

// ---------------------------------------------------------------- text size

/// One factor for all text. Every label keeps its own base size and is multiplied by this,
/// so headings, labels and small print stay in proportion.
static TEXT_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000); // 1.0

pub fn set_text_scale(s: f32) {
    TEXT_SCALE.store(s.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn text_scale() -> f32 {
    f32::from_bits(TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Scaled text size.
pub fn fs(size: f32) -> f32 {
    size * text_scale()
}

pub fn fp(size: f32) -> egui::FontId {
    egui::FontId::proportional(fs(size))
}

pub fn fm(size: f32) -> egui::FontId {
    egui::FontId::monospace(fs(size))
}

/// Interface scale limits, as the original editor (DexedAudioProcessorEditor::resetSize 0x1000662e0
/// clamps its scale to 0.8 .. 1.6).
pub const UI_SCALE_MIN: f32 = 0.8;
pub const UI_SCALE_MAX: f32 = 1.6;
