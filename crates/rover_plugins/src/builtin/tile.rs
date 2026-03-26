//! Tile (Window Management) plugin — exposes tiling commands to move and
//! resize the frontmost window (left half, right half, maximize, etc.).
//!
//! Match mode: `FuzzyCommand` — commands are matched via fuzzy search.

use rover_plugin_api::*;
use std::any::Any;

/// Recognized tiling positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePosition {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
    Center,
}

impl TilePosition {
    fn id(&self) -> &'static str {
        match self {
            Self::LeftHalf => "tile:left_half",
            Self::RightHalf => "tile:right_half",
            Self::TopHalf => "tile:top_half",
            Self::BottomHalf => "tile:bottom_half",
            Self::TopLeft => "tile:top_left",
            Self::TopRight => "tile:top_right",
            Self::BottomLeft => "tile:bottom_left",
            Self::BottomRight => "tile:bottom_right",
            Self::Maximize => "tile:maximize",
            Self::Center => "tile:center",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::LeftHalf => "Tile Left Half",
            Self::RightHalf => "Tile Right Half",
            Self::TopHalf => "Tile Top Half",
            Self::BottomHalf => "Tile Bottom Half",
            Self::TopLeft => "Tile Top Left",
            Self::TopRight => "Tile Top Right",
            Self::BottomLeft => "Tile Bottom Left",
            Self::BottomRight => "Tile Bottom Right",
            Self::Maximize => "Maximize Window",
            Self::Center => "Center Window",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::LeftHalf => "Move window to the left half of the screen",
            Self::RightHalf => "Move window to the right half of the screen",
            Self::TopHalf => "Move window to the top half of the screen",
            Self::BottomHalf => "Move window to the bottom half of the screen",
            Self::TopLeft => "Move window to the top-left quarter",
            Self::TopRight => "Move window to the top-right quarter",
            Self::BottomLeft => "Move window to the bottom-left quarter",
            Self::BottomRight => "Move window to the bottom-right quarter",
            Self::Maximize => "Maximize the frontmost window",
            Self::Center => "Center the frontmost window",
        }
    }

    /// AppleScript to tile the frontmost window to this position.
    fn applescript(&self) -> &'static str {
        match self {
            Self::LeftHalf => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        set _screen to {0, 0, 960, 1080}
                        try
                            set position of window 1 to {item 1 of _screen, item 2 of _screen}
                            set size of window 1 to {item 3 of _screen, item 4 of _screen}
                        end try
                    end tell
                end tell"#
            }
            Self::RightHalf => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {960, 0}
                            set size of window 1 to {960, 1080}
                        end try
                    end tell
                end tell"#
            }
            Self::TopHalf => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {0, 0}
                            set size of window 1 to {1920, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::BottomHalf => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {0, 540}
                            set size of window 1 to {1920, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::TopLeft => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {0, 0}
                            set size of window 1 to {960, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::TopRight => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {960, 0}
                            set size of window 1 to {960, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::BottomLeft => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {0, 540}
                            set size of window 1 to {960, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::BottomRight => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {960, 540}
                            set size of window 1 to {960, 540}
                        end try
                    end tell
                end tell"#
            }
            Self::Maximize => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set position of window 1 to {0, 0}
                            set size of window 1 to {1920, 1080}
                        end try
                    end tell
                end tell"#
            }
            Self::Center => {
                r#"
                tell application "System Events"
                    set _app to name of first application process whose frontmost is true
                end tell
                tell application "System Events"
                    tell process _app
                        try
                            set size of window 1 to {1200, 800}
                            set position of window 1 to {360, 140}
                        end try
                    end tell
                end tell"#
            }
        }
    }

    fn all() -> &'static [TilePosition] {
        &[
            Self::LeftHalf,
            Self::RightHalf,
            Self::TopHalf,
            Self::BottomHalf,
            Self::TopLeft,
            Self::TopRight,
            Self::BottomLeft,
            Self::BottomRight,
            Self::Maximize,
            Self::Center,
        ]
    }

    fn from_id(id: &str) -> Option<Self> {
        TilePosition::all().iter().find(|p| p.id() == id).copied()
    }
}

pub struct TilePlugin;

impl TilePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TilePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TilePlugin {
    fn id(&self) -> &str {
        "builtin.tile"
    }

    fn name(&self) -> &str {
        "Window Tiling"
    }

    fn description(&self) -> &str {
        "Tile and resize the frontmost window"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        8
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        TilePosition::all()
            .iter()
            .map(|pos| CommandDescriptor {
                id: pos.id().to_string(),
                name: pos.name().to_string(),
                description: pos.description().to_string(),
                icon: Some(IconHint::SystemSymbol("rectangle.split.2x1".to_string())),
                keywords: vec![
                    "tile".to_string(),
                    "window".to_string(),
                    "resize".to_string(),
                    "move".to_string(),
                ],
                has_submenu: false,
            })
            .collect()
    }

    fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // FuzzyCommand mode — matched by the registry's fuzzy layer.
        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(pos) = TilePosition::from_id(match_id) {
            let script = pos.applescript();
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .spawn();
            return ConfirmResult::Dismiss;
        }
        ConfirmResult::Stay
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
