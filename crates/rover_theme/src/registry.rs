use crate::{Appearance, Theme, ThemeFamilyContent};
use anyhow::{Context as _, Result};
use gpui::{App, Global};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Global theme registry holding all loaded themes.
pub struct ThemeRegistry {
    themes: HashMap<String, Arc<Theme>>,
}

impl Global for ThemeRegistry {}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self {
            themes: HashMap::new(),
        }
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn get(&self, name: &str) -> Option<Arc<Theme>> {
        self.themes.get(name).cloned()
    }

    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.themes.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn insert(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), Arc::new(theme));
    }

    pub fn load_family(&mut self, content: &ThemeFamilyContent) {
        for theme_content in &content.themes {
            let theme = Theme::from_content(theme_content);
            self.insert(theme);
        }
    }

    pub fn load_bundled(&mut self) {
        let bundled = include_str!("../../../assets/themes/rover-dark.json");
        if let Ok(family) = serde_json::from_str::<ThemeFamilyContent>(bundled) {
            self.load_family(&family);
        }
    }

    pub fn load_user_themes(&mut self) -> Result<()> {
        let theme_dir = user_theme_dir();
        if !theme_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&theme_dir)
            .with_context(|| format!("reading theme dir {:?}", theme_dir))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading theme {:?}", path))?;
                if let Ok(family) = serde_json::from_str::<ThemeFamilyContent>(&content) {
                    self.load_family(&family);
                } else {
                    log::warn!("Failed to parse theme file: {:?}", path);
                }
            }
        }
        Ok(())
    }
}

/// The active theme, stored as a global.
pub struct ActiveTheme {
    pub theme: Arc<Theme>,
}

impl Global for ActiveTheme {}

impl ActiveTheme {
    pub fn get(cx: &App) -> &Arc<Theme> {
        &cx.global::<Self>().theme
    }

    pub fn set(theme: Arc<Theme>, cx: &mut App) {
        cx.set_global(Self { theme });
    }
}

/// Convenience: get the active theme from context.
pub fn theme(cx: &App) -> &Arc<Theme> {
    ActiveTheme::get(cx)
}

pub fn user_theme_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rover")
        .join("themes")
}

pub fn init(cx: &mut App) {
    let mut registry = ThemeRegistry::new();
    registry.load_bundled();
    let _ = registry.load_user_themes();

    let default_theme = registry.get("Rover Dark").unwrap_or_else(|| {
        let t = Theme {
            name: "Fallback Dark".into(),
            appearance: Appearance::Dark,
            colors: crate::ThemeColors::dark(),
        };
        Arc::new(t)
    });

    cx.set_global(registry);
    ActiveTheme::set(default_theme, cx);
}
