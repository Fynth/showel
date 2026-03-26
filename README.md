<p align="center">
  <img src="app/assets/icon.svg" alt="Showel logo" width="96" height="96" />
</p>

<h1 align="center">Showel</h1>

<p align="center">
  A native desktop database client built with Rust and Dioxus.
  <br />
  Persistent database chat threads, fast query workflows, and AI-assisted SQL through ACP.
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
- chat-first database workflows with persistent threads
- fast for daily SQL work
- clear workspace with explorer, editor, results, history, and saved queries
- direct support for local AI agents through ACP
- modular Rust workspace instead of a monolith

It is built for people who want a responsive database tool that feels closer to an editor than to a dashboard.

## What It Can Do

| Area | What you get |
| --- | --- |
| Chat | Persistent database chat threads, context-aware prompts, SQL generation, agent-guided execution |
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

## Installation

Release artifacts are published here:

- [GitHub Releases](https://github.com/Fynth/showel/releases)

### Ubuntu / Debian via APT repository

If the APT repository has already been configured on the machine, installation is just:

```bash
sudo apt update
sudo apt install showel
```

To add the repository first:

```bash
echo "deb [arch=amd64 trusted=yes] https://fynth.github.io/showel/apt stable main" | sudo tee /etc/apt/sources.list.d/showel.list
sudo apt update
sudo apt install showel
```

Notes:

- this currently targets `amd64`
- the repository is currently unsigned, so the source line uses `trusted=yes`

### Ubuntu / Debian via downloaded `.deb`

Download the latest Debian package from:

- [Latest Releases](https://github.com/Fynth/showel/releases/latest)

Then install it with:

```bash
sudo apt install ./showel_<version>_amd64.deb
```

or:

```bash
sudo dpkg -i showel_<version>_amd64.deb
sudo apt -f install
```

### Arch Linux / EndeavourOS / Manjaro via AUR

Available AUR packages:

- `showel`
- `showel-bin`
- `showel-git`

Install with:

```bash
yay -S showel
```

or:

```bash
yay -S showel-bin
```

or:

```bash
yay -S showel-git
```

### Linux via release tarball

Download the Linux archive from:

- [Latest Releases](https://github.com/Fynth/showel/releases/latest)

Then unpack and run:

```bash
tar -xzf showel-linux-x86_64.tar.gz
./bin/showel
```

The archive contains:

- `bin/showel`
- `lib/showel/assets/app.css`
- desktop entry
- app icon

### Windows

Download from:

- [Latest Releases](https://github.com/Fynth/showel/releases/latest)

Available artifacts:

- `showel-windows-x86_64.exe`
- Windows `.msi` installer

Notes:

- for raw `.exe`, Microsoft Edge WebView2 Runtime is required
- `.msi` is the better option for end-user installation

### Build from source

Requirements:

- Rust stable
- platform desktop dependencies required by Dioxus Desktop/WebView

Run directly:

```bash
cargo run -p app --features desktop
```

Build release binary:

```bash
cargo build -p app --release --features desktop
```

## AUR

This repo includes two AUR packaging paths:

- `packaging/aur/showel-git/` for a VCS package that tracks the repository head
- `packaging/aur/showel/PKGBUILD.in` plus `scripts/render-aur-release-package.sh` for a stable `showel` package generated from release tags
- `packaging/aur/showel-bin/PKGBUILD.in` plus `scripts/render-aur-binary-package.sh` for a binary `showel-bin` package generated from GitHub release assets

Once the packages are published to AUR, install with:

```bash
yay -S showel
yay -S showel-bin
yay -S showel-git
```

Update with:

```bash
yay -Syu
```

### Automatic AUR updates on each release

The workflow `.github/workflows/aur-publish.yml` pushes fresh `PKGBUILD` and `.SRCINFO` metadata to the AUR repositories `showel.git` and `showel-bin.git` every time a GitHub release is published.

One-time setup:

1. Create an AUR account.
2. Generate an SSH key dedicated to AUR publishing.
3. Add the public key to your AUR account.
4. Add the private key to this GitHub repository as the Actions secret `AUR_SSH_PRIVATE_KEY`.
5. Publish a GitHub release like `v0.1.5`, or run the workflow manually with the version input.

After that, each new published release updates the AUR package automatically.

Notes:

- `showel` is the stable source package built from the tagged source tarball
- `showel-bin` installs the prebuilt Linux release artifact and is the fastest option on AUR
- `showel-git` is still useful if you want AUR users to track the latest commit instead of tagged releases
- `.github/workflows/aur-check.yml` verifies the tracked `showel-git` metadata and smoke-tests the generated stable and binary package metadata

## Windows CI

GitHub Actions includes a Windows packaging workflow:

- `.github/workflows/main.yml`

It can build:

- raw Windows `.exe` artifact
- `.msi` installer artifact

Linux and Arch workflows:

- `.github/workflows/linux.yml`
- `.github/workflows/arch-repo.yml`
- `.github/workflows/apt-repo.yml`
- `.github/workflows/aur-check.yml`

Notes:

- the raw `.exe` build is the fastest path for testing
- the `.msi` bundle uses Dioxus bundling and is the better option for end-user distribution

## APT Repository

This repo now includes Debian packaging and a GitHub Pages-backed APT repository workflow:

- `.github/workflows/apt-repo.yml`
- `scripts/build-deb-package.sh`
- `scripts/build-apt-repo.sh`

The workflow builds a `showel` package and publishes an `amd64` APT repository under:

```bash
https://<owner>.github.io/<repo>/apt
```

To install from the published repository:

```bash
echo "deb [arch=amd64 trusted=yes] https://<owner>.github.io/<repo>/apt stable main" | sudo tee /etc/apt/sources.list.d/showel.list
sudo apt update
sudo apt install showel
```

Notes:

- the initial repository is unsigned, so the source line uses `trusted=yes`
- runtime dependencies target Ubuntu 24.04 / Debian-family systems with `webkit2gtk-4.1`
- each `v*` release publishes both the `.deb` asset and refreshed APT metadata

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
