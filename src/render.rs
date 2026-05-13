use fontdue::{Font, FontSettings, Metrics};

const FONT_BYTES: &[u8] = include_bytes!("../assets/Montserrat-Bold.ttf");

pub struct Renderer {
    font: Font,
}

impl Renderer {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_BYTES, FontSettings::default())
            .expect("Failed to load embedded font data");
        Renderer { font }
    }

    /// Render `word` centered on a transparent/black RGBA canvas.
    /// `color` is the (R, G, B) to use for the text.
    pub fn render_word(&self, word: &str, canvas_w: u32, canvas_h: u32, color: (u8, u8, u8)) -> Vec<u8> {
        let mut canvas = vec![0u8; (canvas_w * canvas_h * 4) as usize];
        if word.is_empty() || canvas_w == 0 || canvas_h == 0 {
            return canvas;
        }

        let size = find_size(&self.font, word, canvas_w, canvas_h);
        let glyphs: Vec<(Metrics, Vec<u8>)> =
            word.chars().map(|c| self.font.rasterize(c, size)).collect();

        let ascent: i32 = glyphs
            .iter()
            .map(|(m, _)| m.ymin + m.height as i32)
            .max()
            .unwrap_or(0);
        let descent: i32 = glyphs
            .iter()
            .map(|(m, _)| (-m.ymin).max(0))
            .max()
            .unwrap_or(0);
        let total_w: f32 = glyphs.iter().map(|(m, _)| m.advance_width).sum();

        let baseline_y = (canvas_h as i32) / 2 + (ascent - descent) / 2;
        let start_x = ((canvas_w as f32 - total_w) / 2.0) as i32;

        let mut pen_x = start_x;
        for (metrics, bitmap) in &glyphs {
            let gx = pen_x + metrics.xmin;
            let gy = baseline_y - metrics.ymin - metrics.height as i32;

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let coverage = bitmap[row * metrics.width + col];
                    if coverage == 0 {
                        continue;
                    }
                    let px = gx + col as i32;
                    let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= canvas_w as i32 || py >= canvas_h as i32 {
                        continue;
                    }
                    let idx = ((py as u32 * canvas_w + px as u32) * 4) as usize;

                    canvas[idx] = color.0;
                    canvas[idx + 1] = color.1;
                    canvas[idx + 2] = color.2;
                    canvas[idx + 3] = coverage;
                }
            }
            pen_x += metrics.advance_width as i32;
        }

        canvas
    }
}

fn find_size(font: &Font, word: &str, canvas_w: u32, canvas_h: u32) -> f32 {
    let max_w = canvas_w as f32 * 0.92;
    let max_h = canvas_h as f32 * 0.85;

    let mut lo = 8.0f32;
    let mut hi = 2000.0f32;

    for _ in 0..25 {
        let mid = (lo + hi) / 2.0;
        let (w, h) = measure(font, word, mid);
        if w <= max_w && h <= max_h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn measure(font: &Font, word: &str, size: f32) -> (f32, f32) {
    let metrics: Vec<Metrics> = word.chars().map(|c| font.metrics(c, size)).collect();
    let total_w: f32 = metrics.iter().map(|m| m.advance_width).sum();
    let max_asc = metrics
        .iter()
        .map(|m| m.ymin + m.height as i32)
        .max()
        .unwrap_or(0) as f32;
    let max_des = metrics.iter().map(|m| (-m.ymin).max(0)).max().unwrap_or(0) as f32;
    let total_h = max_asc + max_des;
    (total_w, total_h)
}
