use crate::kitty;
use crate::render::Renderer;
use crossterm::{
    cursor,
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

pub struct DisplayState {
    pub renderer: Renderer,
    pixel_w: u32,
    pixel_h: u32,
    cell_rows: u32,
    header_rows: u16,
    pub text_color: (u8, u8, u8),
    last_word: Option<String>,
    last_status: Option<String>,
    last_header: Option<(String, String)>,
}

impl DisplayState {
    pub fn new() -> Self {
        let (pw, ph, _cols, rows) = kitty::get_pixel_size();
        let color = get_system_theme_color();
        DisplayState {
            renderer: Renderer::new(),
            pixel_w: pw,
            pixel_h: ph,
            cell_rows: rows,
            header_rows: 2,
            text_color: color,
            last_word: None,
            last_status: None,
            last_header: None,
        }
    }

    pub fn refresh_size(&mut self) {
        let (pw, ph, _cols, rows) = kitty::get_pixel_size();
        self.pixel_w = pw;
        self.pixel_h = ph;
        self.cell_rows = rows;
    }

    pub fn invalidate(&mut self) {
        self.last_word = None;
        self.last_status = None;
        self.last_header = None;
    }

    pub fn refresh_theme(&mut self) {
        self.text_color = get_system_theme_color();
    }

    /// Draw the artist/title header and clear the word area.
    pub fn render_header(&mut self, artist: &str, title: &str) {
        self.refresh_size();
        self.refresh_theme();
        let header = (artist.to_owned(), title.to_owned());
        if self.last_header.as_ref() == Some(&header) {
            return;
        }

        self.last_header = Some(header);
        self.last_word = None;
        self.last_status = None;
        kitty::clear_images();
        let mut stdout = io::stdout();
        execute!(
            stdout,
            Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            SetForegroundColor(Color::DarkGrey),
            SetAttribute(Attribute::Italic),
            Print(format!("  ♪  {} — {}", artist, title)),
            SetAttribute(Attribute::Reset),
            ResetColor,
        )
        .ok();
    }

    /// Display one big word via Kitty graphics protocol.
    pub fn show_word(&mut self, word: &str) {
        self.refresh_size();
        if self.last_word.as_deref() == Some(word) {
            return;
        }

        self.last_word = Some(word.to_owned());
        self.last_status = None;
        let cell_h = if self.cell_rows > 0 {
            self.pixel_h / self.cell_rows
        } else {
            20
        };
        let img_y_px = self.header_rows as u32 * cell_h;
        let img_h = self.pixel_h.saturating_sub(img_y_px);
        if img_h == 0 || self.pixel_w == 0 {
            return;
        }

        let pixels = self.renderer.render_word(word, self.pixel_w, img_h, self.text_color);

        kitty::clear_images();
        let mut stdout = io::stdout();
        execute!(stdout, cursor::MoveTo(0, self.header_rows)).ok();
        stdout.flush().ok();
        kitty::display_rgba(&pixels, self.pixel_w, img_h);
    }

    /// Show a centered status message.
    pub fn show_status(&mut self, msg: &str) {
        if self.last_status.as_deref() == Some(msg) {
            return;
        }

        self.last_status = Some(msg.to_owned());
        self.last_word = None;
        self.last_header = None;
        kitty::clear_images();
        let (_, rows) = terminal::size().unwrap_or((80, 24));
        let mut stdout = io::stdout();
        execute!(
            stdout,
            Clear(ClearType::All),
            cursor::MoveTo(0, rows / 2),
            SetForegroundColor(Color::DarkGrey),
            SetAttribute(Attribute::Italic),
            Print(format!("  {}", msg)),
            SetAttribute(Attribute::Reset),
            ResetColor,
        )
        .ok();
    }
}

impl Drop for DisplayState {
    fn drop(&mut self) {
        kitty::clear_images();
        execute!(io::stdout(), cursor::Show).ok();
    }
}

fn get_system_theme_color() -> (u8, u8, u8) {
    let is_dark = std::env::var("COLORFGBG")
        .map(|s| {
            if let Some(pos) = s.find(';') {
                let bg = &s[pos + 1..];
                bg.parse::<u8>().unwrap_or(0) < 8
            } else {
                true
            }
        })
        .unwrap_or(true);

    if is_dark {
        (255, 255, 255)
    } else {
        (0, 0, 0)
    }
}