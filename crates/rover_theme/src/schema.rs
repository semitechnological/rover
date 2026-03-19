use gpui::Hsla;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A theme family file (mirrors Zed's JSON schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFamilyContent {
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub themes: Vec<ThemeContent>,
}

/// A single theme variant within a family.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeContent {
    pub name: String,
    pub appearance: Appearance,
    pub style: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

impl Default for Appearance {
    fn default() -> Self {
        Self::Dark
    }
}

/// Runtime theme with parsed colors.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub appearance: Appearance,
    pub colors: ThemeColors,
}

/// Core color tokens used throughout the UI.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub background: Hsla,
    pub surface_background: Hsla,
    pub elevated_surface_background: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_placeholder: Hsla,
    pub text_accent: Hsla,
    pub border: Hsla,
    pub border_variant: Hsla,
    pub border_focused: Hsla,
    pub element_background: Hsla,
    pub element_hover: Hsla,
    pub element_selected: Hsla,
    pub icon: Hsla,
    pub icon_muted: Hsla,
    pub scrollbar_thumb: Hsla,
    pub scrollbar_track: Hsla,
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            background: hex_to_hsla("#1e1e2e"),
            surface_background: hex_to_hsla("#181825"),
            elevated_surface_background: hex_to_hsla("#313244"),
            text: hex_to_hsla("#cdd6f4"),
            text_muted: hex_to_hsla("#a6adc8"),
            text_placeholder: hex_to_hsla("#6c7086"),
            text_accent: hex_to_hsla("#89b4fa"),
            border: hex_to_hsla("#45475a"),
            border_variant: hex_to_hsla("#313244"),
            border_focused: hex_to_hsla("#89b4fa"),
            element_background: hex_to_hsla("#313244"),
            element_hover: hex_to_hsla("#45475a"),
            element_selected: hex_to_hsla("#585b70"),
            icon: hex_to_hsla("#cdd6f4"),
            icon_muted: hex_to_hsla("#6c7086"),
            scrollbar_thumb: hex_to_hsla("#45475a"),
            scrollbar_track: hex_to_hsla("#1e1e2e"),
        }
    }

    pub fn light() -> Self {
        Self {
            background: hex_to_hsla("#eff1f5"),
            surface_background: hex_to_hsla("#e6e9ef"),
            elevated_surface_background: hex_to_hsla("#dce0e8"),
            text: hex_to_hsla("#4c4f69"),
            text_muted: hex_to_hsla("#6c6f85"),
            text_placeholder: hex_to_hsla("#9ca0b0"),
            text_accent: hex_to_hsla("#1e66f5"),
            border: hex_to_hsla("#ccd0da"),
            border_variant: hex_to_hsla("#dce0e8"),
            border_focused: hex_to_hsla("#1e66f5"),
            element_background: hex_to_hsla("#dce0e8"),
            element_hover: hex_to_hsla("#ccd0da"),
            element_selected: hex_to_hsla("#bcc0cc"),
            icon: hex_to_hsla("#4c4f69"),
            icon_muted: hex_to_hsla("#9ca0b0"),
            scrollbar_thumb: hex_to_hsla("#ccd0da"),
            scrollbar_track: hex_to_hsla("#eff1f5"),
        }
    }
}

impl Theme {
    /// Refine a ThemeContent into a runtime Theme, applying overrides on top of defaults.
    pub fn from_content(content: &ThemeContent) -> Self {
        let mut colors = match content.appearance {
            Appearance::Dark => ThemeColors::dark(),
            Appearance::Light => ThemeColors::light(),
        };

        // Apply style overrides from JSON
        for (key, value) in &content.style {
            let color = hex_to_hsla(value);
            match key.as_str() {
                "background" => colors.background = color,
                "surface.background" => colors.surface_background = color,
                "elevated_surface.background" => colors.elevated_surface_background = color,
                "text" => colors.text = color,
                "text.muted" => colors.text_muted = color,
                "text.placeholder" => colors.text_placeholder = color,
                "text.accent" => colors.text_accent = color,
                "border" => colors.border = color,
                "border.variant" => colors.border_variant = color,
                "border.focused" => colors.border_focused = color,
                "element.background" => colors.element_background = color,
                "element.hover" => colors.element_hover = color,
                "element.selected" => colors.element_selected = color,
                "icon" => colors.icon = color,
                "icon.muted" => colors.icon_muted = color,
                "scrollbar.thumb" => colors.scrollbar_thumb = color,
                "scrollbar.track" => colors.scrollbar_track = color,
                _ => {}
            }
        }

        Self {
            name: content.name.clone(),
            appearance: content.appearance,
            colors,
        }
    }
}

fn hex_to_hsla(hex: &str) -> Hsla {
    let hex = hex.trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
            (r, g, b, 1.0f32)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0;
            (r, g, b, a)
        }
        _ => (0.0, 0.0, 0.0, 1.0),
    };

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a,
        };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    Hsla {
        h: h / 6.0,
        s,
        l,
        a,
    }
}
