use std::path::PathBuf;

use crate::app::App;
use crate::layout::PaneId;
use crate::pane::PaneLaunchEnv;
use crate::terminal::{TerminalId, TerminalRuntime, TerminalState};

impl App {
    pub(crate) fn docked_runtime(&self) -> Option<&TerminalRuntime> {
        let terminal_id = &self.state.docked_pane.as_ref()?.terminal_id;
        self.terminal_runtimes.get(terminal_id)
    }

    /// Closes the docked plugin pane, if one is open. Mirrors
    /// `close_popup_pane`, but the dock is not modal: it never changes
    /// `state.mode`, only `dock_focused`.
    pub(crate) fn close_docked_pane(&mut self) -> bool {
        let Some(dock) = self.state.docked_pane.take() else {
            return false;
        };
        self.state
            .direct_attach_resize_locks
            .remove(&dock.terminal_id);
        self.state.terminals.remove(&dock.terminal_id);
        self.state.plugin_panes.remove(&dock.pane_id);
        if let Some(runtime) = self.terminal_runtimes.remove(&dock.terminal_id) {
            runtime.shutdown();
        }
        self.state.dock_focused = false;
        self.render_dirty
            .store(true, std::sync::atomic::Ordering::Release);
        self.render_notify.notify_one();
        true
    }

    pub(crate) fn try_route_paste_to_docked_pane(&mut self, text: &str) -> bool {
        if !self.state.dock_focused || self.state.docked_pane.is_none() {
            return false;
        }
        let Some(runtime) = self.docked_runtime() else {
            self.close_docked_pane();
            return true;
        };
        let _ = runtime.try_send_paste(text.to_owned());
        true
    }

    pub(crate) fn spawn_docked_argv_command(
        &mut self,
        argv: &[String],
        cwd: Option<PathBuf>,
        extra_env: Vec<(String, String)>,
    ) -> std::io::Result<()> {
        self.spawn_docked_command(
            cwd,
            extra_env,
            |pane_id, rows, cols, cwd, launch_env, app| {
                TerminalRuntime::spawn_argv_command(
                    pane_id,
                    rows,
                    cols,
                    cwd,
                    argv,
                    launch_env,
                    crate::pane::AgentDetection::Disabled,
                    app.state.pane_scrollback_limit_bytes,
                    app.state.host_terminal_theme,
                    app.event_tx.clone(),
                    app.render_notify.clone(),
                    app.render_dirty.clone(),
                )
                .map(|runtime| (runtime, Some(argv.to_vec())))
            },
        )
    }

    fn spawn_docked_command<F>(
        &mut self,
        cwd: Option<PathBuf>,
        extra_env: Vec<(String, String)>,
        spawn: F,
    ) -> std::io::Result<()>
    where
        F: FnOnce(
            PaneId,
            u16,
            u16,
            PathBuf,
            &PaneLaunchEnv,
            &mut App,
        ) -> std::io::Result<(TerminalRuntime, Option<Vec<String>>)>,
    {
        if self.state.docked_pane.is_some() {
            return Err(std::io::Error::other("dock already occupied"));
        }
        let cwd = cwd.or_else(|| {
            let ws = self
                .state
                .active
                .and_then(|ws_idx| self.state.workspaces.get(ws_idx))?;
            let active_tab = ws.active_tab()?;
            let focused_pane = ws.focused_pane_id()?;
            active_tab.cwd_for_pane(focused_pane, &self.state.terminals, &self.terminal_runtimes)
        });
        let cwd = cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| "/".into()));
        let pane_id = PaneId::alloc();
        let terminal_id = TerminalId::alloc();
        let launch_env = PaneLaunchEnv::from_extra(extra_env).without_pane_identity();

        let width = self.state.dock_right_width.clamp(
            self.state.dock_right_min_width,
            self.state.dock_right_max_width,
        );
        let terminal_area = self.state.view.terminal_area;
        let (rows, cols) = if terminal_area.height >= 4 && width >= 4 {
            (
                terminal_area.height.saturating_sub(2).max(1),
                width.saturating_sub(2).max(1),
            )
        } else {
            let (estimated_rows, estimated_cols) = self.state.estimate_pane_size();
            (estimated_rows, estimated_cols)
        };

        let (runtime, launch_argv) = spawn(pane_id, rows, cols, cwd.clone(), &launch_env, self)?;
        let terminal = match launch_argv {
            Some(argv) => TerminalState::new(terminal_id.clone(), cwd).with_launch_argv(argv),
            None => TerminalState::new(terminal_id.clone(), cwd),
        };
        self.terminal_runtimes.insert(terminal_id.clone(), runtime);
        self.state.terminals.insert(terminal_id.clone(), terminal);
        self.state.docked_pane = Some(crate::app::state::DockedPaneState {
            pane_id,
            terminal_id,
            side: crate::app::state::DockSide::Right,
        });
        self.render_dirty
            .store(true, std::sync::atomic::Ordering::Release);
        self.render_notify.notify_one();
        Ok(())
    }
}

#[cfg(test)]
impl App {
    pub(crate) fn install_test_docked_runtime(
        &mut self,
        runtime: TerminalRuntime,
    ) -> (PaneId, TerminalId) {
        let pane_id = PaneId::alloc();
        let terminal_id = TerminalId::alloc();
        self.terminal_runtimes.insert(terminal_id.clone(), runtime);
        self.state.terminals.insert(
            terminal_id.clone(),
            TerminalState::new(terminal_id.clone(), PathBuf::from("/dock")),
        );
        self.state.docked_pane = Some(crate::app::state::DockedPaneState {
            pane_id,
            terminal_id: terminal_id.clone(),
            side: crate::app::state::DockSide::Right,
        });
        (pane_id, terminal_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;

    fn app_with_dock() -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("dock")];
        app.state.active = Some(0);
        app.state.selected = 0;
        let terminal_id = TerminalId::alloc();
        app.state.terminals.insert(
            terminal_id.clone(),
            TerminalState::new(terminal_id.clone(), PathBuf::from("/dock")),
        );
        app.state.docked_pane = Some(crate::app::state::DockedPaneState {
            pane_id: PaneId::alloc(),
            terminal_id,
            side: crate::app::state::DockSide::Right,
        });
        app
    }

    #[test]
    fn close_docked_pane_clears_dock_focused_and_direct_attach_resize_lock() {
        let mut app = app_with_dock();
        app.state.dock_focused = true;
        let terminal_id = app.state.docked_pane.as_ref().unwrap().terminal_id.clone();
        app.state
            .direct_attach_resize_locks
            .insert(terminal_id.clone());

        assert!(app.close_docked_pane());

        assert!(app.state.docked_pane.is_none());
        assert!(!app.state.dock_focused);
        assert!(!app.state.direct_attach_resize_locks.contains(&terminal_id));
    }

    #[test]
    fn close_docked_pane_does_not_change_mode() {
        let mut app = app_with_dock();
        app.state.mode = Mode::Navigate;

        assert!(app.close_docked_pane());

        assert_eq!(app.state.mode, Mode::Navigate);
    }

    #[test]
    fn close_docked_pane_on_empty_dock_returns_false() {
        let mut app = app_with_dock();
        app.state.docked_pane = None;

        assert!(!app.close_docked_pane());
    }

    fn pane_read_dock_request() -> crate::api::schema::Request {
        crate::api::schema::Request {
            id: "read-dock".into(),
            method: crate::api::schema::Method::PaneRead(crate::api::schema::PaneReadParams {
                pane_id: App::DOCK_RIGHT_PUBLIC_PANE_ID.into(),
                source: crate::api::schema::ReadSource::Visible,
                lines: None,
                format: crate::api::schema::ReadFormat::default(),
                strip_ansi: true,
            }),
        }
    }

    /// `plugin.pane.open` hands back `dock_right` as a pane id, so `pane.read`
    /// must accept that same id. The dock has no workspace, so it cannot go
    /// through `parse_pane_id` and needs its own branch.
    #[tokio::test]
    async fn pane_read_resolves_the_docked_pane_by_its_public_id() {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("dock")];
        app.state.active = Some(0);
        app.state.selected = 0;
        let (runtime, _rx) = TerminalRuntime::test_with_channel(40, 12);
        app.install_test_docked_runtime(runtime);

        let response = app.handle_api_request(pane_read_dock_request());
        let response: crate::api::schema::SuccessResponse =
            serde_json::from_str(&response).expect("dock read should succeed");

        match response.result {
            crate::api::schema::ResponseResult::PaneRead { read } => {
                assert_eq!(read.pane_id, App::DOCK_RIGHT_PUBLIC_PANE_ID);
            }
            other => panic!("expected pane_read result, got {other:?}"),
        }
    }

    /// Focusing the dock must not trap the keyboard inside it: the prefix key
    /// has to keep switching to prefix mode instead of being forwarded to the
    /// docked pane, or there is no keyboard route back out.
    #[tokio::test]
    async fn prefix_key_escapes_a_focused_dock_instead_of_reaching_the_pane() {
        let mut app = app_with_dock();
        let (runtime, mut rx) = TerminalRuntime::test_with_channel(40, 12);
        app.install_test_docked_runtime(runtime);
        app.state.dock_focused = true;
        app.state.mode = Mode::Terminal;

        let prefix = crate::input::TerminalKey::from(crossterm::event::KeyEvent::new(
            app.state.prefix_code,
            app.state.prefix_mods,
        ));
        assert!(app.handle_key(prefix).await.is_none());

        assert_eq!(app.state.mode, Mode::Prefix);
        assert!(
            rx.try_recv().is_err(),
            "prefix key must not be forwarded to the docked pane"
        );
    }

    /// Once the prefix has been pressed, the following key must reach the
    /// normal prefix dispatch rather than being swallowed by the dock.
    #[tokio::test]
    async fn keys_after_the_prefix_are_not_swallowed_by_a_focused_dock() {
        let mut app = app_with_dock();
        let (runtime, mut rx) = TerminalRuntime::test_with_channel(40, 12);
        app.install_test_docked_runtime(runtime);
        app.state.dock_focused = true;
        app.state.mode = Mode::Prefix;

        let key = crate::input::TerminalKey::from(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::empty(),
        ));
        let _ = app.handle_key(key).await;

        assert!(
            rx.try_recv().is_err(),
            "prefix chords must not leak into the docked pane"
        );
    }

    #[test]
    fn pane_read_on_dock_id_without_a_dock_reports_pane_not_found() {
        let mut app = app_with_dock();
        app.state.docked_pane = None;

        let response = app.handle_api_request(pane_read_dock_request());
        let response: crate::api::schema::ErrorResponse =
            serde_json::from_str(&response).expect("expected an error response");

        assert_eq!(response.error.code, "pane_not_found");
    }

    #[test]
    fn dock_survives_background_workspace_removal() {
        let mut app = app_with_dock();
        app.state.workspaces.clear();
        app.state.active = None;

        app.state.assert_invariants_for_test();

        assert!(app.state.docked_pane.is_some());
    }
}
