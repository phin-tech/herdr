# Fork inventory

Everything `phin-tech/hoarder` changes relative to `ogulcancelik/herdr`, grouped
by *why it exists* and *how much it costs at rebase time*.

Keep this current. When you touch an upstream file, add it here — the point is
that a rebase never has to be archaeology.

**Current size:** see `git diff --stat upstream/master`. Zero test-file
divergence, deliberately — see Tier 2.

---

## Tier 1 — the feature (low rebase risk)

The `sidebar-right` docked pane. Mostly new files and additive blocks; upstream
rarely touches the same lines.

| Area | Files | Notes |
|---|---|---|
| New modules | `src/app/hoarder_dock.rs`, `src/app/input/hoarder_dock.rs` | Entirely ours. The `hoarder_` prefix makes fork files obvious in any diff. Never conflicts. |
| Placement enum | `src/api/schema/plugins.rs` | One variant + serde alias. |
| API surface | `src/app/api/plugins/{mod,panes}.rs` | New arms in existing matches. |
| State | `src/app/state.rs` | Additive struct fields. Conflicts are trivial. |
| Layout | `src/ui.rs` | **The one hot spot** — the 2-way → 3-way split in `compute_view_internal`. Keep the region split factored into a helper. |
| Render | `src/ui/panes.rs` | New `docked_pane_*` fns, mirrors the popup trio. |
| Input | `src/app/input/{mod,mouse,terminal,navigate}.rs` | Early-dispatch lines; keep them one-liners into `hoarder_dock.rs`. |
| Persistence | `src/persist/{snapshot,restore,io}.rs` | Additive `Option` fields; old snapshots still load. |
| Config | `src/config/{model,keybinds}.rs` | New keys + the `dock` custom-command type. |
| CLI | `src/cli/{plugin,spec}.rs`, `src/cli.rs` | `dock` subcommand + placement value list. |
| Docs | `docs/next/**`, `config-reference.json` | Enforced by `just check`. |

### Design decisions worth not re-litigating

- **The dock lives outside the tile tree.** It is subtracted from the frame in
  `compute_view_internal` before the tab surface is computed. Making it a BSP
  `Node` would force it to fight zoom, splits and close-rebalancing.
- **`dock_right` is a synthetic pane id.** `parse_pane_id` returns
  `(workspace_index, pane_id)` and the dock has no workspace. Widening that
  signature would touch **50 call sites**, 28 of them in `src/app/api/panes.rs`.
  Instead the id is special-cased in `plugin.pane.{open,focus,close}` and
  `pane.read`. `pane.list` still omits the dock — deliberate, not an oversight.
- **`HERDR_*` env vars are unchanged.** Plugins read `HERDR_PLUGIN_ROOT`,
  `HERDR_ENV` and friends. Renaming them breaks every installed plugin.

---

## Tier 2 — the `hoarder` rename (low risk, via a debug/release split)

The fork ships as `hoarder` and keeps its own `~/.config/hoarder/` so it can be
installed and run alongside upstream herdr without sharing config, session
state, plugins or logs.

| Change | File | Notes |
|---|---|---|
| `app_dir_name()` → `hoarder` **in release only** | `src/config/io.rs` | Debug stays `herdr-dev`. See below — this is the whole trick. |
| CLI display name and help text | `src/cli/spec.rs`, `src/main.rs` | ~16 usage strings, `override_usage` on several subcommands, plus the in-file `cli::spec::tests` assertions. Self-contained. |

### Why release-only

A first attempt renamed the directory unconditionally and broke **12
integration tests**. The cause is an upstream bug worth knowing: the test
harness writes `config.toml` into `<XDG_CONFIG_HOME>/herdr/`, but a debug build
reads `<XDG_CONFIG_HOME>/herdr-dev/`. **Upstream's integration tests have never
loaded the config they write.** Renaming the debug directory made that config
real, which changed server behaviour and broke tests that had been passing on
the accident — `cross_area_two_clients_shared_view_...` and
`multi_client_broadcasts_...` among them.

Tests only ever run debug builds; users only ever run release ones. So debug
keeps upstream's name and the harness is untouched, while the shipped binary is
fully isolated. **Zero test files diverge from upstream.**

If upstream ever fixes the harness path bug, the `cfg!(debug_assertions)` arm
can collapse back to a single name.

### Deliberately NOT renamed

- **Cargo package name** stays `herdr` — 31 `CARGO_BIN_EXE_herdr` references
  across 10 test files. `hoarder --version` therefore prints `herdr <version>`,
  and the built artifact is still `target/release/herdr`.
- **Log and socket filenames** stay `herdr.log` / `herdr.sock` — they already
  live inside `~/.config/hoarder/`, so the directory does the isolating, and
  several tests assert on those names.
- **`HERDR_*` env vars** — plugins read `HERDR_PLUGIN_ROOT`, `HERDR_ENV` and
  friends. Renaming breaks every installed plugin.

---

## Rebase procedure

Upstream moves fast — roughly 280 commits/month, and `src/app/state.rs`,
`src/ui.rs` and `src/app/input/mouse.rs` each see 45–75 commits a quarter. Drift
compounds; ten small rebases cost far less than one big one.

```sh
.local/sync-upstream.sh          # gitignored; rebases + regenerates artifacts
```

`git rerere` is enabled with `autoupdate`, so each conflict is solved once.

### Conflict playbook

| File | Resolution |
|---|---|
| `docs/next/api/herdr-api.schema.json` | Generated. Take either side, then `HERDR_UPDATE_API_SCHEMA=1 just test-one generated_protocol_schema_artifact_is_current`. |
| `src/protocol/wire.rs` | Take upstream's `PROTOCOL_VERSION`, then re-apply the bump policy from `CLAUDE.md`. |
| `src/ui.rs` (`compute_view_internal`) | Likeliest conflict. Keep the region split in a helper so the diffs stay disjoint. |
| `tests/*.rs` | Should never conflict — the fork does not modify any test file. If one does, something in Tier 2 crept back in. |

---

## Environment notes (macOS)

- **`cp` into `~/.local/bin` produces a binary macOS SIGKILLs.** Identical
  SHA-256, `codesign -v` clean, killed instantly — the copy picks up a
  `com.apple.provenance` xattr. Always:
  ```sh
  cp target/release/herdr ~/.local/bin/hoarder
  xattr -c ~/.local/bin/hoarder
  codesign --force --sign - ~/.local/bin/hoarder
  ```
- **`just check`'s final step needs Python 3.11+** for `tomllib`; macOS ships
  3.9. Run it via `uv run --python 3.12 --no-project python -m unittest ...`.
- **Panes inherit the *server's* environment**, not the client's. A server
  started from a shell where `SHELL=/bin/zsh` gives every pane zsh. Pin
  `terminal.default_shell` in config so it does not depend on who launched it.

## Known-flaky upstream tests

These fail on **clean upstream master** on this machine — timing-sensitive PTY
integration tests. Confirm against master before blaming a fork change:

- `pane_info_and_subscriptions_expose_done_agent_status` (`tests/api_ping.rs`)
- `live_handoff_keeps_agent_started_pane_after_agent_exits`
- `live_handoff_keeps_unmanaged_agent_name_bound_to_saved_session`

```sh
git stash -u && cargo nextest run --locked <test_name>; git stash pop
```
