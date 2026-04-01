//! Tile Window Manager plugin — exposes tile_core's window tiling actions
//! as searchable commands in the Rover picker.
//!
//! Uses `tile_core` for action definitions and frame computation, and `tile_ax`
//! for Accessibility API calls to actually move/resize the frontmost window.
//!
//! Gated behind the `tile` feature flag.

use rover_plugin_api::*;
use tile_core::TileAction;

/// Descriptor for a tile command: pairs a display name, match keywords,
/// and the underlying `TileAction`.
struct TileCommand {
    /// Unique id within the plugin, e.g. `"left_half"`.
    id: &'static str,
    /// Display name shown in the picker, e.g. `"Tile Left Half"`.
    name: &'static str,
    /// Short description of what this action does.
    description: &'static str,
    /// Extra keywords for fuzzy matching.
    keywords: &'static [&'static str],
    /// The tile action to execute.
    action: TileAction,
}

/// All tile commands. Each entry maps to a `TileAction` variant.
const TILE_COMMANDS: &[TileCommand] = &[
    // ── Halves ───────────────────────────────────────────────────────
    TileCommand {
        id: "left_half",
        name: "Tile Left Half",
        description: "Move window to fill the left half of the screen",
        keywords: &["left", "half", "split", "snap"],
        action: TileAction::LeftHalf,
    },
    TileCommand {
        id: "right_half",
        name: "Tile Right Half",
        description: "Move window to fill the right half of the screen",
        keywords: &["right", "half", "split", "snap"],
        action: TileAction::RightHalf,
    },
    TileCommand {
        id: "top_half",
        name: "Tile Top Half",
        description: "Move window to fill the top half of the screen",
        keywords: &["top", "half", "upper"],
        action: TileAction::TopHalf,
    },
    TileCommand {
        id: "bottom_half",
        name: "Tile Bottom Half",
        description: "Move window to fill the bottom half of the screen",
        keywords: &["bottom", "half", "lower"],
        action: TileAction::BottomHalf,
    },
    // ── Thirds ───────────────────────────────────────────────────────
    TileCommand {
        id: "left_third",
        name: "Tile Left Third",
        description: "Move window to fill the left third of the screen",
        keywords: &["left", "third"],
        action: TileAction::LeftThird,
    },
    TileCommand {
        id: "center_third",
        name: "Tile Center Third",
        description: "Move window to fill the center third of the screen",
        keywords: &["center", "middle", "third"],
        action: TileAction::CenterThird,
    },
    TileCommand {
        id: "right_third",
        name: "Tile Right Third",
        description: "Move window to fill the right third of the screen",
        keywords: &["right", "third"],
        action: TileAction::RightThird,
    },
    // ── Two-thirds ───────────────────────────────────────────────────
    TileCommand {
        id: "left_two_thirds",
        name: "Tile Left Two Thirds",
        description: "Move window to fill the left two-thirds of the screen",
        keywords: &["left", "two", "thirds", "2/3"],
        action: TileAction::LeftTwoThirds,
    },
    TileCommand {
        id: "center_two_thirds",
        name: "Tile Center Two Thirds",
        description: "Move window to fill the center two-thirds of the screen",
        keywords: &["center", "middle", "two", "thirds", "2/3"],
        action: TileAction::CenterTwoThirds,
    },
    TileCommand {
        id: "right_two_thirds",
        name: "Tile Right Two Thirds",
        description: "Move window to fill the right two-thirds of the screen",
        keywords: &["right", "two", "thirds", "2/3"],
        action: TileAction::RightTwoThirds,
    },
    // ── Quarters ─────────────────────────────────────────────────────
    TileCommand {
        id: "top_left_quarter",
        name: "Tile Top Left Quarter",
        description: "Move window to the top-left quarter of the screen",
        keywords: &["top", "left", "quarter", "corner"],
        action: TileAction::TopLeftQuarter,
    },
    TileCommand {
        id: "top_right_quarter",
        name: "Tile Top Right Quarter",
        description: "Move window to the top-right quarter of the screen",
        keywords: &["top", "right", "quarter", "corner"],
        action: TileAction::TopRightQuarter,
    },
    TileCommand {
        id: "bottom_left_quarter",
        name: "Tile Bottom Left Quarter",
        description: "Move window to the bottom-left quarter of the screen",
        keywords: &["bottom", "left", "quarter", "corner"],
        action: TileAction::BottomLeftQuarter,
    },
    TileCommand {
        id: "bottom_right_quarter",
        name: "Tile Bottom Right Quarter",
        description: "Move window to the bottom-right quarter of the screen",
        keywords: &["bottom", "right", "quarter", "corner"],
        action: TileAction::BottomRightQuarter,
    },
    // ── Special ──────────────────────────────────────────────────────
    TileCommand {
        id: "maximize",
        name: "Tile Maximize",
        description: "Maximize window to fill the entire screen",
        keywords: &["maximize", "max", "full", "fullscreen"],
        action: TileAction::Maximize,
    },
    TileCommand {
        id: "center",
        name: "Tile Center",
        description: "Center window at 60% of screen size",
        keywords: &["center", "middle", "centered"],
        action: TileAction::Center,
    },
    TileCommand {
        id: "restore",
        name: "Tile Restore",
        description: "Restore window to its original size and position",
        keywords: &["restore", "original", "undo", "reset"],
        action: TileAction::Restore,
    },
    TileCommand {
        id: "equalize_all",
        name: "Tile: Equalize",
        description: "Equalize all split ratios to 50/50",
        keywords: &["equalize", "equal", "even", "balance"],
        action: TileAction::EqualizeAll,
    },
    TileCommand {
        id: "toggle_zoom",
        name: "Tile: Toggle Zoom",
        description: "Toggle zooming the focused pane to fill the screen",
        keywords: &["zoom", "toggle", "focus"],
        action: TileAction::ToggleZoom,
    },
    // ── Pane movement ────────────────────────────────────────────────
    TileCommand {
        id: "move_pane_left",
        name: "Tile: Move Pane Left",
        description: "Move the focused pane to the left",
        keywords: &["move", "pane", "left"],
        action: TileAction::MovePaneLeft,
    },
    TileCommand {
        id: "move_pane_right",
        name: "Tile: Move Pane Right",
        description: "Move the focused pane to the right",
        keywords: &["move", "pane", "right"],
        action: TileAction::MovePaneRight,
    },
    TileCommand {
        id: "move_pane_up",
        name: "Tile: Move Pane Up",
        description: "Move the focused pane upward",
        keywords: &["move", "pane", "up"],
        action: TileAction::MovePaneUp,
    },
    TileCommand {
        id: "move_pane_down",
        name: "Tile: Move Pane Down",
        description: "Move the focused pane downward",
        keywords: &["move", "pane", "down"],
        action: TileAction::MovePaneDown,
    },
    // ── Swap ─────────────────────────────────────────────────────────
    TileCommand {
        id: "swap_pane_left",
        name: "Tile: Swap Pane Left",
        description: "Swap the focused pane with the one to its left",
        keywords: &["swap", "pane", "left"],
        action: TileAction::SwapPaneLeft,
    },
    TileCommand {
        id: "swap_pane_right",
        name: "Tile: Swap Pane Right",
        description: "Swap the focused pane with the one to its right",
        keywords: &["swap", "pane", "right"],
        action: TileAction::SwapPaneRight,
    },
    TileCommand {
        id: "swap_pane_up",
        name: "Tile: Swap Pane Up",
        description: "Swap the focused pane with the one above it",
        keywords: &["swap", "pane", "up"],
        action: TileAction::SwapPaneUp,
    },
    TileCommand {
        id: "swap_pane_down",
        name: "Tile: Swap Pane Down",
        description: "Swap the focused pane with the one below it",
        keywords: &["swap", "pane", "down"],
        action: TileAction::SwapPaneDown,
    },
    // ── Stacking ─────────────────────────────────────────────────────
    TileCommand {
        id: "stack_next",
        name: "Tile: Stack Next",
        description: "Cycle to the next window in the current stack",
        keywords: &["stack", "next", "tab", "cycle"],
        action: TileAction::StackNext,
    },
    TileCommand {
        id: "stack_prev",
        name: "Tile: Stack Previous",
        description: "Cycle to the previous window in the current stack",
        keywords: &["stack", "previous", "prev", "tab", "cycle"],
        action: TileAction::StackPrev,
    },
    // ── Snap ─────────────────────────────────────────────────────────
    TileCommand {
        id: "snap_to_nearest",
        name: "Tile: Snap to Nearest",
        description: "Snap the window beside the nearest adjacent window",
        keywords: &["snap", "nearest", "adjacent", "beside"],
        action: TileAction::SnapToNearest,
    },
];

/// The Tile Window Manager plugin.
///
/// Wraps tile_core's `TileAction` variants as Rover picker commands and
/// uses tile_ax's Accessibility API to execute them on the frontmost window.
pub struct TilePlugin {
    /// Cached original frames for restore functionality.
    /// Keyed by (pid, window_index) to allow restoring windows.
    original_frames: Vec<(i32, usize, tile_core::Rect)>,
}

impl TilePlugin {
    pub fn new() -> Self {
        Self {
            original_frames: Vec::new(),
        }
    }

    /// Execute a TileAction on the frontmost window.
    fn execute_action(&mut self, action: TileAction) -> Result<(), String> {
        // Get the frontmost window via tile_ax.
        let (window_element, ax_ref, _app_info) = tile_ax::get_frontmost_window()
            .ok_or_else(|| "No frontmost window found".to_string())?;

        // Get the usable screen frame (primary display, index 0).
        let screen = tile_ax::get_usable_screen_frame(0)
            .ok_or_else(|| {
                tile_ax::release_frontmost_window(window_element);
                "Could not get screen frame".to_string()
            })?;

        let result = match action {
            // Actions that compute a target frame from the screen rect.
            TileAction::LeftHalf
            | TileAction::RightHalf
            | TileAction::TopHalf
            | TileAction::BottomHalf
            | TileAction::LeftThird
            | TileAction::CenterThird
            | TileAction::RightThird
            | TileAction::LeftTwoThirds
            | TileAction::CenterTwoThirds
            | TileAction::RightTwoThirds
            | TileAction::TopLeftQuarter
            | TileAction::TopRightQuarter
            | TileAction::BottomLeftQuarter
            | TileAction::BottomRightQuarter
            | TileAction::Maximize
            | TileAction::Center => {
                if let Some(frame) = action.compute_frame(screen) {
                    // Save original frame for potential restore.
                    if let Some(current_frame) =
                        tile_ax::get_window_frame_raw(window_element)
                    {
                        self.save_original_frame(
                            ax_ref.pid,
                            ax_ref.window_index,
                            current_frame,
                        );
                    }
                    tile_ax::set_window_frame_raw(window_element, frame);
                    Ok(())
                } else {
                    Err("Action did not produce a target frame".to_string())
                }
            }

            TileAction::Restore => {
                // Restore the frontmost window to its original frame.
                if let Some(original) =
                    self.get_original_frame(ax_ref.pid, ax_ref.window_index)
                {
                    tile_ax::set_window_frame_raw(window_element, original);
                    self.remove_original_frame(ax_ref.pid, ax_ref.window_index);
                    Ok(())
                } else {
                    Err("No original frame saved for this window".to_string())
                }
            }

            // Actions that require BSP tree management. Since we operate on the
            // frontmost window without a persistent tree, these are no-ops with
            // a descriptive message for now.
            TileAction::EqualizeAll
            | TileAction::ToggleZoom
            | TileAction::MovePaneLeft
            | TileAction::MovePaneRight
            | TileAction::MovePaneUp
            | TileAction::MovePaneDown
            | TileAction::SwapPaneLeft
            | TileAction::SwapPaneRight
            | TileAction::SwapPaneUp
            | TileAction::SwapPaneDown
            | TileAction::StackNext
            | TileAction::StackPrev
            | TileAction::SnapToNearest => {
                log::info!(
                    "Tile action {:?} requires BSP tree state (not yet wired for single-shot commands)",
                    action
                );
                Err(format!(
                    "Action {:?} requires a running tile session with managed windows",
                    action
                ))
            }
        };

        // Release the AX element to avoid memory leaks.
        tile_ax::release_frontmost_window(window_element);

        result
    }

    /// Save the original frame of a window for later restore.
    fn save_original_frame(
        &mut self,
        pid: i32,
        window_index: usize,
        frame: tile_core::Rect,
    ) {
        // Only save if we don't already have one for this window.
        if !self
            .original_frames
            .iter()
            .any(|(p, i, _)| *p == pid && *i == window_index)
        {
            self.original_frames.push((pid, window_index, frame));
        }
    }

    /// Get the saved original frame for a window.
    fn get_original_frame(
        &self,
        pid: i32,
        window_index: usize,
    ) -> Option<tile_core::Rect> {
        self.original_frames
            .iter()
            .find(|(p, i, _)| *p == pid && *i == window_index)
            .map(|(_, _, f)| *f)
    }

    /// Remove the saved original frame after restoring.
    fn remove_original_frame(&mut self, pid: i32, window_index: usize) {
        self.original_frames
            .retain(|(p, i, _)| !(*p == pid && *i == window_index));
    }

    /// Find a TileCommand by its id string.
    fn find_command(id: &str) -> Option<&'static TileCommand> {
        TILE_COMMANDS.iter().find(|c| c.id == id)
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
        "Tile Window Manager"
    }

    fn description(&self) -> &str {
        "Tile, snap, and organize windows using keyboard commands"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        // Check accessibility permissions on load.
        if !tile_ax::check_accessibility_permission() {
            log::warn!(
                "Tile plugin: Accessibility permissions not granted. \
                 Window tiling will not work until permissions are enabled \
                 in System Settings > Privacy & Security > Accessibility."
            );
            // Request the permission prompt.
            tile_ax::request_accessibility_permission();
        }
        log::info!("Tile Window Manager plugin loaded ({} commands)", TILE_COMMANDS.len());
        Ok(())
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        TILE_COMMANDS
            .iter()
            .map(|cmd| CommandDescriptor {
                id: cmd.id.to_string(),
                name: cmd.name.to_string(),
                description: cmd.description.to_string(),
                icon: Some(IconHint::SystemSymbol("rectangle.split.2x1".to_string())),
                keywords: cmd.keywords.iter().map(|s| s.to_string()).collect(),
                has_submenu: false,
            })
            .collect()
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        10
    }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // FuzzyCommand mode — the registry handles fuzzy matching against our
        // commands() descriptors. This method is called only when there is a
        // fuzzy match. We return the full set and let the registry sort them.
        let query_lower = query.to_lowercase();
        TILE_COMMANDS
            .iter()
            .filter(|cmd| {
                let name_lower = cmd.name.to_lowercase();
                let desc_lower = cmd.description.to_lowercase();
                name_lower.contains(&query_lower)
                    || desc_lower.contains(&query_lower)
                    || cmd
                        .keywords
                        .iter()
                        .any(|kw| kw.contains(&query_lower))
            })
            .map(|cmd| MatchResult {
                id: cmd.id.to_string(),
                title: cmd.name.to_string(),
                subtitle: Some(cmd.description.to_string()),
                icon: Some(IconHint::SystemSymbol("rectangle.split.2x1".to_string())),
                score: 0.8,
                plugin_id: "builtin.tile".to_string(),
            })
            .collect()
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(cmd) = Self::find_command(match_id) {
            match self.execute_action(cmd.action) {
                Ok(()) => {
                    log::info!("Tile action '{}' executed successfully", cmd.name);
                    ConfirmResult::Dismiss
                }
                Err(e) => {
                    log::warn!("Tile action '{}' failed: {}", cmd.name, e);
                    // Stay open so the user sees something went wrong.
                    ConfirmResult::Dismiss
                }
            }
        } else {
            log::warn!("Unknown tile command: {}", match_id);
            ConfirmResult::Stay
        }
    }

    fn render_match(&self, match_id: &str, _selected: bool) -> RenderHint {
        if let Some(cmd) = Self::find_command(match_id) {
            RenderHint::WithSubtitle {
                subtitle: cmd.description.to_string(),
            }
        } else {
            RenderHint::Default
        }
    }

    fn actions_for_match(&self, _match_id: &str) -> Vec<ActionDescriptor> {
        // No sub-actions for tile commands.
        vec![]
    }
}
