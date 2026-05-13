# Vellum

Vellum is a tiny Linux terminal app that displays exactly one lyric word at a time and advances that word from real timestamps instead of estimated animation. The design is intentionally strict: the core display is driven by enhanced LRC, where every word can carry its own timestamp, and the app only falls back to line-synced lyrics when strict word sync is not required.

## How it works

The app is split into a small pipeline:

1. Lyrics are loaded either from a local `.lrc` file or from the current MPRIS player.
2. `syncedlyrics` is used only as the external lyrics fetcher in `--from-player` mode.
3. The returned LRC text is parsed into a flat list of `WordSpan` entries with millisecond timestamps.
4. On each frame, the app reads the current playback position from MPRIS, or in file mode uses elapsed wall-clock time.
5. The latest word whose timestamp is less than or equal to the current position is selected.
6. Ratatui centers that single active word on screen.

The render loop is timestamp-aware: after each draw it wakes on the next lyric boundary when one is imminent, instead of waiting on a fixed cadence alone. That keeps the display aligned to the embedded word timestamps as closely as the input and playback position allow.

This is the important part of the architecture: Vellum does not divide a lyric line by word count and does not invent timing. When enhanced LRC is available, the timestamps already exist in the source and the app uses them directly.

## Project structure

- `src/main.rs` wires the CLI, terminal lifecycle, and runtime loop.
- `src/app.rs` stores the loaded lyrics and active word selection.
- `src/lyrics.rs` parses LRC and fetches lyrics through `syncedlyrics`.
- `src/player.rs` talks to MPRIS and reads track metadata plus playback position.
- `src/render.rs` draws the centered active word.

## Supported modes

### `vellum-lyrics --file song.lrc`

Debug mode for local files. This is the easiest way to verify parsing and rendering before connecting to MPRIS. The app reads the file, parses the LRC, then advances using elapsed time from the moment the UI starts.

Use this mode to test enhanced LRC files first:

```text
[00:12.00]I <00:12.20>see <00:12.45>the <00:12.70>light
```

In that format, each word has its own timestamp and Vellum can show one exact active word at a time.

### `vellum-lyrics --from-player`

Playback mode for Linux desktops with an MPRIS-compatible media player.

Vellum:

- finds the active player on D-Bus through MPRIS
- reads the current track title and artist
- calls `syncedlyrics` to fetch synchronized lyrics
- parses the returned enhanced LRC
- redraws using the player’s playback position on each tick

This mode is the live integration path and depends on both MPRIS and the `syncedlyrics` CLI being available on your system.

The fetched lyrics are used only in memory while Vellum is running. The app does not save or delete lyrics on disk.

## Parsing rules

Vellum’s parser is deliberately small and strict:

- Enhanced LRC: parsed into individual word spans with per-word timestamps.
- Regular line-synced LRC: accepted as a fallback when strict word sync is not enabled, but the display is only line-accurate, not word-accurate.
- Plain text: rejected rather than silently fabricated.

If you want the app to fail whenever input is not truly word-synced, use `--strict-word-sync`.

## Build requirements

- Rust 2021 or newer
- Linux with an MPRIS-compatible player for `--from-player`
- `syncedlyrics` installed for player mode

The Rust dependencies used here are:

- `ratatui` for terminal rendering
- `crossterm` for terminal setup and keyboard handling
- `clap` for CLI parsing
- `mpris` for D-Bus playback queries
- `ctrlc` for clean termination on Ctrl+C

## Installation

### Automated Installation (Recommended)

Download and run the install script to set up both the Vellum binary and `syncedlyrics`:

```bash
curl -fsSL https://raw.githubusercontent.com/beamlakchernet/vellum/main/install.sh | bash
```

The script will:
1. Check for Rust and Cargo
2. Install the `vellum` binary
3. Install `syncedlyrics` via pip if not already present

Then use:

```bash
vellum-lyrics --from-player
```

### Manual Installation

If you prefer to install manually:

1. **Install the Rust binary:**

```bash
cargo install vellum-lyrics
```

2. **Install `syncedlyrics`:**

```bash
pip install syncedlyrics
```

After installation, `vellum-lyrics` will be available on your PATH and can be run from any directory.

## Usage

Run against a known enhanced LRC file:

```text
cargo run -- --file song.lrc
```

Require true enhanced word sync:

```text
cargo run -- --file song.lrc --strict-word-sync
```

Follow the currently playing track from MPRIS:

```text
cargo run -- --from-player
```

## Development

Run the parser tests:

```text
cargo test
```

Build the binary:

```text
cargo build
```

## Behavior notes

- The screen is intentionally minimal: one centered active word.
- Accuracy comes from timestamps, not animation.
- The app follows embedded word timestamps directly and adjusts its wakeup timing to the next lyric boundary when possible.
- The app restores the terminal state on exit and on Ctrl+C.
- If `--from-player` cannot fetch enhanced lyrics, Vellum returns a clear error instead of pretending the lyrics are word-synced.

## Lyrics Caching

**Memory-only by design:**
- Vellum **does not save lyrics to disk**. All fetched lyrics exist only in memory while the app is running.
- When you restart Vellum on the same track, `syncedlyrics` fetches the lyrics again.

**External caching (syncedlyrics):**
- The `syncedlyrics` tool may cache downloads in its own cache directory (typically `~/.cache/syncedlyrics/` on Linux).
- This is transparent to Vellum — the cache improves performance on repeated queries but doesn't affect Vellum's operation.
- Users have no special setup needed; everything is handled automatically.
