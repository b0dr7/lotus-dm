//! FDM-style media downloads: video and audio with quality choices.
//!
//! Anything that is not a torrent, FTP or plain file goes through here when it
//! is recognised as media (YouTube / Vimeo links). The quality pick (a video
//! height such as `1080p` or an audio format such as `mp3`) selects the media
//! stream, so the transfer is owned by the external `yt-dlp` binary instead of
//! the ranged HTTP worker, and the outcome is reported back over the same
//! engine event channel.

use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Sender;

use crate::core::queue::DownloadJob;

use super::{emit_state, fail, Control, EngineState, Event};

pub const VIDEO_HEIGHTS: &[u32] = &[2160, 1440, 1080, 720, 480, 360, 240];
pub const AUDIO_FORMATS: &[&str] = &["mp3", "aac", "opus", "flac", "wav"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Clone, Debug)]
pub struct Quality {
    pub kind: MediaKind,
    pub label: String,
    pub ext: String,
    pub height: Option<u32>,
}

pub fn video_qualities() -> Vec<Quality> {
    vec![
        Quality { kind: MediaKind::Video, label: "2160p".to_string(), ext: "mp4".to_string(), height: Some(2160) },
        Quality { kind: MediaKind::Video, label: "1440p".to_string(), ext: "mp4".to_string(), height: Some(1440) },
        Quality { kind: MediaKind::Video, label: "1080p".to_string(), ext: "mp4".to_string(), height: Some(1080) },
        Quality { kind: MediaKind::Video, label: "720p".to_string(), ext: "mp4".to_string(), height: Some(720) },
        Quality { kind: MediaKind::Video, label: "480p".to_string(), ext: "mp4".to_string(), height: Some(480) },
        Quality { kind: MediaKind::Video, label: "360p".to_string(), ext: "mp4".to_string(), height: Some(360) },
        Quality { kind: MediaKind::Video, label: "240p".to_string(), ext: "mp4".to_string(), height: Some(240) },
    ]
}

pub fn audio_qualities() -> Vec<Quality> {
    vec![
        Quality { kind: MediaKind::Audio, label: "mp3".to_string(), ext: "mp3".to_string(), height: None },
        Quality { kind: MediaKind::Audio, label: "aac".to_string(), ext: "aac".to_string(), height: None },
        Quality { kind: MediaKind::Audio, label: "opus".to_string(), ext: "opus".to_string(), height: None },
        Quality { kind: MediaKind::Audio, label: "flac".to_string(), ext: "flac".to_string(), height: None },
        Quality { kind: MediaKind::Audio, label: "wav".to_string(), ext: "wav".to_string(), height: None },
    ]
}

/// True for links the media engine owns: the video/audio hosts FDM-style
/// downloaders cover (YouTube family + Vimeo). Plain `http(s)` links fall
/// through to the ranged HTTP worker.
pub fn is_media(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("https://www.youtube.com/")
        || lower.starts_with("https://youtube.com/")
        || lower.starts_with("https://m.youtube.com/")
        || lower.starts_with("https://music.youtube.com/")
        || lower.starts_with("https://youtube-nocookie.com/")
        || lower.starts_with("https://youtu.be/")
        || lower.starts_with("https://vimeo.com/")
}

/// The kind of media a quality pick describes.
fn media_kind_of(pick: &str) -> MediaKind {
    if AUDIO_FORMATS
        .iter()
        .any(|format| format.eq_ignore_ascii_case(pick))
    {
        MediaKind::Audio
    } else {
        MediaKind::Video
    }
}

/// Resolve a quality pick (`1080p`, `mp3`, ...) into a `Quality`, falling back
/// to the FDM-ish middle default (720p video / mp3 audio).
pub fn resolve(kind: MediaKind, pick: &str) -> Quality {
    if kind == MediaKind::Audio {
        let ext = AUDIO_FORMATS
            .iter()
            .find(|format| format.eq_ignore_ascii_case(pick))
            .copied()
            .unwrap_or("mp3");
        return Quality {
            kind,
            label: ext.to_string(),
            ext: ext.to_string(),
            height: None,
        };
    }
    let height = VIDEO_HEIGHTS
        .iter()
        .copied()
        .find(|h| format!("{h}p").eq_ignore_ascii_case(&pick.trim()))
        .unwrap_or(720);
    Quality {
        kind,
        label: format!("{height}p"),
        ext: "mp4".to_string(),
        height: Some(height),
    }
}

/// Final file name for a media download: the video stream gets a `-<height>`
/// qualifier (`Name-1080p.%(ext)s`) so quality never collides with another pick;
/// the audio stream just takes its format.
pub fn output_name(url: &str, quality: &Quality) -> String {
    let base = url.split(['?', '#']).next().unwrap_or("");
    let stem = Path::new(base)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "download".to_string());
    if quality.kind == MediaKind::Audio {
        return format!("{stem}.{}", quality.ext);
    }
    format!("{}-{}.%(ext)s", stem, quality.label)
}

/// Hand a media job to the external media downloader. Runs to completion
/// unless cancelled; the job keeps the normal queue state machine.
pub(super) fn run(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    if control.cancelled() {
        fail(
            &tx,
            &job.id,
            "media download cancelled".to_string(),
        );
        return;
    }
    let pick = job.quality.as_deref().unwrap_or("720p");
    let kind = media_kind_of(pick);
    let quality = resolve(kind, pick);

    let exe = std::env::var("LOTUS_YTDLP").unwrap_or_else(|_| "yt-dlp".to_string());
    let mut command = Command::new(&exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: console-subsystem children spawned from a GUI
        // process would otherwise get their own console window each run.
        command.creation_flags(0x0000_0080);
    }
    command.arg("--no-playlist");
    if kind == MediaKind::Audio {
        command
            .arg("-f")
            .arg("bestaudio/best")
            .arg("--extract-audio")
            .arg("--audio-format")
            .arg(&quality.ext);
    } else {
        let height = quality.height.unwrap_or(720);
        command.arg("-f").arg(format!("bestvideo[height<={height}]+bestaudio/best[height<={height}]"));
    }
    command
        .arg("-o")
        .arg(format!("{}/{}", job.folder.replace('\\', "/"), output_name(&job.url, &quality)))
        .arg(&job.url);

    let output = match command.output() {
        Ok(output) => output,
        Err(err) => {
            fail(&tx, &job.id, format!("yt-dlp failed to start: {err}"));
            return;
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let reason = stderr
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .last()
            .map(str::to_string)
            .unwrap_or_else(|| format!("yt-dlp exited with {}", output.status));
        fail(&tx, &job.id, format!("yt-dlp: {reason}"));
        return;
    }

    emit_state(&tx, &job.id, EngineState::Done);
}
