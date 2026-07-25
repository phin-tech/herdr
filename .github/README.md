# herdr — phin-tech fork

A fork of [`ogulcancelik/herdr`](https://github.com/ogulcancelik/herdr), the terminal-based agent runtime for coding agents.

> **This file exists to explain the fork.** For what herdr *is*, how to install it, and how to use it, the upstream material is authoritative: [herdr.dev](https://herdr.dev) · [docs](https://herdr.dev/docs/) · [root `README.md`](../README.md).
>
> GitHub renders `.github/README.md` in preference to the root `README.md`, which is why the fork notes live here — it keeps the root README byte-identical to upstream and therefore conflict-free on every rebase.

---

## Why this fork exists

To carry one feature that isn't upstream: **a right-hand docked pane region**, addressable as the plugin pane placement `sidebar-right`.

Upstream herdr offers five plugin pane placements — `overlay`, `popup`, `split`, `tab`, `zoomed`. All five either live inside a tab's BSP tiling tree or float above it. None lets a plugin occupy a *persistent region of screen chrome* alongside the workspace panes.

Upstream does have one docked chrome region — the left sidebar — but it renders workspace cards and agent detail and has never hosted a PTY. This fork adds a second docked region, on the right, that **does** host a terminal runtime:

```
┌────────┬──────────────────┬──────────┐
│sidebar │ tabs             │ sidebar- │
│ (left) ├──────────────────┤ right    │
│ spaces │ [pane] [pane]    │ ┌──────┐ │
│ agents │                  │ │plugin│ │
│        │                  │ │ PTY  │ │
│        │                  │ └──────┘ │
└────────┴──────────────────┴──────────┘
   upstream      upstream      this fork
```

### Behaviour

| Aspect | Decision |
|---|---|
| Contents | Hosts exactly one plugin PTY pane. Not a TUI-chrome surface. |
| Scope | Global app-wide singleton, modeled on `AppState.popup_pane`. |
| Second open while occupied | Rejected with `ui_busy`. |
| Resize / collapse | Full parity with the left sidebar — drag divider, collapse mode, config keys, session persistence. |
| Modality | **Not** modal. Unlike the popup, focus moves freely between tiled panes and the dock. |

Open one at runtime:

```bash
hoarder dock <plugin> <entrypoint>
# or, equivalently
hoarder plugin pane open --plugin <id> --entrypoint <id> --placement sidebar-right
```

Both `sidebar-right` and `sidebar_right` deserialize; the hyphenated form is what gets emitted.

---

## The `[hoarder]` manifest namespace

A plugin cannot simply write `placement = "sidebar-right"` in `[[panes]]`.
Upstream Herdr rejects placements it does not know, and it fails the **whole
manifest** — every action, hook and pane goes with it:

```
unknown variant `sidebar-right`, expected one of `overlay`, `popup`, `split`, `tab`, `zoomed`
```

Upstream *does* silently ignore unknown manifest **keys**, though. So this fork
reads a `[hoarder]` namespace that upstream never sees:

```toml
[[panes]]
id = "board"
title = "Board"
placement = "popup"          # what upstream Herdr uses
command = ["sh", "-c", 'exec "$HERDR_PLUGIN_ROOT/bin/board"']

[[hoarder.panes]]
id = "sidebar"
title = "Board"
placement = "sidebar-right"  # invisible to upstream
command = ["sh", "-c", 'exec "$HERDR_PLUGIN_ROOT/bin/board" sidebar']
```

| | hoarder | upstream Herdr |
|---|---|---|
| `board` | `popup` | `popup` |
| `sidebar` | `sidebar-right` | not present |

A `[[hoarder.panes]]` entry whose `id` matches an existing pane **replaces** it,
so a manifest can either add a fork-only entrypoint or redeclare an existing one
for this fork. One manifest, one plugin, works on both runtimes.

The namespace is deliberately general — future fork-only settings go under
`[hoarder]` rather than accumulating prefixed keys.

### Environment a plugin can rely on

| Variable | Meaning |
|---|---|
| `HOARDER_ENV=1` | Set in every pane and plugin process this fork spawns |
| `HERDR_PLUGIN_PLACEMENT` | Where the pane was opened: `overlay`, `popup`, `split`, `tab`, `zoomed`, `sidebar-right` |

`HERDR_ENV` stays set alongside `HOARDER_ENV`. Plugins and the shell
integrations read that name, so replacing it would break every installed
plugin.

Prefer a separate `[[hoarder.panes]]` entrypoint over sniffing
`HERDR_PLUGIN_PLACEMENT` when the docked rendering is genuinely different code
— it is explicit, and it keeps the two paths independently testable.

---

## Design notes

Two choices are worth knowing before you touch this code.

**The dock lives outside the tile tree.** It is subtracted from `area` in `compute_view_internal` *before* the tab surface is computed — it is not a `Node` in the BSP layout. This is deliberate. The tiling tree is about how one tab's panes share that tab's area; the dock is app-global chrome. Modeling it as a layout node would have forced it to fight zoom, splits, and close-rebalancing for no benefit. (For contrast: [a fork adding *stacked* panes](https://github.com/universalmind303/herdr/commit/eeffd720f8138ee3869011ff27134bcc2f7ee61a) correctly put that feature *inside* the tree, because stacking genuinely is about intra-tab tiling — and paid for it by having to hook `focus_pane`, `close_focused`, and `remove_pane`.)

**The wire token is UI-flavoured; the internal state is not.** Upstream's `CLAUDE.md` asks for neutral server/API names rather than UI-surface names like "sidebar". `placement` is an exception by precedent — its siblings are already `popup`, `zoomed`, `tab` — so the wire keeps `sidebar-right`. Internally the state is named neutrally (`DockedPaneState`, `AppState.docked_pane`, with a `side` field), so a future bottom or left dock is purely additive.

Everything here is **server-side**. Herdr's client is a thin display terminal: the server owns `App`/`AppState`, runs layout and render, and streams encoded frames (`src/server/render_stream.rs`). The client only forwards input and its size. No client changes were needed — including for the new keybindings, which travel to the server as raw TOML and are parsed there.

---

## Repository layout

| Remote | Points at | Push |
|---|---|---|
| `origin` | `phin-tech/herdr` (this fork) | yes |
| `upstream` | `ogulcancelik/herdr` | **disabled** |

| Branch | Tracks | Role |
|---|---|---|
| `master` | `upstream/master` | Clean mirror of upstream. Never commit here. |
| `feat/*` | `origin/*` | Fork work, rebased onto upstream. |

Pushing to `upstream` is disabled at the git level (`remote.upstream.pushurl = DISABLED`) so an absent-minded `git push upstream` fails loudly instead of attempting to write to someone else's repository.

---

## Staying current with upstream

Upstream is **fast** — roughly 280 commits a month, and the files this fork touches most (`src/app/state.rs`, `src/ui.rs`, `src/app/input/mouse.rs`) each see 45–75 commits a quarter. Drift is the main maintenance cost here, and it compounds: ten small rebases are far cheaper than one large one.

```bash
.local/sync-upstream.sh              # rebase current branch onto upstream/master
.local/sync-upstream.sh feat/foo     # or name a branch
```

The script refuses a dirty tree, reports how much upstream churn landed in the fork's exposed files, rebases, regenerates derived artifacts, and runs `just check`.

**Cadence:** weekly, matching upstream's Wed/Fri preview releases. `git rerere` is enabled with `autoupdate`, so each conflict is resolved once and replayed automatically on subsequent rebases.

### Conflict playbook

| File | Resolution |
|---|---|
| `docs/next/api/herdr-api.schema.json` | Generated — never hand-merge. Take either side, then `HERDR_UPDATE_API_SCHEMA=1 just test-one generated_protocol_schema_artifact_is_current`. |
| `src/protocol/wire.rs` (`PROTOCOL_VERSION`) | Take upstream's value, then re-apply the bump policy from `CLAUDE.md`. |
| `src/ui.rs` (`compute_view_internal`) | The likeliest conflict. Keep the region split factored into a helper so upstream's edits and ours stay disjoint. |
| Everything else | Resolve normally; rerere remembers. |

**Keep the contact surface small.** New files never conflict; edited lines inside hot upstream functions always do. Prefer adding to `src/app/dock.rs` over expanding an existing function body.

---

## Build and test

```bash
just build          # release build
just test           # cargo nextest + maintenance script tests
just check          # fmt + clippy + test + windows lint   <- the gate
```

Run `just check` before committing. Don't bypass a failure — fix it, or state plainly why a narrower check suffices.

Testing a debug build from inside a running herdr session requires clearing inherited socket overrides, or the debug binary talks to the installed stable server:

```bash
env -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH cargo run -- <command>
```

`just check` enforces two things easy to forget when adding surface area: `scripts.test_config_reference_check` (every config key and keybind must appear in `docs/next/website/src/data/config-reference.json`) and `scripts.test_docs_translation_parity` (every docs edit must be mirrored into `ja/` and `zh-cn/`).

### Python caveat on macOS

The maintenance scripts need `tomllib`, i.e. **Python 3.11+**. macOS ships `python3` 3.9, so the final step of `just check` fails on a stock machine with `ModuleNotFoundError: No module named 'tomllib'`. That failure is environmental, not a real check failure.

Run that step through `uv` instead:

```bash
uv run --python 3.12 --no-project python -m unittest \
  scripts.test_agent_detection_manifest_check scripts.test_changelog \
  scripts.test_config_reference_check scripts.test_docs_translation_parity \
  scripts.test_hermes_integration_asset scripts.test_package_windows_conpty \
  scripts.test_preview scripts.test_vendor_libghostty_vt \
  scripts.test_vendor_portable_pty
```

The scripts are stdlib-only, so `--no-project` is safe and no dependency resolution happens. The durable fix is to put a 3.11+ interpreter ahead of `/usr/bin/python3` on `PATH`; the `justfile` is deliberately left unmodified so it stays conflict-free against upstream.

### Known-flaky tests

Three integration tests are timing-sensitive and fail on this machine **on clean upstream master**, unrelated to any fork change — they spawn a real herdr binary in a PTY and time out waiting on a socket:

- `pane_info_and_subscriptions_expose_done_agent_status` (`tests/api_ping.rs`)
- `live_handoff_keeps_agent_started_pane_after_agent_exits` (`tests/live_handoff.rs`)
- `live_handoff_keeps_unmanaged_agent_name_bound_to_saved_session` (`tests/live_handoff.rs`)

Before blaming a fork change for one of these, confirm it against clean master first:

```bash
git stash -u && cargo nextest run --locked <test_name>; git stash pop
```

---

## Contributing upstream

This fork does **not** change how upstream accepts contributions. If any of this work goes back to `ogulcancelik/herdr`, follow that repo's `CONTRIBUTING.md`: start a GitHub Discussion, wait for a maintainer to accept and convert it into an issue, and only open a PR once approved. Agents working in this repository must not open issues or PRs on upstream on a human's behalf.

## License

Inherited from upstream. See [`LICENSE`](../LICENSE).
