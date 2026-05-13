use crate::lyrics::{TrackLyrics, WordSpan};

#[derive(Debug, Clone)]
pub struct App {
    lyrics: TrackLyrics,
    active_index: Option<usize>,
    status: Option<String>,
}

impl App {
    pub fn new(lyrics: TrackLyrics) -> Self {
        Self { lyrics, active_index: None, status: None }
    }

    pub fn update(&mut self, position_ms: u64) {
        self.active_index = self.lyrics.active_word_index(position_ms);
    }

    pub fn title(&self) -> &str {
        &self.lyrics.title
    }

    pub fn artist(&self) -> &str {
        &self.lyrics.artist
    }

    pub fn active_word(&self) -> Option<&WordSpan> {
        self.active_index.and_then(|index| self.lyrics.words.get(index))
    }

    pub fn lyrics(&self) -> &TrackLyrics {
        &self.lyrics
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = Some(s.into());
    }

    pub fn set_lyrics(&mut self, lyrics: TrackLyrics) {
        self.lyrics = lyrics;
        self.active_index = None;
        self.status = None;
    }

    pub fn set_track_info(&mut self, title: impl Into<String>, artist: impl Into<String>) {
        self.lyrics.title = title.into();
        self.lyrics.artist = artist.into();
        self.lyrics.words.clear();
        self.active_index = None;
    }

    pub fn clear_lyrics(&mut self) {
        self.lyrics.words.clear();
        self.active_index = None;
    }
}
