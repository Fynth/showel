<p align="center">
  <img src="app/assets/icon.svg" alt="Showel logo" width="96" height="96" />
</p>

<h1 align="center">Showel</h1>

<p align="center">
  A native desktop database client built with Rust and Dioxus.
  <br />
  Fast query workflows, editable table results, and AI-assisted SQL through ACP.
</p>

<p align="center">
  SQLite • PostgreSQL • ClickHouse • Rust • Dioxus • ACP • Ollama
</p>

<p align="center">
  If Showel saves you time, give the repo a star.
</p>

---

## Why Showel

Most database clients are either heavy, web-first, or overloaded with enterprise UI noise.

Showel is trying to be the opposite:

- native desktop app, not a browser tab pretending to be one
- fast for daily SQL work
- clear workspace with explorer, editor, results, history, and saved queries
- direct support for local AI agents through ACP
- modular Rust workspace instead of a monolith

It is built for people who want a responsive database tool that feels closer to an editor than to a dashboard.

## What It Can Do

| Area | What you get |
| --- | --- |
| Querying | Multi-tab SQL editor, query history, structure tabs, formatting, pagination |
| Data exploration | Connection explorer, schemas, tables, views, column loading |
| Result workflows | Sort, filter, inspect rows, JSON view, row details |
| Editing | Draft inserts, cell edits, deletes, apply/discard changes for editable table views |
| Import / export | CSV import, CSV/JSON/XLSX export |
| AI workflows | ACP agent panel, OpenCode via ACP registry, embedded Ollama ACP bridge |
| UX | Compact desktop layout, dark/light theme, saved queries/snippets |

## Database Support

| Database | Connect | Explore | Query | Export | Edit table rows |
| --- | --- | --- | --- | --- | --- |
| SQLite | Yes | Yes | Yes | Yes | Yes |
| PostgreSQL | Yes | Yes | Yes | Yes | Yes |
| ClickHouse | Yes | Yes | Yes | Yes | Not yet |

## AI / ACP Support

Showel includes an ACP client layer and an embedded Ollama ACP bridge.

That means you can:

- connect an external ACP-compatible coding/database agent over `stdio`
- install and connect supported ACP registry agents such as OpenCode
- spawn an embedded Ollama-backed ACP agent directly from the UI
- generate SQL against the active connection context
- send general database prompts and insert generated SQL into the editor

This is opt-in. If you do not care about AI features, Showel still works as a regular database client.

## Quick Start

### Requirements

- Rust stable
- for desktop builds: system dependencies required by Dioxus Desktop/WebView on your platform
- on Windows for raw `.exe`: Microsoft Edge WebView2 Runtime

### Run the desktop app

```bash
cargo run -p app --features desktop
```

### Build a release binary

```bash
cargo build -p app --release --features desktop
```

### Build a Linux release package artifact

GitHub Actions can build a Linux desktop tarball from `.github/workflows/linux.yml`.

The archive contains:

- `bin/showel`
- desktop entry
- app icon
- README

### Build a Windows bundle

```bash
dx bundle --release --platform desktop --package app --features bundle --package-types msi
```

## Arch Linux Updates With Pacman

`pacman -Syu` only works if Showel is shipped through a real pacman repository.

This repo now includes:

- `.github/workflows/arch-repo.yml` to build an Arch `pkg.tar.zst` on version tags
- `packaging/arch/PKGBUILD.in` to define the Arch package
- GitHub Pages publication of `showel.db` and the package payload

Release flow:

1. bump the Rust package version if needed
2. create and push a tag like `v0.1.0`
3. GitHub Actions builds `showel-0.1.0-1-x86_64.pkg.tar.zst`
4. Actions publishes the pacman repo to `https://fynth.github.io/showel/arch/x86_64`

Client setup on Arch:

```ini
[showel]
SigLevel = Optional TrustAll
Server = https://fynth.github.io/showel/arch/$arch
```

Then install and update with:

```bash
sudo pacman -Sy showel
sudo pacman -Syu
```

Notes:

- `SigLevel = Optional TrustAll` is used because the workflow currently publishes an unsigned repo
- if you want stricter pacman trust, the next step is adding package and repo signing with a dedicated GPG key in GitHub Actions secrets

## Windows CI

GitHub Actions includes a Windows packaging workflow:

- `.github/workflows/main.yml`

It can build:

- raw Windows `.exe` artifact
- `.msi` installer artifact

Linux and Arch workflows:

- `.github/workflows/linux.yml`
- `.github/workflows/arch-repo.yml`

Notes:

- the raw `.exe` build is the fastest path for testing
- the `.msi` bundle uses Dioxus bundling and is the better option for end-user distribution

## Project Layout

Showel is organized as a Rust workspace with focused crates instead of one large application crate.

| Crate | Responsibility |
| --- | --- |
| `app` | desktop launcher, build pipeline, embedded ACP agent entrypoint |
| `ui` | Dioxus desktop frontend |
| `models` | shared domain models and contracts |
| `connection` / `connection-ssh` | connection orchestration and SSH support |
| `explorer` | schema and object discovery |
| `query-core` | query execution, pagination, editable rows |
| `query-format` | SQL formatting |
| `query-io` | CSV/JSON/XLSX import-export |
| `acp` / `acp-registry` | ACP runtime, registry integration, Ollama bridge |
| `driver-*` | database-specific implementations |
| `storage` | local persistence for settings, sessions, history, saved queries |

## What Makes It Different

- Native UI with Rust and Dioxus instead of Electron
- AI integration built around ACP instead of a one-off prompt box
- Editable result grid for real table workflows, not just read-only browsing
- Workspace split into independent crates, which makes the codebase easier to evolve

## Current Status

Showel is actively evolving.

Today it is already useful for:

- running queries quickly
- exploring local and remote databases
- editing SQLite/PostgreSQL table data
- exporting/importing common formats
- using ACP-powered assistants for SQL generation

Areas that still need expansion:

- deeper ClickHouse edit workflows
- broader packaging polish across platforms
- more agent presets and richer ACP UX

## Contributing

Issues, UX feedback, database-specific bugs, and performance reports are useful.

If you open a bug report, include:

- database type and version
- the query or workflow that failed
- expected behavior
- actual behavior
- platform (`Linux`, `macOS`, `Windows`)

## Vision

The long-term goal is straightforward:

> make a database client that feels fast, local, hackable, and AI-native without turning into a bloated IDE.

If that direction matches what you want from a desktop database tool, star the repo and follow the project.
