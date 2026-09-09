//! Tray icon wrapper with the single `TrayWrapper` naming set.
//!
//! `new / rebuild_menu / sync_menu / update_tooltip / update_icon` —
//! construction failures return [`TrayError`]; runtime updates log through
//! `tracing` (the loop continues on a failed tooltip/icon push).

use thiserror::Error;
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::ui::icon::make_icon;
use crate::ui::menu::{MenuHandles, MenuState, build_menu};
use crate::ui::tooltip::format_tooltip;

/// Tray construction failure.
#[derive(Debug, Error)]
pub enum TrayError {
    /// The underlying `tray-icon` build failed (e.g. Explorer not running).
    /// The source keeps the OS error kind so triage can tell Explorer-down
    /// apart from a bad icon (`{:#}` renders the full chain).
    #[error("tray icon build failed")]
    Build(#[from] tray_icon::Error),
}

/// Wrapper around `tray-icon`'s `TrayIcon` holding the menu handles.
pub struct TrayWrapper {
    /// Underlying tray icon.
    pub tray: TrayIcon,
    /// Handles to keep the menu alive.
    pub handles: MenuHandles,
}

impl TrayWrapper {
    /// Build a new tray icon and menu for `state`.
    ///
    /// The tooltip paints a placeholder volume (50); the caller refreshes it
    /// from the first snapshot immediately after construction.
    ///
    /// # Errors
    ///
    /// Returns [`TrayError::Build`] when the tray icon cannot be created.
    pub fn new(state: &MenuState<'_>) -> Result<Self, TrayError> {
        let handles = build_menu(state);
        let icon = make_icon(state.muted);
        let tooltip = format_tooltip(
            state
                .default_id
                .and_then(|id| state.devices.iter().find(|d| d.id == id)),
            50,
            state.muted,
            state.ui_lang,
        );
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tooltip)
            .with_menu_on_left_click(false)
            .build()?;
        Ok(Self { tray, handles })
    }

    /// Update the tooltip text (logs on failure, loop continues).
    pub fn update_tooltip(&self, text: String) {
        if let Err(e) = self.tray.set_tooltip(Some(text)) {
            tracing::warn!("tray set_tooltip failed: {e:?}");
        }
    }

    /// Update the tray icon for mute state (logs on failure, loop continues).
    pub fn update_icon(&self, muted: bool) {
        let icon = make_icon(muted);
        if let Err(e) = self.tray.set_icon(Some(icon)) {
            tracing::warn!("tray set_icon failed: {e:?}");
        }
    }

    /// Rebuild the context menu from `state` (logs on failure).
    pub fn rebuild_menu(&mut self, state: &MenuState<'_>) {
        let new_handles = build_menu(state);
        self.tray.set_menu(Some(Box::new(new_handles.menu.clone())));
        self.handles = new_handles;
    }

    /// Update the context menu from current state.
    ///
    /// Applies checks/enabled states in place when the device list is
    /// unchanged; falls back to a full rebuild only when devices were
    /// added/removed/reordered.
    pub fn sync_menu(&mut self, state: &MenuState<'_>) {
        if !self.handles.sync_state(state) {
            self.rebuild_menu(state);
        }
    }
}
