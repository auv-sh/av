# av

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](#platform-support)
[![GitHub release](https://img.shields.io/github/release/auv-sh/av.svg)](https://github.com/auv-sh/av/releases)

An extremely handy AV searcher and downloader, written in Rust. A gift to my bro Mr.Fu

Inspired by the style of [astral-sh/uv](https://github.com/astral-sh/uv).

## Highlights

[![Build Status](https://img.shields.io/github/actions/workflow/status/auv-sh/av/release.yml?branch=main)](https://github.com/auv-sh/av/actions)
[![GitHub Stars](https://img.shields.io/github/stars/auv-sh/av?style=social)](https://github.com/auv-sh/av)

- 🚀 One tool for search, details, listing, and downloading
- ⚡️ Async scraping for fast responses (JavDB first, Sukebei as fallback and magnet merge)
- 🧾 `--json` output for scripting and automation
- 🧲 Picks the magnet with the highest seeders for download
- 🖥️ Cross-platform (macOS / Linux / Windows) with optional aria2c integration

## Installation

One-line install (from Releases):

```bash
curl -fsSL https://raw.github.com/auv-sh/av/master/install.sh | sh
```

Build from source (Rust stable toolchain required):

```bash
git clone <your-repo-url> av && cd av
cargo build --release
./target/release/av --help
```

Optional: add to PATH

```bash
sudo cp target/release/av /usr/local/bin/
```

Optional downloader dependency:

- Install `aria2c` for a more controllable download experience
  - macOS: `brew install aria2`
  - Linux/Windows: use your package manager
- Without `aria2c`, the system default magnet handler is used (macOS: `open` / Linux: `xdg-open` / Windows: `start`)

## Quickstart

```bash
# Search (actor or code), table output by default
av search 三上悠亞
av search FSDSS-351 --json

# Show details (rich fields when available)
av detail FSDSS-351

# List all codes for an actor (table + total)
av list 橋本ありな
av ls 橋本ありな    # alias of list

# Show the latest releases (default 20; use --limit)
av top --limit 30

# Only show uncensored entries (works with search/list/top)
av search 三上悠亞 --uncen
av list 橋本ありな -u
av top --limit 30 --uncen

# Actors ranking (with pagination)
av actors --page 1 -n 50
av actors --uncen --page 3 -n 30

# Download (alias of install: get)
av get FSDSS-351

# Open in browser to watch
av view FSDSS-351
av see FSDSS-351    # alias of view

# Update to the latest version
av update
```

## Features

### Search

```bash
av search <keyword> [--json]
```

- Supports both actor names and codes
- Non-JSON uses a table: `# / Code / Title`, with a total count on top
- Supports uncensored-only filter: `--uncen` (alias `-u`)

### Detail

```bash
av detail <code> [--json]
```

Displays when available:

- Code, Title, Actors, Release date, Cover
- Plot, Duration, Director, Studio, Label, Series, Genres, Rating
- Preview images
- Magnet count and a few sample links

### List / Ls

```bash
av list <actor> [--json]
av ls <actor> [--json]    # alias of list
```

- Lists all codes for an actor; shows a table with total count
- Supports uncensored-only filter: `--uncen` (alias `-u`)

### Top (latest releases)

```bash
av top [--limit N] [--json]
```

- Lists latest titles from JavDB (most recent first); defaults to 20 items
- Respects `AV_JAVDB_BASE` / `AV_HTTP_PROXY` / `AV_JAVDB_COOKIE`
- Supports uncensored-only filter: `--uncen` (alias `-u`)

### Actors (ranking)

```bash
av actors [--page N] [--per-page N|-n N] [--uncen] [--json]
```

- Lists actors ranked by trending/hotness; supports pagination
- `--uncen/-u`: lists uncensored actors from `actors/uncensored?page=N`
- Output: table with index, actor name, hot value; top shows total and current page
- Respects `AV_JAVDB_BASE` / `AV_HTTP_PROXY` / `AV_JAVDB_COOKIE`

### Install / Get

```bash
av install <code>
av get <code>        # alias of install
```

- Automatically selects magnets (preferring higher seeders)
- Uses `aria2c` if available; otherwise falls back to the system default BT client

### View / See

```bash
av view <code>
av see <code>    # alias of view
```

- Opens the video in your default browser
- Requires `AV_JAVDB_COOKIE` environment variable for full access
- Finds the "Watch Full Movie" link from JavDB

### Update

```bash
av update
```

- Updates the tool to the latest version
- Downloads the installer script from GitHub
- Automatically installs the latest release
- Preserves your current installation path

## Output

- Every subcommand supports `--json` for structured output
- Non-JSON favors readability:
  - `search` / `list`: table + total count
  - `detail`: grouped fields

## Data sources

[![JavDB](https://img.shields.io/badge/JavDB-primary-red.svg)](https://javdb.com)
[![Sukebei](https://img.shields.io/badge/Sukebei-magnets-orange.svg)](https://sukebei.nyaa.si)

- Details and search: JavDB (preferred)
- Magnets and fallback: Sukebei (merge magnet details when possible)

Note: field availability depends on page structure and visibility; it may vary by region, mirror, or anti-bot measures.

## Platform support

[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](#platform-support)
[![Alpine Linux](https://img.shields.io/badge/Alpine%20Linux-Ready-blue.svg)](#platform-support)

Verified on:

- macOS (Apple Silicon)
- Linux (glibc-based distributions)
- Linux (musl-based distributions like Alpine Linux)
- Windows

The installer automatically detects your system and downloads the appropriate binary. For Alpine Linux and other musl-based distributions, a statically linked binary is provided.

## Acknowledgements

- README organization inspired by [astral-sh/uv](https://github.com/astral-sh/uv)

## License / Disclaimer

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

For learning and research purposes only. Use at your own risk and follow local laws and site terms.
