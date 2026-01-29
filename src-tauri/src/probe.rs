use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("FFprobe not found. Please install FFmpeg.")]
    FfprobeNotFound,
    #[error("Failed to execute ffprobe: {0}")]
    ExecutionFailed(String),
    #[error("Failed to parse ffprobe output: {0}")]
    ParseFailed(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub path: String,
    pub filename: String,
    pub format: FormatInfo,
    pub streams: Vec<StreamInfo>,
    pub has_video: bool,
    pub has_audio: bool,
    pub has_subtitles: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format_name: String,
    pub format_long_name: String,
    pub duration: Option<f64>,
    pub size: Option<u64>,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub index: u32,
    pub stream_type: StreamType,
    pub codec_name: Option<String>,
    pub codec_long_name: Option<String>,
    // Video specific
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub pix_fmt: Option<String>,
    // Audio specific
    pub sample_rate: Option<String>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    // Subtitle specific
    pub language: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
    Unknown,
}

impl From<&str> for StreamType {
    fn from(s: &str) -> Self {
        match s {
            "video" => StreamType::Video,
            "audio" => StreamType::Audio,
            "subtitle" => StreamType::Subtitle,
            "data" => StreamType::Data,
            "attachment" => StreamType::Attachment,
            _ => StreamType::Unknown,
        }
    }
}

// FFprobe JSON output structures
#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    filename: Option<String>,
    format_name: Option<String>,
    format_long_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    index: Option<u32>,
    codec_type: Option<String>,
    codec_name: Option<String>,
    codec_long_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    pix_fmt: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    channel_layout: Option<String>,
    tags: Option<FfprobeStreamTags>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStreamTags {
    language: Option<String>,
    title: Option<String>,
}

/// Check if ffprobe is available
pub fn check_ffprobe(sidecar_path: Option<&std::path::Path>) -> Result<String, ProbeError> {
    let ffprobe_cmd = if let Some(path) = sidecar_path {
        path.to_string_lossy().to_string()
    } else {
        "ffprobe".to_string()
    };
    
    let output = Command::new(&ffprobe_cmd)
        .arg("-version")
        .output()
        .map_err(|_| ProbeError::FfprobeNotFound)?;
    
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        let first_line = version.lines().next().unwrap_or("ffprobe version unknown");
        Ok(first_line.to_string())
    } else {
        Err(ProbeError::FfprobeNotFound)
    }
}

/// Probe a media file and return its info
pub fn probe_file(path: &str, sidecar_path: Option<&std::path::Path>) -> Result<MediaInfo, ProbeError> {
    // Check if file exists
    if !std::path::Path::new(path).exists() {
        return Err(ProbeError::FileNotFound(path.to_string()));
    }
    
    let ffprobe_cmd = if let Some(p) = sidecar_path {
        p.to_string_lossy().to_string()
    } else {
        "ffprobe".to_string()
    };
    
    // Run ffprobe
    let output = Command::new(&ffprobe_cmd)
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .map_err(|e| ProbeError::ExecutionFailed(e.to_string()))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ProbeError::ExecutionFailed(stderr.to_string()));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let probe_output: FfprobeOutput = serde_json::from_str(&stdout)
        .map_err(|e| ProbeError::ParseFailed(e.to_string()))?;
    
    // Parse format info
    let format = probe_output.format.ok_or_else(|| {
        ProbeError::ParseFailed("Missing format info".to_string())
    })?;
    
    let format_info = FormatInfo {
        format_name: format.format_name.unwrap_or_default(),
        format_long_name: format.format_long_name.unwrap_or_default(),
        duration: format.duration.and_then(|d| d.parse().ok()),
        size: format.size.and_then(|s| s.parse().ok()),
        bit_rate: format.bit_rate.and_then(|b| b.parse().ok()),
    };
    
    // Parse streams
    let streams: Vec<StreamInfo> = probe_output
        .streams
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let stream_type = StreamType::from(s.codec_type.as_deref().unwrap_or("unknown"));
            let tags = s.tags.unwrap_or(FfprobeStreamTags {
                language: None,
                title: None,
            });
            
            StreamInfo {
                index: s.index.unwrap_or(0),
                stream_type,
                codec_name: s.codec_name,
                codec_long_name: s.codec_long_name,
                width: s.width,
                height: s.height,
                frame_rate: s.r_frame_rate,
                pix_fmt: s.pix_fmt,
                sample_rate: s.sample_rate,
                channels: s.channels,
                channel_layout: s.channel_layout,
                language: tags.language,
                title: tags.title,
            }
        })
        .collect();
    
    let has_video = streams.iter().any(|s| s.stream_type == StreamType::Video);
    let has_audio = streams.iter().any(|s| s.stream_type == StreamType::Audio);
    let has_subtitles = streams.iter().any(|s| s.stream_type == StreamType::Subtitle);
    
    // Extract filename from path
    let filename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    
    Ok(MediaInfo {
        path: path.to_string(),
        filename,
        format: format_info,
        streams,
        has_video,
        has_audio,
        has_subtitles,
    })
}
