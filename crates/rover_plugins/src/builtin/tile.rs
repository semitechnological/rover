//! Tile Window Manager plugin — exposes tile_core's window tiling actions
//! as searchable commands in the Rover picker.
//!
//! Uses `tile_core` for action definitions and frame computation, and `tile_ax`
//! for Accessibility API calls to actually move/resize the frontmost window.
//!
//! Gated behind the `tile` feature flag.

use rover_plugin_api::*;
use tile_core::{
    AXWindowRef, Direction, ManagedWindow, Node, Rect, SnapSide, TileAction, TileTree, WindowInfo,
};

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
        id: "snap_to_nearest",
        name: "Tile: Snap to Nearest",
        description: "Snap the window beside the nearest adjacent window",
        keywords: &["snap", "nearest", "adjacent", "beside"],
        action: TileAction::SnapToNearest,
    },
    TileCommand {
        id: "display_next",
        name: "Tile: Move to Next Display",
        description: "Move window to the next monitor while preserving relative size/position",
        keywords: &["display", "monitor", "next", "screen"],
        action: TileAction::MoveToNextDisplay,
    },
    TileCommand {
        id: "display_prev",
        name: "Tile: Move to Previous Display",
        description: "Move window to the previous monitor while preserving relative size/position",
        keywords: &["display", "monitor", "previous", "screen"],
        action: TileAction::MoveToPreviousDisplay,
    },
    TileCommand {
        id: "undo_last_action",
        name: "Tile: Undo Last Action",
        description: "Undo the last tile action for the focused window",
        keywords: &["undo", "revert", "back"],
        action: TileAction::UndoLastAction,
    },
    TileCommand {
        id: "toggle_multiplexer",
        name: "Tile: Toggle Multiplexer Mode",
        description: "Toggle local multiplexer tiling mode",
        keywords: &["multiplexer", "mode", "toggle", "local"],
        action: TileAction::ToggleMultiplexerMode,
    },
    TileCommand {
        id: "set_multiplexer_region",
        name: "Tile: Set Multiplexer Region",
        description: "Use the focused window frame as the local multiplexer region",
        keywords: &["multiplexer", "region", "windowed", "local"],
        action: TileAction::SetMultiplexerRegionFromFrontmost,
    },
];

/// The Tile Window Manager plugin.
///
/// Wraps tile_core's `TileAction` variants as Rover picker commands and
/// uses tile_ax's Accessibility API to execute them on the frontmost window.
pub struct TilePlugin {
    tree: TileTree,
    original_frames: Vec<(i32, usize, tile_core::Rect)>,
    action_history: Vec<ActionSnapshot>,
    tiling_mode: TilingMode,
    multiplexer_region: Option<tile_core::Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TilingMode {
    Snap,
    Multiplexer,
}

#[derive(Debug, Clone, Copy)]
struct ActionSnapshot {
    pid: i32,
    window_index: usize,
    frame: tile_core::Rect,
}

impl TilePlugin {
    pub fn new() -> Self {
        Self {
            tree: TileTree::new(),
            original_frames: Vec::new(),
            action_history: Vec::new(),
            tiling_mode: TilingMode::Snap,
            multiplexer_region: None,
        }
    }

    /// Execute a TileAction on the frontmost window.
    fn execute_action(&mut self, action: TileAction) -> Result<(), String> {
        let (window_element, ax_ref, app_info) = tile_ax::get_frontmost_window()
            .ok_or_else(|| "No frontmost window found".to_string())?;

        let current_frame = match tile_ax::get_window_frame_raw(window_element) {
            Some(frame) => frame,
            None => {
                tile_ax::release_frontmost_window(window_element);
                return Err("Could not get current window frame".to_string());
            }
        };

        let screen = tile_ax::get_usable_screen_frame(0)
            .ok_or_else(|| {
                tile_ax::release_frontmost_window(window_element);
                "Could not get screen frame".to_string()
            })?;

        let result = match action {
            TileAction::Restore => {
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
            TileAction::UndoLastAction => {
                if let Some(ActionSnapshot {
                    pid,
                    window_index,
                    frame,
                }) = self.action_history.pop()
                {
                    if pid == ax_ref.pid && window_index == ax_ref.window_index {
                        tile_ax::set_window_frame_raw(window_element, frame);
                        Ok(())
                    } else {
                        Err("No matching undo history for this window".to_string())
                    }
                } else {
                    Err("No action history available".to_string())
                }
            }
            TileAction::ToggleMultiplexerMode => {
                self.tiling_mode = if self.tiling_mode == TilingMode::Snap {
                    TilingMode::Multiplexer
                } else {
                    TilingMode::Snap
                };
                Ok(())
            }
            TileAction::SetMultiplexerRegionFromFrontmost => {
                self.multiplexer_region = Some(current_frame);
                self.tiling_mode = TilingMode::Multiplexer;
                Ok(())
            }
            TileAction::EqualizeAll => {
                self.focus_current_window(ax_ref.pid);
                if self.tree.root.window_count() == 0 {
                    Err("No managed Tile session is active yet".to_string())
                } else {
                    self.tree.root.equalize_all();
                    self.relayout(self.base_rect(screen));
                    Ok(())
                }
            }
            TileAction::ToggleZoom => {
                self.focus_current_window(ax_ref.pid);
                if let Some(pane_id) = self.tree.focused_pane {
                    self.tree.root.toggle_zoom(pane_id);
                    self.relayout(self.base_rect(screen));
                    Ok(())
                } else {
                    Err("No managed pane is focused".to_string())
                }
            }
            TileAction::MovePaneLeft
            | TileAction::MovePaneRight
            | TileAction::MovePaneUp
            | TileAction::MovePaneDown => {
                self.focus_current_window(ax_ref.pid);
                if self.tree.root.window_count() == 0 {
                    Err("No managed Tile session is active yet".to_string())
                } else {
                    let dir = match action {
                        TileAction::MovePaneLeft => Direction::Left,
                        TileAction::MovePaneRight => Direction::Right,
                        TileAction::MovePaneUp => Direction::Up,
                        TileAction::MovePaneDown => Direction::Down,
                        _ => unreachable!(),
                    };
                    self.tree.navigate_focus(dir, self.base_rect(screen));
                    self.focus_active_window_in_focused_pane();
                    Ok(())
                }
            }
            TileAction::SwapPaneLeft
            | TileAction::SwapPaneRight
            | TileAction::SwapPaneUp
            | TileAction::SwapPaneDown => {
                self.focus_current_window(ax_ref.pid);
                if self.tree.root.window_count() < 2 {
                    Err("At least two managed panes are required to swap".to_string())
                } else {
                    let dir = match action {
                        TileAction::SwapPaneLeft => Direction::Left,
                        TileAction::SwapPaneRight => Direction::Right,
                        TileAction::SwapPaneUp => Direction::Up,
                        TileAction::SwapPaneDown => Direction::Down,
                        _ => unreachable!(),
                    };
                    let current_pane = self.tree.focused_pane;
                    self.tree.navigate_focus(dir, self.base_rect(screen));
                    let target_pane = self.tree.focused_pane;
                    if let (Some(a), Some(b)) = (current_pane, target_pane) {
                        if a != b {
                            self.tree.swap_panes(a, b);
                            self.relayout(self.base_rect(screen));
                            self.focus_active_window_in_focused_pane();
                            Ok(())
                        } else {
                            Err("No adjacent pane found in that direction".to_string())
                        }
                    } else {
                        Err("No managed pane is focused".to_string())
                    }
                }
            }
            TileAction::StackNext | TileAction::StackPrev => {
                self.focus_current_window(ax_ref.pid);
                if let Some(pane_id) = self.tree.focused_pane {
                    let forward = matches!(action, TileAction::StackNext);
                    if let Some(new_active) = self.tree.cycle_tab(pane_id, forward) {
                        if let Some(Node::Pane { tabs, .. }) = self.tree.root.find(pane_id) {
                            if let Some(active_win) = tabs.get(new_active) {
                                tile_ax::focus_window(&active_win.ax_ref);
                                Ok(())
                            } else {
                                Err("No active window found after cycling stack".to_string())
                            }
                        } else {
                            Err("Focused pane is no longer available".to_string())
                        }
                    } else {
                        Err("The focused pane does not contain a window stack".to_string())
                    }
                } else {
                    Err("No managed pane is focused".to_string())
                }
            }
            TileAction::SnapToNearest => {
                let windows = tile_ax::list_visible_windows();
                if let Some(nearest) = find_nearest_window(current_frame, &windows, app_info.pid) {
                    let side = if current_frame.center().0 < nearest.frame.center().0 {
                        SnapSide::Left
                    } else {
                        SnapSide::Right
                    };
                    let snap_frame =
                        TileTree::snap_window_beside(nearest.frame, current_frame, side, screen);
                    self.push_action_snapshot(ax_ref.pid, ax_ref.window_index, current_frame);
                    self.save_original_frame(ax_ref.pid, ax_ref.window_index, current_frame);
                    tile_ax::set_window_frame_raw(window_element, snap_frame);
                    Ok(())
                } else {
                    Err("No nearby visible window found to snap beside".to_string())
                }
            }
            TileAction::MoveToNextDisplay | TileAction::MoveToPreviousDisplay => {
                let displays = tile_ax::all_usable_frames();
                if displays.len() < 2 {
                    return Err("Only one display detected".to_string());
                }
                let (cx, cy) = current_frame.center();
                let mut current_idx = 0usize;
                for (i, d) in displays.iter().enumerate() {
                    if d.contains_point(cx, cy) {
                        current_idx = i;
                        break;
                    }
                }
                let target_idx = if matches!(action, TileAction::MoveToNextDisplay) {
                    (current_idx + 1) % displays.len()
                } else if current_idx == 0 {
                    displays.len() - 1
                } else {
                    current_idx - 1
                };
                let src = displays[current_idx];
                let dst = displays[target_idx];
                let rx = if src.width > 0.0 {
                    (current_frame.x - src.x) / src.width
                } else {
                    0.0
                };
                let ry = if src.height > 0.0 {
                    (current_frame.y - src.y) / src.height
                } else {
                    0.0
                };
                let rw = if src.width > 0.0 {
                    current_frame.width / src.width
                } else {
                    0.5
                };
                let rh = if src.height > 0.0 {
                    current_frame.height / src.height
                } else {
                    0.5
                };
                let mapped = tile_core::Rect::new(
                    dst.x + dst.width * rx,
                    dst.y + dst.height * ry,
                    dst.width * rw,
                    dst.height * rh,
                );
                self.push_action_snapshot(ax_ref.pid, ax_ref.window_index, current_frame);
                self.save_original_frame(ax_ref.pid, ax_ref.window_index, current_frame);
                tile_ax::set_window_frame_raw(window_element, mapped);
                Ok(())
            }
            _ => {
                self.save_original_frame(ax_ref.pid, ax_ref.window_index, current_frame);
                let base_rect = self.base_rect(screen);
                if self.tiling_mode == TilingMode::Multiplexer {
                    self.ensure_window_managed(
                        &ax_ref,
                        &app_info.name,
                        current_frame,
                        window_element,
                        base_rect,
                    );
                }

                if let Some(frame) = action.compute_frame(base_rect) {
                    self.push_action_snapshot(ax_ref.pid, ax_ref.window_index, current_frame);
                    tile_ax::set_window_frame_raw(window_element, frame);
                    Ok(())
                } else {
                    Err("Action did not produce a target frame".to_string())
                }
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

    fn push_action_snapshot(&mut self, pid: i32, window_index: usize, frame: tile_core::Rect) {
        self.action_history.push(ActionSnapshot {
            pid,
            window_index,
            frame,
        });
    }

    fn base_rect(&self, screen: Rect) -> Rect {
        match (self.tiling_mode, self.multiplexer_region) {
            (TilingMode::Multiplexer, Some(region)) => region,
            _ => screen,
        }
    }

    fn focus_current_window(&mut self, pid: i32) {
        self.tree.focused_pane = self.tree.root.find_pane_by_pid(pid);
    }

    fn ensure_window_managed(
        &mut self,
        ax_ref: &AXWindowRef,
        app_name: &str,
        current_frame: Rect,
        raw_element: *const std::ffi::c_void,
        base_rect: Rect,
    ) {
        if self.tree.root.find_pane_by_pid(ax_ref.pid).is_none() {
            let window = ManagedWindow::new(
                AXWindowRef::new(ax_ref.pid, ax_ref.window_index, raw_element as usize),
                ax_ref.pid,
                app_name.to_string(),
                app_name.to_string(),
                current_frame,
            );
            self.tree.add_window(window);
            self.relayout(base_rect);
        }
        self.focus_current_window(ax_ref.pid);
    }

    fn relayout(&self, screen: Rect) {
        let layout = self.tree.compute_layout(screen);
        for (pane_id, rect) in &layout {
            if let Some(Node::Pane { tabs, active, .. }) = self.tree.root.find(*pane_id) {
                if let Some(window) = tabs.get(*active) {
                    tile_ax::set_window_frame(&window.ax_ref, *rect);
                }
            }
        }
    }

    fn focus_active_window_in_focused_pane(&self) {
        if let Some(pane_id) = self.tree.focused_pane {
            if let Some(Node::Pane { tabs, active, .. }) = self.tree.root.find(pane_id) {
                if let Some(window) = tabs.get(*active) {
                    tile_ax::focus_window(&window.ax_ref);
                }
            }
        }
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
                    ConfirmResult::Stay
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

fn find_nearest_window(from: Rect, windows: &[WindowInfo], exclude_pid: i32) -> Option<WindowInfo> {
    let (cx, cy) = from.center();
    windows
        .iter()
        .filter(|w| w.pid != exclude_pid && !w.is_minimized)
        .min_by(|a, b| {
            let (ax, ay) = a.frame.center();
            let (bx, by) = b.frame.center();
            let da = (ax - cx).powi(2) + (ay - cy).powi(2);
            let db = (bx - cx).powi(2) + (by - cy).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}
