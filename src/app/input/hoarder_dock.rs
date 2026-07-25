use crate::app::state::AppState;

impl AppState {
    /// Hit test for the drag handle between the tiled panes and the right
    /// dock. Mirrors `on_sidebar_divider`.
    pub(super) fn on_dock_right_divider(&self, col: u16, row: u16) -> bool {
        if self.docked_pane().is_none() || self.dock_right_collapsed {
            return false;
        }
        let rect = self.view.dock_right_divider_rect;
        rect.width > 0
            && col >= rect.x
            && col < rect.x + rect.width
            && row >= rect.y
            && row < rect.y + rect.height
    }

    /// Sets `dock_right_width` from a divider drag/click column. Mirrors
    /// `set_manual_sidebar_width`: the dock is anchored to the right edge, so
    /// width grows as the divider moves left (away from the screen edge).
    pub(super) fn set_manual_dock_right_width(&mut self, divider_col: u16) {
        let dock = self.view.dock_right_rect;
        if dock.width == 0 {
            return;
        }
        let right_edge = dock.x + dock.width;
        let width = right_edge.saturating_sub(divider_col);
        self.dock_right_width = width.clamp(self.dock_right_min_width, self.dock_right_max_width);
        self.mark_session_dirty();
    }
}
