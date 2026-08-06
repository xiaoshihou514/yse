//! Safe wrappers around the native libavformat/libavcodec media backend.

use crate::bridge as ffi;

/// Basic information discovered by FFmpeg without launching an external tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaInfo {
    /// Human-readable stream and duration summary.
    pub summary: String,
    /// Duration in milliseconds, or zero when the container does not report it.
    pub duration_ms: i64,
    /// Whether the file has at least one video stream.
    pub has_video: bool,
    /// Whether the file has at least one audio stream.
    pub has_audio: bool,
}

/// Output profile used by [`convert_media`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaPreset {
    /// H.264 video and AAC audio in an MP4 container.
    Mp4,
    /// VP9 video and Opus audio in a WebM container.
    WebM,
    /// MP3 audio; video is omitted.
    Mp3,
    /// Lossless FLAC audio; video is omitted.
    Flac,
}

impl MediaPreset {
    fn code(self) -> i32 {
        match self {
            Self::Mp4 => 0,
            Self::WebM => 1,
            Self::Mp3 => 2,
            Self::Flac => 3,
        }
    }
}

/// Inspect a media file through libavformat.
pub fn probe_media(path: &str) -> Result<MediaInfo, String> {
    let result = ffi::media_probe(path);
    if result.ok {
        Ok(MediaInfo {
            summary: result.summary,
            duration_ms: result.duration_ms,
            has_video: result.has_video,
            has_audio: result.has_audio,
        })
    } else {
        Err(result.summary)
    }
}

/// Transcode a file through libavformat/libavcodec on the calling thread.
///
/// This is deliberately synchronous so callers can choose their scheduler;
/// GUI applications should call it from a Yse background task.
pub fn convert_media(input: &str, output: &str, preset: MediaPreset) -> Result<(), String> {
    let result = ffi::media_convert(input, output, preset.code());
    if result.ok {
        Ok(())
    } else {
        Err(result.message)
    }
}
