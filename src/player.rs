use std::time::Duration;

use anyhow::{Context, Result};
use mpris::{Player, PlayerFinder};

#[derive(Debug)]
pub struct PlayerSession {
    player: Player,
}

#[derive(Debug, Clone)]
pub struct PlayerTrackInfo {
    pub title: String,
    pub artist: String,
}

impl PlayerSession {
    pub fn capture() -> Result<Self> {
        let finder = PlayerFinder::new().context("failed to connect to MPRIS/D-Bus")?;
        let player = finder.find_active().context("no active MPRIS player was found")?;
        Ok(Self { player })
    }

    pub fn metadata(&self) -> PlayerTrackInfo {
        let metadata = self.player.get_metadata().ok();
        let title = metadata
            .as_ref()
            .and_then(|metadata| metadata.title())
            .unwrap_or("Unknown title")
            .to_owned();
        let artist = metadata
            .as_ref()
            .and_then(|metadata| metadata.artists())
            .and_then(|artists| artists.first().copied())
            .unwrap_or("Unknown artist")
            .to_owned();

        PlayerTrackInfo { title, artist }
    }

    pub fn position_ms(&self) -> Result<u64> {
        let position = self.player.get_position().context("failed to read MPRIS playback position")?;
        Ok(duration_to_ms(position))
    }
}

fn duration_to_ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
}
