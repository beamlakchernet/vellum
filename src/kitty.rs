use base64::{engine::general_purpose, Engine as _};
use std::io::{self, Write};

/// Get terminal pixel + cell dimensions via TIOCGWINSZ ioctl.
/// Returns (pixel_w, pixel_h, cell_cols, cell_rows).
pub fn get_pixel_size() -> (u32, u32, u32, u32) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws);
        if ws.ws_xpixel > 0 && ws.ws_ypixel > 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            return (
                ws.ws_xpixel as u32,
                ws.ws_ypixel as u32,
                ws.ws_col as u32,
                ws.ws_row as u32,
            );
        }
    }

    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    (cols as u32 * 10, rows as u32 * 20, cols as u32, rows as u32)
}

/// Delete all Kitty graphics placements.
pub fn clear_images() {
    let mut stdout = io::stdout();
    write!(stdout, "\x1b_Ga=d,d=a\x1b\\").ok();
    stdout.flush().ok();
}

/// Transmit and display an RGBA image at the current cursor position.
pub fn display_rgba(pixels: &[u8], width: u32, height: u32) {
    let b64 = general_purpose::STANDARD.encode(pixels);
    let mut stdout = io::stdout();
    let chunk_size = 4096;
    let bytes = b64.as_bytes();
    let total_chunks = (bytes.len() + chunk_size - 1) / chunk_size;

    for (i, chunk) in bytes.chunks(chunk_size).enumerate() {
        let more = if i < total_chunks - 1 { 1 } else { 0 };
        let chunk_str = std::str::from_utf8(chunk).unwrap_or("");
        if i == 0 {
            write!(
                stdout,
                "\x1b_Ga=T,f=32,s={},v={},q=2,m={};{}\x1b\\",
                width, height, more, chunk_str
            )
            .ok();
        } else {
            write!(stdout, "\x1b_Gm={};{}\x1b\\", more, chunk_str).ok();
        }
    }
    stdout.flush().ok();
}
