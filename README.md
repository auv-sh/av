# av

An extremely handy AV searcher and downloader, written in Rust.

Inspired by the style of [astral-sh/uv](https://github.com/astral-sh/uv).

## Highlights

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

# Download (alias of install: get)
av get FSDSS-351
```

## Features

### Search

```bash
av search <keyword> [--json]
```

- Supports both actor names and codes
- Non-JSON uses a table: `# / Code / Title`, with a total count on top

### Detail

```bash
av detail <code> [--json]
```

Displays when available:

- Code, Title, Actors, Release date, Cover
- Plot, Duration, Director, Studio, Label, Series, Genres, Rating
- Preview images
- Magnet count and a few sample links

### List

```bash
av list <actor> [--json]
```

- Lists all codes for an actor; shows a table with total count

### Install / Get

```bash
av install <code>
av get <code>        # alias of install
```

- Automatically selects magnets (preferring higher seeders)
- Uses `aria2c` if available; otherwise falls back to the system default BT client

## Output

- Every subcommand supports `--json` for structured output
- Non-JSON favors readability:
  - `search` / `list`: table + total count
  - `detail`: grouped fields

## Data sources

- Details and search: JavDB (preferred)
- Magnets and fallback: Sukebei (merge magnet details when possible)

Note: field availability depends on page structure and visibility; it may vary by region, mirror, or anti-bot measures.

## Platform support

Verified on macOS / Linux. For Windows, download the zip artifact from Releases.

## Acknowledgements

- README organization inspired by [astral-sh/uv](https://github.com/astral-sh/uv)

## License / Disclaimer

For learning and research purposes only. Use at your own risk and follow local laws and site terms.
