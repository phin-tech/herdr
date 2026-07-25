//! Header row for the right-hand dock: `‹ Title ›`, with clickable arrows
//! that switch between the plugin panes docked there.
//!
//! Modelled on the tab bar (`super::tabs`), which solves the same
//! "several things, one visible, clickable arrows" problem. The geometry is
//! deliberately duplicated rather than shared: the tab bar scrolls a strip of
//! many items, this shows exactly one title, and keeping them independent
//! means upstream changes to the tab bar never conflict with the dock.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::text::display_width_u16;
use crate::app::AppState;

/// Width of each arrow button, matching the tab bar's scroll buttons.
const ARROW_WIDTH: u16 = 3;
const ARROW_LEFT: &str = " ‹ ";
const ARROW_RIGHT: &str = " › ";

/// Geometry of the dock header. Empty rects mean "not shown": the header is
/// hidden when nothing is docked or the dock is collapsed, and the arrows are
/// hidden when only one pane is docked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DockHeaderView {
    pub row: Rect,
    pub title_rect: Rect,
    pub prev_hit_area: Rect,
    pub next_hit_area: Rect,
}

impl DockHeaderView {
    pub fn is_visible(&self) -> bool {
        self.row.width > 0 && self.row.height > 0
    }
}

/// Split a dock area into its header row and the remaining pane area.
///
/// Returns an empty header when there is nothing to show, in which case the
/// pane keeps the whole area — a single docked plugin looks exactly as it did
/// before the header existed.
pub(crate) fn split_dock_area(app: &AppState, area: Rect) -> (Rect, Rect) {
    if !header_is_shown(app, area) {
        return (Rect::default(), area);
    }
    let header = Rect { height: 1, ..area };
    let body = Rect {
        y: area.y.saturating_add(1),
        height: area.height.saturating_sub(1),
        ..area
    };
    (header, body)
}

fn header_is_shown(app: &AppState, area: Rect) -> bool {
    // Needs a row for itself plus at least one for the pane, or the dock
    // becomes a header with nothing under it.
    !app.docked_panes.is_empty() && !app.dock_right_collapsed && area.width > 0 && area.height > 1
}

pub(crate) fn compute_dock_header_view(app: &AppState, area: Rect) -> DockHeaderView {
    let (row, _) = split_dock_area(app, area);
    if row.width == 0 || row.height == 0 {
        return DockHeaderView::default();
    }

    // One docked pane needs no way to switch, so the title takes the full row.
    if app.docked_panes.len() < 2 || row.width <= ARROW_WIDTH * 2 {
        return DockHeaderView {
            row,
            title_rect: row,
            prev_hit_area: Rect::default(),
            next_hit_area: Rect::default(),
        };
    }

    let prev_hit_area = Rect {
        width: ARROW_WIDTH,
        ..row
    };
    let next_hit_area = Rect {
        x: row.x + row.width - ARROW_WIDTH,
        width: ARROW_WIDTH,
        ..row
    };
    let title_rect = Rect {
        x: row.x + ARROW_WIDTH,
        width: row.width - ARROW_WIDTH * 2,
        ..row
    };
    DockHeaderView {
        row,
        title_rect,
        prev_hit_area,
        next_hit_area,
    }
}

/// Title for the active dock, with a position indicator when several are
/// docked so it is obvious there is more than one.
fn header_label(app: &AppState) -> String {
    let Some(dock) = app.docked_pane() else {
        return String::new();
    };
    if app.docked_panes.len() < 2 {
        return dock.title.clone();
    }
    format!(
        "{} ({}/{})",
        dock.title,
        app.dock_active + 1,
        app.docked_panes.len()
    )
}

fn centered_title(label: &str, width: u16) -> String {
    let label_width = display_width_u16(label);
    if label_width >= width {
        return label.to_string();
    }
    let pad = (width - label_width) / 2;
    format!("{}{}", " ".repeat(pad as usize), label)
}

pub(crate) fn render_dock_header(app: &AppState, frame: &mut Frame, view: &DockHeaderView) {
    if !view.is_visible() {
        return;
    }
    let base = Style::default().bg(app.palette.panel_bg);
    let title_style = if app.dock_focused {
        base.fg(app.palette.accent).add_modifier(Modifier::BOLD)
    } else {
        base.fg(app.palette.text)
    };

    frame.render_widget(
        Paragraph::new(centered_title(&header_label(app), view.title_rect.width))
            .style(title_style),
        view.title_rect,
    );

    if view.prev_hit_area.width > 0 {
        let arrow_style = base.fg(app.palette.subtext0);
        frame.render_widget(
            Paragraph::new(ARROW_LEFT).style(arrow_style),
            view.prev_hit_area,
        );
        frame.render_widget(
            Paragraph::new(ARROW_RIGHT).style(arrow_style),
            view.next_hit_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dock(plugin: &str) -> crate::app::state::DockedPaneState {
        crate::app::state::DockedPaneState {
            pane_id: crate::layout::PaneId::alloc(),
            terminal_id: crate::terminal::TerminalId::alloc(),
            side: crate::app::state::DockSide::Right,
            plugin_id: plugin.to_string(),
            entrypoint: "pane".to_string(),
            title: plugin.to_string(),
        }
    }

    fn app_with(count: usize) -> AppState {
        let mut app = AppState::test_new();
        app.docked_panes = (0..count).map(|i| dock(&format!("p{i}"))).collect();
        app.dock_active = 0;
        app
    }

    #[test]
    fn no_header_without_a_docked_pane() {
        let app = app_with(0);
        let area = Rect::new(0, 0, 40, 20);

        assert_eq!(split_dock_area(&app, area), (Rect::default(), area));
        assert!(!compute_dock_header_view(&app, area).is_visible());
    }

    #[test]
    fn header_takes_one_row_and_leaves_the_rest_to_the_pane() {
        let app = app_with(1);
        let area = Rect::new(10, 0, 40, 20);

        let (header, body) = split_dock_area(&app, area);

        assert_eq!(header.height, 1);
        assert_eq!(header.y, area.y);
        assert_eq!(body.y, area.y + 1);
        assert_eq!(header.height + body.height, area.height);
        assert_eq!(body.width, area.width);
    }

    #[test]
    fn a_single_docked_pane_shows_a_title_but_no_arrows() {
        let app = app_with(1);

        let view = compute_dock_header_view(&app, Rect::new(0, 0, 40, 20));

        assert!(view.is_visible());
        assert_eq!(view.title_rect, view.row);
        assert_eq!(view.prev_hit_area, Rect::default());
        assert_eq!(view.next_hit_area, Rect::default());
    }

    #[test]
    fn arrows_appear_once_a_second_pane_is_docked() {
        let app = app_with(2);
        let area = Rect::new(5, 0, 40, 20);

        let view = compute_dock_header_view(&app, area);

        assert_eq!(view.prev_hit_area.width, ARROW_WIDTH);
        assert_eq!(view.next_hit_area.width, ARROW_WIDTH);
        assert_eq!(view.prev_hit_area.x, area.x);
        assert_eq!(
            view.next_hit_area.x + view.next_hit_area.width,
            area.x + area.width
        );
        // Arrows and title tile the row without overlapping.
        assert_eq!(view.title_rect.x, view.prev_hit_area.x + ARROW_WIDTH);
        assert_eq!(
            view.title_rect.x + view.title_rect.width,
            view.next_hit_area.x
        );
    }

    #[test]
    fn collapsed_dock_has_no_header() {
        let mut app = app_with(2);
        app.dock_right_collapsed = true;

        assert!(!compute_dock_header_view(&app, Rect::new(0, 0, 40, 20)).is_visible());
    }

    /// A dock one row tall would otherwise be all header and no pane.
    #[test]
    fn header_is_suppressed_when_there_is_no_room_for_the_pane() {
        let app = app_with(2);

        assert!(!compute_dock_header_view(&app, Rect::new(0, 0, 40, 1)).is_visible());
    }

    #[test]
    fn a_narrow_dock_drops_the_arrows_rather_than_overlapping_them() {
        let app = app_with(2);

        let view = compute_dock_header_view(&app, Rect::new(0, 0, ARROW_WIDTH * 2, 20));

        assert!(view.is_visible());
        assert_eq!(view.prev_hit_area, Rect::default());
        assert_eq!(view.title_rect, view.row);
    }

    /// End-to-end check that the header actually draws into the buffer, not
    /// just that its geometry computes.
    #[test]
    fn header_renders_title_and_arrows_into_the_buffer() {
        let mut app = app_with(2);
        app.dock_active = 1;
        let area = Rect::new(0, 0, 24, 10);
        let view = compute_dock_header_view(&app, area);

        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 10)).unwrap();
        terminal
            .draw(|frame| render_dock_header(&app, frame, &view))
            .unwrap();

        let row: String = (0..24)
            .map(|x| {
                terminal.backend().buffer()[(x, 0)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap_or(' ')
            })
            .collect();

        assert!(row.contains('\u{2039}'), "expected a left arrow in {row:?}");
        assert!(
            row.contains('\u{203a}'),
            "expected a right arrow in {row:?}"
        );
        assert!(row.contains("p1"), "expected the active title in {row:?}");
        assert!(row.contains("(2/2)"), "expected a position hint in {row:?}");
    }

    #[test]
    fn label_shows_position_only_when_several_are_docked() {
        let single = app_with(1);
        assert_eq!(header_label(&single), "p0");

        let mut several = app_with(3);
        several.dock_active = 1;
        assert_eq!(header_label(&several), "p1 (2/3)");
    }
}
