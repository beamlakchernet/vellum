mod app;
mod display;
mod kitty;
mod lyrics;
mod player;
mod render;

use std::{io, path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, time::{Duration, Instant}};

use anyhow::{Context, Result};
use clap::{ArgGroup, Parser};
use crossterm::{event::{self, Event, KeyCode, KeyEventKind, KeyModifiers}, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};

use crate::{app::App, display::DisplayState, lyrics::{fetch_syncedlyrics_for_query, load_lyrics_from_file, parse_lrc_words, TrackLyrics}, player::PlayerSession};

type FetchMessage = (u64, std::result::Result<TrackLyrics, String>);

#[derive(Parser, Debug)]
#[command(name = "vellum", version, about = "A tiny word-synced lyrics TUI")]
#[command(group(
    ArgGroup::new("source")
        .required(true)
        .args(["file", "from_player"])
))]
struct Cli {
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    #[arg(long)]
    from_player: bool,

    #[arg(long)]
    strict_word_sync: bool,

    #[arg(long, default_value_t = 33)]
    tick_ms: u64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let should_require_word_sync = cli.strict_word_sync || cli.from_player;

    let (lyrics, player_session, start_time) = if let Some(path) = cli.file.as_ref() {
        let lyrics = load_lyrics_from_file(path, should_require_word_sync)
            .with_context(|| format!("failed to load lyrics from {}", path.display()))?;
        (lyrics, None, Instant::now())
    } else {
        let player_session = PlayerSession::capture()?;
        let placeholder = TrackLyrics::new("", "", Vec::new());
        (placeholder, Some(player_session), Instant::now())
    };

    let mut app = App::new(lyrics);
    let mut display = DisplayState::new();
    let (fetch_tx, fetch_rx) = mpsc::channel::<FetchMessage>();

    run_tui(
        &mut app,
        &mut display,
        player_session,
        start_time,
        Duration::from_millis(cli.tick_ms),
        should_require_word_sync,
        fetch_tx,
        fetch_rx,
    )
}

fn run_tui(
    app: &mut App,
    display: &mut DisplayState,
    player_session: Option<PlayerSession>,
    start_time: Instant,
    tick_rate: Duration,
    require_word_sync: bool,
    fetch_tx: mpsc::Sender<FetchMessage>,
    fetch_rx: mpsc::Receiver<FetchMessage>,
) -> Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;

    let quit_requested = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&quit_requested);
    ctrlc::set_handler(move || {
        signal_flag.store(true, Ordering::SeqCst);
    })?;

    let mut active_request_id: u64 = 0;
    let mut last_header: Option<(String, String)> = None;

    loop {
        if quit_requested.load(Ordering::SeqCst) {
            break;
        }

        while let Ok((request_id, result)) = fetch_rx.try_recv() {
            if request_id != active_request_id {
                continue;
            }

            match result {
                Ok(new_lyrics) => {
                    let artist = new_lyrics.artist.clone();
                    let title = new_lyrics.title.clone();
                    app.set_lyrics(new_lyrics);
                    display.render_header(&artist, &title);
                    last_header = Some((artist, title));
                }
                Err(err) => {
                    app.clear_lyrics();
                    app.set_status(format!("lyrics unavailable: {}", err));
                    last_header = None;
                }
            }
        }

        let frame_start = Instant::now();
        let position_ms = match &player_session {
            Some(session) => session.position_ms().unwrap_or(0),
            None => frame_start.duration_since(start_time).as_millis() as u64,
        };

        if let Some(session) = &player_session {
            let meta = session.metadata();
            if meta.title != app.title() || meta.artist != app.artist() {
                active_request_id = active_request_id.wrapping_add(1);
                app.set_track_info(meta.title.clone(), meta.artist.clone());
                app.set_status("Loading lyrics...");
                last_header = None;

                let query = build_query(&meta.artist, &meta.title);
                let tx = fetch_tx.clone();
                let title = meta.title.clone();
                let artist = meta.artist.clone();
                let request_id = active_request_id;
                std::thread::spawn(move || {
                    let res = (|| -> std::result::Result<crate::lyrics::TrackLyrics, String> {
                        let raw = fetch_syncedlyrics_for_query(&query).map_err(|e| e.to_string())?;
                        let words = parse_lrc_words(&raw, require_word_sync).map_err(|e| e.to_string())?;
                        Ok(crate::lyrics::TrackLyrics::new(title, artist, words))
                    })();
                    let _ = tx.send((request_id, res));
                });
            }
        }

        app.update(position_ms);

        if let Some(status) = app.status() {
            display.show_status(status);
        } else if let Some(word) = app.active_word() {
            if let Some(session) = &player_session {
                let meta = session.metadata();
                let header = (meta.artist.clone(), meta.title.clone());
                if last_header.as_ref() != Some(&header) {
                    display.render_header(&meta.artist, &meta.title);
                    last_header = Some(header);
                }
            }
            display.show_word(&word.text);
        } else if player_session.is_some() {
            if let Some(session) = &player_session {
                let meta = session.metadata();
                let header = (meta.artist.clone(), meta.title.clone());
                if last_header.as_ref() != Some(&header) {
                    display.render_header(&meta.artist, &meta.title);
                    last_header = Some(header);
                }
            }
        }

        let wait_timeout = next_wait_timeout(app, position_ms, tick_rate);

        if event::poll(wait_timeout)? {
            match event::read()? {
                Event::Resize(_, _) => {
                    display.refresh_size();
                    display.invalidate();
                }
                Event::Key(key) if key.kind == KeyEventKind::Press && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) => break,
                Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) => break,
                _ => {}
            }
        }
    }

    Ok(())
}

fn next_wait_timeout(app: &App, position_ms: u64, tick_rate: Duration) -> Duration {
    const MIN_WAIT_MS: u64 = 1;

    let lyric_delay = app
        .lyrics()
        .next_word_delay_ms(position_ms)
        .map(Duration::from_millis);

    match lyric_delay {
        Some(delay) if delay.is_zero() => Duration::from_millis(MIN_WAIT_MS),
        Some(delay) => delay.min(tick_rate).max(Duration::from_millis(MIN_WAIT_MS)),
        None => tick_rate,
    }
}

fn build_query(artist: &str, title: &str) -> String {
    let trimmed_artist = artist.trim();
    let trimmed_title = title.trim();
    match (trimmed_artist.is_empty(), trimmed_title.is_empty()) {
        (false, false) => format!("{trimmed_artist} {trimmed_title}"),
        (false, true) => trimmed_artist.to_owned(),
        (true, false) => trimmed_title.to_owned(),
        (true, true) => "Unknown track".to_owned(),
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, crossterm::cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
