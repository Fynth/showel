This repository is Showel: a native desktop database client built as a Rust workspace with Dioxus Desktop 0.7.3.

Use this file as the repo-specific operating manual. Prefer it over generic framework advice.

## Core rules

- This project uses Dioxus 0.7 APIs only. `cx`, `Scope`, and `use_state` do not belong here.
- The app is desktop-first. Do not assume web routing, SSR, or fullstack patterns unless the user explicitly asks for them.
- Prefer existing workspace patterns over generic Dioxus examples.
- Keep code examples concise, but anchor decisions in the actual repo structure.
- Before changing behavior, inspect the relevant crate. The workspace is modular, but the `ui` crate still orchestrates a lot directly.

## Product summary

Showel is a desktop DB client with:

- SQLite, PostgreSQL, MySQL, and ClickHouse connections
- schema explorer, table previews, query execution, exports, and row editing
- saved queries and query history
- persistent chat threads and ACP-assisted SQL workflows
- ACP registry agent install/connect flows plus an embedded Ollama ACP bridge
- dark/light theming and persisted UI settings

Per the current README, ClickHouse supports connect/explore/query/export, but not table-row editing.

## Workspace map

The root `Cargo.toml` defines a multi-crate workspace. The important crates are:

- `app`: desktop shell, Dioxus launch, crash reporting, window config, embedded ACP entrypoint
- `ui`: Dioxus UI, global app state, settings modal, workspace screen, connect screen
- `models`: shared domain models and persisted settings types
- `storage`: local persistence for settings, sessions, saved connections, query history, saved queries, chat DB
- `connection`: DB connection orchestration and SSH tunnel lifecycle
- `connection-ssh`: SSH tunnel implementation
- `database`: common driver traits and error types
- `driver-sqlite`, `driver-postgres`, `driver-mysql`, `driver-clickhouse`: backend-specific drivers
- `drivers`: shared driver-facing types
- `explorer`: schema/database tree loading and table metadata
- `query-core`: core query execution and table-edit logic
- `query-format`: SQL formatting
- `query-io`: import/export logic
- `query`: facade re-exporting `query-core`, `query-format`, and `query-io`
- `acp`: ACP runtime, stdio bridge, permissions, terminals, embedded Ollama support
- `acp-registry`: ACP registry fetch/install logic
- `services`: facade crate re-exporting major operations, but currently not used as a real boundary by the rest of the workspace

## Runtime and startup flow

- `app/src/main.rs` is the real entrypoint.
- Normal startup installs a panic hook, configures the Dioxus desktop window, injects `app.css`, and launches `ui::App`.
- Panic handling writes crash logs under the OS temp dir in `showel/crash-<timestamp>.log` and shows a native error dialog.
- On Unix desktop builds the app sets `app_id = "dev.showel.app"`.
- The binary also supports an embedded ACP mode:
  - `showel acp-agent ollama --model ...`
  - this bypasses UI launch and runs the embedded ACP agent instead

## UI architecture

The UI is Dioxus 0.7 and currently uses modern hooks correctly:

- `use_signal`
- `use_resource`
- `use_effect`
- `#[component]`

Important state facts:

- `ui/src/app.rs` loads persisted UI settings and SQL format settings via `use_resource`.
- It restores connection sessions on launch when `restore_session_on_launch` is enabled.
- It persists updated UI settings and SQL format settings back to storage from effects.
- `ui/src/app_state.rs` contains global signals for:
  - `APP_STATE`
  - theme
  - UI settings
  - SQL format settings
  - settings modal visibility
  - history visibility
  - tooltips
  - toast notifications
- `app_state` also owns session add/remove/restore behavior and triggers session persistence.

This means UI work often spans both component files and `app_state.rs`. Do not assume state is localized.

## Persisted UI settings

`models::AppUiSettings` currently persists:

- `theme`
- `ai_features_enabled`
- `restore_session_on_launch`
- `show_saved_queries`
- `show_connections`
- `show_explorer`
- `show_history`
- `show_sql_editor`
- `show_agent_panel`
- `default_page_size`
- `tool_panel_layout`

Important current-tree fact:

- There is no separate `show_query_manager` flag in the current codebase.
- `show_saved_queries` exists and is persisted.

When adding a new persisted UI toggle, update all of these together:

- `models/src/settings.rs` defaults and serde-compat tests
- any settings modal controls
- workspace visibility/filter helpers
- toolbar buttons or toggle entrypoints
- any flows that should auto-open the relevant panel

## Storage model

Local app data lives under:

- `dirs::data_local_dir()/showel`

Current storage files include:

- `saved_connections.json`
- `session_state.json`
- `saved_queries.json`
- `query_history.json`
- `sql_format_settings.json`
- `app_ui_settings.json`
- `showel.db`
- `acp/workspace/...`

Storage behavior that matters:

- Saved connection metadata is serialized without passwords.
- Passwords are stored via `keyring` service `showel.connections`.
- Secret entries use a hashed key derived from `request.identity_key()`.
- Legacy secret lookup by display name is still supported and migrated forward to the identity-key form.
- Session state persists open connection requests plus the active connection key.
- UI session persistence is triggered from `ui/src/app_state.rs` via `tokio::task::spawn_blocking`.

Current secure-storage limitation:

- If metadata writes succeed but keyring operations fail, the code returns a partial-success error like:
  - `saved connection metadata, but secure storage had issues: ...`
- That means callers may see an error even though JSON metadata was already written.
- There is no fallback plaintext secret store in the current tree.

## Connection layer

`connection::connect_to_db` is the main entrypoint for live connections.

Behavior to preserve:

- SQLite connects directly.
- PostgreSQL, MySQL, and ClickHouse can use SSH tunnels.
- SSH tunnels are registered by session identity key and released via `release_ssh_tunnel`.
- PostgreSQL and MySQL reject SSH tunneling when the host field is a DSN string.
- ClickHouse tunneling parses host/scheme/port more carefully because the host field may contain a URL-like form.

If you touch session removal or replacement logic, make sure SSH tunnel cleanup still happens.

## Query layer

`query` is a thin facade re-exporting:

- `query-core`
- `query-format`
- `query-io`

This means feature work may look like it belongs in `query`, but the real implementation often belongs in one of those lower crates.

## ACP layer

The ACP subsystem is substantial.

- `acp/src/runtime.rs` handles ACP client transport, session lifecycle, permission requests, terminal tools, file IO, and event bridging back into the UI.
- `acp-registry` fetches the ACP registry document and merges built-in registry entries.
- Registry agent ordering currently prioritizes:
  - `codex-acp`
  - `opencode`
  - everything else
- ACP terminals are created through the runtime and tracked in a terminal registry.
- ACP workspace files are scoped under the app-local storage root.

Important runtime detail:

- ACP child spawning already retries Unix `ETXTBSY` / `os error 26` a few times in `spawn_acp_child`.

## Services crate

`services/src/lib.rs` re-exports operations from `acp`, `connection`, `explorer`, `query`, and `storage`.

Important current-tree fact:

- The rest of the workspace does not currently use `services` as its main dependency boundary.
- Do not assume a refactor has already happened just because the crate exists.

## Packaging and release workflows

The repo includes desktop packaging and repo publication workflows.

Current workflow split:

- `test.yml`: formatting, clippy, tests
- `linux.yml`: Linux tarball release asset
- `main.yml`: Windows artifacts
- `flatpak.yml`: Flatpak release artifact
- `apt-repo.yml`: Debian package and APT repo artifact build
- `arch-repo.yml`: Arch package and pacman repo artifact build
- `aur-check.yml`, `aur-publish.yml`, `aur-publish-bin.yml`: AUR validation/publication
- `repo-pages.yml`: the only GitHub Pages deploy workflow

Important Pages rule:

- APT and Arch workflows no longer deploy Pages directly.
- `repo-pages.yml` manually assembles one combined Pages site with:
  - `/apt`
  - `/arch`
  - `/flatpak`
- If a task mentions GitHub Pages publication for package repos, this workflow is the source of truth.

Flatpak repo publishing:

- `scripts/build-flatpak-repo.sh` builds a static Flatpak repo for Pages.
- It archives the current git tree, runs `cargo vendor`, builds the Flatpak repo, generates static deltas, and writes `showel.flatpakrepo` plus a simple `index.html`.

## Build and test commands

Common local commands:

```bash
cargo run -p app --features desktop
cargo build -p app --release --features desktop
cargo check --workspace
cargo test --workspace
```

CI expectations from `test.yml`:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The CI workflow installs Linux desktop dependencies before building/testing Dioxus Desktop code.

## Hotspots and risk areas

These files/crates are large and should be edited carefully:

- `ui/src/screens/workspace/mod.rs`
- `ui/src/screens/workspace/components/result_table.rs`
- `ui/src/screens/workspace/components/explorer/create_table_modal.rs`
- `query-core/src/lib.rs`
- `acp/src/introspection.rs`
- `acp/src/runtime.rs`

Architectural reality:

- `ui` is the most orchestration-heavy crate.
- state, persistence triggers, and side effects are not fully separated yet.
- `services` exists, but the workspace still mostly couples `ui` directly to lower-level crates.

## Dioxus-specific guidance for this repo

- Use Dioxus 0.7 syntax and hooks only.
- Prefer owned props (`String`, `Vec<T>`, cloned model structs) over borrowed props.
- Follow existing signal patterns before introducing new context layers or abstractions.
- Keep RSX readable; this codebase already has some very large component files, so avoid making them worse.
- If a change is not naturally local, check whether it also needs a settings model update, storage change, and toolbar/modal wiring.

## Practical expectations for future agents

- Read the target crate before proposing abstractions.
- Verify whether a feature already exists in settings/models before claiming it is missing.
- Do not invent support that is not present in the current tree.
- When the user asks for a repo-level analysis, focus first on `ui`, `storage`, `connection`, `query-core`, and `acp`.
