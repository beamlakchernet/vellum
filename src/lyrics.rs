use std::{fs, path::Path};

use anyhow::{anyhow, bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordSpan {
    pub start_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLyrics {
    pub title: String,
    pub artist: String,
    pub words: Vec<WordSpan>,
}

impl TrackLyrics {
    pub fn new(title: impl Into<String>, artist: impl Into<String>, words: Vec<WordSpan>) -> Self {
        Self {
            title: title.into(),
            artist: artist.into(),
            words,
        }
    }

    pub fn active_word_index(&self, position_ms: u64) -> Option<usize> {
        if self.words.is_empty() {
            return None;
        }

        let index = self.words.partition_point(|word| word.start_ms <= position_ms);
        index.checked_sub(1)
    }

    pub fn next_word_delay_ms(&self, position_ms: u64) -> Option<u64> {
        let index = self.words.partition_point(|word| word.start_ms <= position_ms);
        let next_word = self.words.get(index)?;

        Some(next_word.start_ms.saturating_sub(position_ms))
    }
}

pub fn load_lyrics_from_file(path: impl AsRef<Path>, strict_word_sync: bool) -> Result<TrackLyrics> {
    let raw = fs::read_to_string(path.as_ref())
        .with_context(|| format!("unable to read {}", path.as_ref().display()))?;
    let words = parse_lrc_words(&raw, strict_word_sync)?;
    Ok(TrackLyrics::new(
        path.as_ref().file_stem().and_then(|stem| stem.to_str()).unwrap_or("Vellum"),
        "",
        words,
    ))
}

fn fetch_syncedlyrics(query: &str) -> Result<String> {
    let candidates: [&[&str]; 4] = [
        &["--enhanced", query],
        &[query, "--enhanced"],
        &["-m", "syncedlyrics", "--enhanced", query],
        &["-m", "syncedlyrics", query, "--enhanced"],
    ];

    for args in candidates {
        let program = if args.first() == Some(&"-m") { "python3" } else { "syncedlyrics" };
        let output = std::process::Command::new(program).args(args).output();

        if let Ok(output) = output {
            if output.status.success() {
                return String::from_utf8(output.stdout).map_err(|error| anyhow!(error));
            }
        }
    }

    bail!("syncedlyrics could not fetch enhanced lyrics")
}

/// Public wrapper used by background workers to fetch raw syncedlyrics output.
pub fn fetch_syncedlyrics_for_query(query: &str) -> Result<String> {
    fetch_syncedlyrics(query)
}

pub fn parse_lrc_words(raw: &str, strict_word_sync: bool) -> Result<Vec<WordSpan>> {
    let parsed = parse_lrc_internal(raw)?;

    if strict_word_sync && !parsed.saw_word_timestamps {
        bail!("strict word sync requested, but the lyrics are not enhanced LRC");
    }

    if parsed.words.is_empty() {
        bail!("no lyric timestamps were found in the supplied content");
    }

    Ok(parsed.words)
}

struct ParsedLyrics {
    words: Vec<WordSpan>,
    saw_word_timestamps: bool,
}

fn parse_lrc_internal(raw: &str) -> Result<ParsedLyrics> {
    let mut words = Vec::new();
    let mut saw_word_timestamps = false;
    let mut offset_ms: i64 = 0;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some((key, value)) = parse_metadata_line(line) {
            match key.as_str() {
                "offset" => {
                    offset_ms = value.parse::<i64>().context("invalid LRC offset")?;
                }
                _ => {}
            }
            continue;
        }

        let (line_times, content) = split_leading_timestamps(line)?;
        if line_times.is_empty() {
            continue;
        }

        let content = content.trim();
        if content.is_empty() {
            continue;
        }

        if content.contains('<') || content.contains('>') {
            saw_word_timestamps = true;
            let first_time = line_times[0];
            let mut current_time = Some(first_time);

            for token in content.split_whitespace() {
                let (explicit_times, text) = split_embedded_timestamps(token)?;
                let text = text.trim();

                // Update current_time from any embedded timestamps, even if no text
                if let Some(time) = explicit_times.last().copied() {
                    current_time = Some(time);
                }

                // Skip tokens that have no actual text (pure timestamps)
                if text.is_empty() {
                    continue;
                }

                let time = current_time.unwrap_or(first_time);
                words.push(WordSpan {
                    start_ms: adjust_offset(time, offset_ms)?,
                    text: text.to_owned(),
                });
            }

            continue;
        }

        for time in line_times {
            words.push(WordSpan {
                start_ms: adjust_offset(time, offset_ms)?,
                text: content.to_owned(),
            });
        }
    }

    words.sort_by_key(|span| span.start_ms);
    Ok(ParsedLyrics { words, saw_word_timestamps })
}

fn parse_metadata_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('[')?;
    let rest = rest.strip_suffix(']')?;
    let (key, value) = rest.split_once(':')?;
    if key.chars().all(|character| character.is_ascii_alphabetic()) {
        Some((key.to_lowercase(), value.to_owned()))
    } else {
        None
    }
}

fn split_leading_timestamps(line: &str) -> Result<(Vec<u64>, &str)> {
    let mut timestamps = Vec::new();
    let mut rest = line;

    while let Some(stripped) = rest.strip_prefix('[') {
        let Some(end) = stripped.find(']') else {
            break;
        };

        let tag = &stripped[..end];
        if let Some(timestamp) = parse_lrc_timestamp(tag) {
            timestamps.push(timestamp);
            rest = &stripped[end + 1..];
            continue;
        }

        break;
    }

    Ok((timestamps, rest))
}

fn split_embedded_timestamps(token: &str) -> Result<(Vec<u64>, &str)> {
    let mut timestamps = Vec::new();
    let mut rest = token;

    while let Some(stripped) = rest.trim_start().strip_prefix('<').or_else(|| rest.trim_start().strip_prefix('[')) {
        let closing = if rest.trim_start().starts_with('<') { '>' } else { ']' };
        let Some(end) = stripped.find(closing) else {
            break;
        };

        let tag = &stripped[..end];
        if let Some(timestamp) = parse_lrc_timestamp(tag) {
            timestamps.push(timestamp);
            rest = &stripped[end + 1..];
            continue;
        }

        break;
    }

    Ok((timestamps, rest.trim_start()))
}

fn parse_lrc_timestamp(tag: &str) -> Option<u64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let (seconds, fraction) = parse_seconds(seconds)?;
    Some(minutes.saturating_mul(60_000) + seconds.saturating_mul(1000) + fraction)
}

fn parse_seconds(value: &str) -> Option<(u64, u64)> {
    let (seconds, fraction) = if let Some((seconds, fraction)) = value.split_once('.') {
        (seconds, fraction)
    } else if let Some((seconds, fraction)) = value.split_once(':') {
        (seconds, fraction)
    } else {
        return None;
    };

    let seconds = seconds.parse::<u64>().ok()?;
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction[..3.min(fraction.len())].parse::<u64>().ok()?,
    };

    Some((seconds, fraction))
}

fn adjust_offset(timestamp_ms: u64, offset_ms: i64) -> Result<u64> {
    let shifted = timestamp_ms as i64 + offset_ms;
    if shifted < 0 {
        bail!("LRC offset moved a timestamp before zero")
    }
    Ok(shifted as u64)
}

#[cfg(test)]
mod tests {
    use super::{parse_lrc_words, TrackLyrics, WordSpan};

    #[test]
    fn parses_enhanced_lrc_into_word_spans() {
        let raw = r#"
[ti:Glow]
[ar:Vellum]
[00:12.00]I <00:12.20>see <00:12.45>the <00:12.70>light
"#;

        let words = parse_lrc_words(raw, true).expect("enhanced LRC should parse");
        assert_eq!(
            words,
            vec![
                WordSpan { start_ms: 12_000, text: "I".to_owned() },
                WordSpan { start_ms: 12_200, text: "see".to_owned() },
                WordSpan { start_ms: 12_450, text: "the".to_owned() },
                WordSpan { start_ms: 12_700, text: "light".to_owned() },
            ]
        );
    }

    #[test]
    fn parses_regular_synced_lrc_as_line_spans_when_not_strict() {
        let raw = r#"
[00:10.00]Hello world
[00:15.00]Another line
"#;

        let words = parse_lrc_words(raw, false).expect("line-synced LRC should parse");
        assert_eq!(
            words,
            vec![
                WordSpan { start_ms: 10_000, text: "Hello world".to_owned() },
                WordSpan { start_ms: 15_000, text: "Another line".to_owned() },
            ]
        );
    }

    #[test]
    fn strict_mode_rejects_non_enhanced_input() {
        let raw = r#"
[00:10.00]Hello world
"#;

        let error = parse_lrc_words(raw, true).expect_err("strict mode should reject line sync");
        assert!(error.to_string().contains("strict word sync"));
    }

    #[test]
    fn active_word_lookup_uses_binary_search() {
        let lyrics = TrackLyrics::new(
            "Title",
            "Artist",
            vec![
                WordSpan { start_ms: 100, text: "one".into() },
                WordSpan { start_ms: 200, text: "two".into() },
                WordSpan { start_ms: 300, text: "three".into() },
            ],
        );

        assert_eq!(lyrics.active_word_index(50), None);
        assert_eq!(lyrics.active_word_index(100), Some(0));
        assert_eq!(lyrics.active_word_index(250), Some(1));
        assert_eq!(lyrics.active_word_index(400), Some(2));
    }
}
