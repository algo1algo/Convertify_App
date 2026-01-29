use crate::presets::find_preset;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("FFmpeg not found. Please install FFmpeg.")]
    FfmpegNotFound,
    #[error("Input file not found: {0}")]
    InputNotFound(String),
    #[error("Preset not found: {0}")]
    PresetNotFound(String),
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Conversion cancelled")]
    Cancelled,
    #[error("Invalid output path: {0}")]
    InvalidOutputPath(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSelection {
    pub include_video: bool,
    pub include_audio: bool,
    pub include_subtitles: bool,
}

impl Default for StreamSelection {
    fn default() -> Self {
        Self {
            include_video: true,
            include_audio: true,
            include_subtitles: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedOptions {
    pub format: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub extra_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertOptions {
    pub input_path: String,
    pub output_path: String,
    pub preset_id: Option<String>,
    pub advanced: Option<AdvancedOptions>,
    pub stream_selection: Option<StreamSelection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConvertProgress {
    pub percent: f64,
    pub time_secs: f64,
    pub speed: Option<String>,
    pub bitrate: Option<String>,
    pub size_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConvertResult {
    pub success: bool,
    pub output_path: String,
    pub duration_secs: f64,
    pub message: Option<String>,
}

/// Check if ffmpeg is available
pub fn check_ffmpeg(sidecar_path: Option<&std::path::Path>) -> Result<String, ConvertError> {
    use std::process::Command;
    
    let ffmpeg_cmd = if let Some(path) = sidecar_path {
        path.to_string_lossy().to_string()
    } else {
        "ffmpeg".to_string()
    };
    
    let output = Command::new(&ffmpeg_cmd)
        .arg("-version")
        .output()
        .map_err(|_| ConvertError::FfmpegNotFound)?;
    
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        let first_line = version.lines().next().unwrap_or("ffmpeg version unknown");
        Ok(first_line.to_string())
    } else {
        Err(ConvertError::FfmpegNotFound)
    }
}

/// Build ffmpeg arguments from options
fn build_ffmpeg_args(options: &ConvertOptions) -> Result<Vec<String>, ConvertError> {
    let mut args: Vec<String> = Vec::new();
    
    // Input file
    args.push("-i".to_string());
    args.push(options.input_path.clone());
    
    // Stream selection flags
    let stream_sel = options.stream_selection.clone().unwrap_or_default();
    
    if !stream_sel.include_video {
        args.push("-vn".to_string());
    }
    if !stream_sel.include_audio {
        args.push("-an".to_string());
    }
    if !stream_sel.include_subtitles {
        args.push("-sn".to_string());
    }
    
    // Preset or advanced options
    if let Some(ref preset_id) = options.preset_id {
        let preset = find_preset(preset_id)
            .ok_or_else(|| ConvertError::PresetNotFound(preset_id.clone()))?;
        
        let preset_args = preset.build_args();
        args.extend(preset_args);
    }
    
    // Advanced options override preset
    if let Some(ref advanced) = options.advanced {
        if let Some(ref format) = advanced.format {
            args.push("-f".to_string());
            args.push(format.clone());
        }
        if let Some(ref vcodec) = advanced.video_codec {
            // Remove any existing -c:v if present
            if let Some(pos) = args.iter().position(|a| a == "-c:v") {
                args.remove(pos);
                if pos < args.len() {
                    args.remove(pos);
                }
            }
            args.push("-c:v".to_string());
            args.push(vcodec.clone());
        }
        if let Some(ref acodec) = advanced.audio_codec {
            // Remove any existing -c:a if present
            if let Some(pos) = args.iter().position(|a| a == "-c:a") {
                args.remove(pos);
                if pos < args.len() {
                    args.remove(pos);
                }
            }
            args.push("-c:a".to_string());
            args.push(acodec.clone());
        }
        if let Some(ref extra) = advanced.extra_args {
            // Parse extra args (split by whitespace, respecting quotes)
            let parsed = parse_extra_args(extra);
            args.extend(parsed);
        }
    }
    
    // Overwrite output without asking
    args.push("-y".to_string());
    
    // Output file
    args.push(options.output_path.clone());
    
    Ok(args)
}

/// Parse time string "HH:MM:SS.ms" to seconds
fn parse_time_str(time: &str) -> f64 {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() == 3 {
        let hours: f64 = parts[0].parse().unwrap_or(0.0);
        let minutes: f64 = parts[1].parse().unwrap_or(0.0);
        let seconds: f64 = parts[2].parse().unwrap_or(0.0);
        hours * 3600.0 + minutes * 60.0 + seconds
    } else {
        0.0
    }
}

/// Parse extra arguments string into a vector
fn parse_extra_args(extra: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    
    for c in extra.chars() {
        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
            }
            c if c == quote_char && in_quotes => {
                in_quotes = false;
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    
    if !current.is_empty() {
        args.push(current);
    }
    
    args
}

/// Get the duration of the input file in seconds
fn get_duration(input_path: &str, ffprobe_path: Option<&std::path::Path>) -> Option<f64> {
    crate::probe::probe_file(input_path, ffprobe_path)
        .ok()
        .and_then(|info| info.format.duration)
}

/// Start a conversion with progress reporting and logging
pub async fn start_conversion(
    app_handle: AppHandle,
    options: ConvertOptions,
    cancel_flag: Arc<AtomicBool>,
    ffmpeg_path: Option<std::path::PathBuf>,
    ffprobe_path: Option<std::path::PathBuf>,
    log_store: Arc<crate::logger::LogStore>,
) -> Result<ConvertResult, ConvertError> {
    use crate::logger::{ConversionLog, LogLevel as AppLogLevel};
    
    // Build ffmpeg arguments first to include in log
    let args = build_ffmpeg_args(&options)?;
    let ffmpeg_command = format!("ffmpeg {}", args.join(" "));
    
    // Create advanced options string for logging
    let advanced_str = options.advanced.as_ref().map(|a| {
        format!(
            "format={:?}, video_codec={:?}, audio_codec={:?}, extra_args={:?}",
            a.format, a.video_codec, a.audio_codec, a.extra_args
        )
    });
    
    // Create conversion log
    let mut conv_log = ConversionLog::new(
        &options.input_path,
        &options.output_path,
        options.preset_id.as_deref(),
        advanced_str,
        &ffmpeg_command,
    );
    
    conv_log.add_entry(AppLogLevel::Info, "Starting conversion", None);
    
    // Validate input file exists
    if !std::path::Path::new(&options.input_path).exists() {
        conv_log.add_entry(AppLogLevel::Error, "Input file not found", Some(&options.input_path));
        conv_log.finish(false, Some("Input file not found".to_string()));
        log_store.add_log(conv_log);
        return Err(ConvertError::InputNotFound(options.input_path.clone()));
    }
    
    // Validate output directory exists
    if let Some(parent) = std::path::Path::new(&options.output_path).parent() {
        if !parent.exists() {
            let err_msg = format!("Output directory does not exist: {}", parent.display());
            conv_log.add_entry(AppLogLevel::Error, &err_msg, None);
            conv_log.finish(false, Some(err_msg.clone()));
            log_store.add_log(conv_log);
            return Err(ConvertError::InvalidOutputPath(err_msg));
        }
    }
    
    // Log FFmpeg path
    if let Some(ref path) = ffmpeg_path {
        conv_log.add_entry(AppLogLevel::Debug, "Using bundled FFmpeg", Some(&path.display().to_string()));
    } else {
        conv_log.add_entry(AppLogLevel::Debug, "Using system FFmpeg", None);
    }
    
    // Get input duration for progress calculation
    let duration = get_duration(&options.input_path, ffprobe_path.as_deref());
    if let Some(dur) = duration {
        conv_log.add_entry(AppLogLevel::Info, &format!("Input duration: {:.2}s", dur), None);
    }
    
    let start_time = std::time::Instant::now();
    
    // If we have a sidecar path, add its directory to PATH so ffmpeg-sidecar can find it
    if let Some(ref path) = ffmpeg_path {
        if let Some(parent) = path.parent() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{}", parent.display(), current_path);
            std::env::set_var("PATH", new_path);
        }
    }
    
    let mut cmd = FfmpegCommand::new();
    
    for arg in &args {
        cmd.arg(arg);
    }
    
    conv_log.add_entry(AppLogLevel::Info, "Spawning FFmpeg process", None);
    
    // Spawn the process
    let mut child = cmd.spawn().map_err(|e| {
        let err_msg = format!("Failed to spawn ffmpeg: {}", e);
        conv_log.add_entry(AppLogLevel::Error, &err_msg, None);
        conv_log.finish(false, Some(err_msg.clone()));
        log_store.add_log(conv_log.clone());
        ConvertError::ConversionFailed(err_msg)
    })?;
    
    // Iterate over events
    let iter = child.iter().map_err(|e| {
        let err_msg = format!("Failed to get iterator: {}", e);
        conv_log.add_entry(AppLogLevel::Error, &err_msg, None);
        conv_log.finish(false, Some(err_msg.clone()));
        log_store.add_log(conv_log.clone());
        ConvertError::ConversionFailed(err_msg)
    })?;
    
    let mut last_error: Option<String> = None;
    let mut warning_count = 0;
    let mut error_count = 0;
    
    for event in iter {
        // Check cancellation
        if cancel_flag.load(Ordering::Relaxed) {
            child.kill().ok();
            conv_log.add_entry(AppLogLevel::Warning, "Conversion cancelled by user", None);
            conv_log.finish(false, Some("Cancelled".to_string()));
            log_store.add_log(conv_log);
            return Err(ConvertError::Cancelled);
        }
        
        match event {
            FfmpegEvent::Progress(progress) => {
                // Parse time from string format "HH:MM:SS.ms"
                let time_secs = parse_time_str(&progress.time);
                let percent = if let Some(dur) = duration {
                    if dur > 0.0 {
                        (time_secs / dur * 100.0).min(100.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                
                let progress_event = ConvertProgress {
                    percent,
                    time_secs,
                    speed: if progress.speed > 0.0 { Some(format!("{:.2}x", progress.speed)) } else { None },
                    bitrate: if progress.bitrate_kbps > 0.0 { Some(format!("{:.0} kbps", progress.bitrate_kbps)) } else { None },
                    size_kb: Some(progress.size_kb as u64),
                };
                
                let _ = app_handle.emit("convert-progress", &progress_event);
            }
            FfmpegEvent::Log(level, msg) => {
                match level {
                    LogLevel::Error | LogLevel::Fatal => {
                        error_count += 1;
                        conv_log.add_entry(AppLogLevel::Error, &msg, Some("FFmpeg"));
                        last_error = Some(msg);
                    }
                    LogLevel::Warning => {
                        warning_count += 1;
                        conv_log.add_entry(AppLogLevel::Warning, &msg, Some("FFmpeg"));
                    }
                    LogLevel::Info => {
                        conv_log.add_entry(AppLogLevel::Info, &msg, Some("FFmpeg"));
                    }
                    _ => {
                        // Log debug/verbose messages as debug
                        conv_log.add_entry(AppLogLevel::Debug, &msg, Some("FFmpeg"));
                    }
                }
            }
            FfmpegEvent::ParsedVersion(v) => {
                conv_log.add_entry(AppLogLevel::Info, &format!("FFmpeg version: {}", v.version), None);
            }
            FfmpegEvent::ParsedConfiguration(config) => {
                conv_log.add_entry(AppLogLevel::Debug, &format!("FFmpeg config: {:?}", config), None);
            }
            FfmpegEvent::ParsedInput(input) => {
                conv_log.add_entry(AppLogLevel::Info, &format!("Input #{}: duration={:?}s", input.index, input.duration), None);
            }
            FfmpegEvent::ParsedOutput(output) => {
                conv_log.add_entry(AppLogLevel::Info, &format!("Output #{}: {}", output.index, output.to), None);
            }
            FfmpegEvent::ParsedStreamMapping(mapping) => {
                conv_log.add_entry(AppLogLevel::Debug, &format!("Stream mapping: {}", mapping), None);
            }
            FfmpegEvent::Done => {
                conv_log.add_entry(AppLogLevel::Info, "FFmpeg process completed", None);
                break;
            }
            _ => {}
        }
    }
    
    // Wait for process to finish
    let status = child.wait().map_err(|e| {
        let err_msg = format!("Failed to wait for ffmpeg: {}", e);
        conv_log.add_entry(AppLogLevel::Error, &err_msg, None);
        conv_log.finish(false, Some(err_msg.clone()));
        log_store.add_log(conv_log.clone());
        ConvertError::ConversionFailed(err_msg)
    })?;
    
    let elapsed = start_time.elapsed().as_secs_f64();
    
    // Log summary
    conv_log.add_entry(AppLogLevel::Info, &format!("Conversion took {:.2}s", elapsed), None);
    if warning_count > 0 {
        conv_log.add_entry(AppLogLevel::Info, &format!("Total warnings: {}", warning_count), None);
    }
    if error_count > 0 {
        conv_log.add_entry(AppLogLevel::Info, &format!("Total errors: {}", error_count), None);
    }
    
    if status.success() {
        conv_log.add_entry(AppLogLevel::Info, "Conversion successful", None);
        conv_log.finish(true, None);
        log_store.add_log(conv_log);
        
        let result = ConvertResult {
            success: true,
            output_path: options.output_path,
            duration_secs: elapsed,
            message: None,
        };
        let _ = app_handle.emit("convert-done", &result);
        Ok(result)
    } else {
        let error_msg = last_error.unwrap_or_else(|| "Unknown error".to_string());
        conv_log.add_entry(AppLogLevel::Error, &format!("Conversion failed: {}", error_msg), None);
        conv_log.finish(false, Some(error_msg.clone()));
        log_store.add_log(conv_log);
        
        let _ = app_handle.emit("convert-error", &error_msg);
        Err(ConvertError::ConversionFailed(error_msg))
    }
}

/// Generate output path from input path and preset/format
/// Uses "_Convertified" postfix and adds number if file exists
pub fn generate_output_path(input_path: &str, preset_id: Option<&str>, format: Option<&str>) -> String {
    let path = std::path::Path::new(input_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    
    let extension = if let Some(preset_id) = preset_id {
        find_preset(preset_id)
            .map(|p| p.extension)
            .unwrap_or_else(|| "mp4".to_string())
    } else if let Some(fmt) = format {
        format_to_extension(fmt)
    } else {
        "mp4".to_string()
    };
    
    // Try base name first
    let base_output = parent.join(format!("{}_Convertified.{}", stem, extension));
    if !base_output.exists() {
        return base_output.to_string_lossy().to_string();
    }
    
    // If exists, add number suffix
    let mut counter = 2;
    loop {
        let output_path = parent.join(format!("{}_Convertified_{}.{}", stem, counter, extension));
        if !output_path.exists() {
            return output_path.to_string_lossy().to_string();
        }
        counter += 1;
        // Safety limit
        if counter > 9999 {
            return output_path.to_string_lossy().to_string();
        }
    }
}

/// Map format to common extension
fn format_to_extension(format: &str) -> String {
    match format {
        "mp4" => "mp4".to_string(),
        "mov" => "mov".to_string(),
        "matroska" | "mkv" => "mkv".to_string(),
        "webm" => "webm".to_string(),
        "avi" => "avi".to_string(),
        "flv" => "flv".to_string(),
        "wmv" => "wmv".to_string(),
        "mpeg" => "mpeg".to_string(),
        "mpegts" => "ts".to_string(),
        "3gp" => "3gp".to_string(),
        "mp3" => "mp3".to_string(),
        "flac" => "flac".to_string(),
        "wav" => "wav".to_string(),
        "ogg" => "ogg".to_string(),
        "opus" => "opus".to_string(),
        "aac" => "aac".to_string(),
        "m4a" | "ipod" => "m4a".to_string(),
        "gif" => "gif".to_string(),
        "image2" | "png" => "png".to_string(),
        "mjpeg" | "jpeg" | "jpg" => "jpg".to_string(),
        "webp" => "webp".to_string(),
        "rawvideo" => "raw".to_string(),
        "null" => "null".to_string(),
        _ => format.to_string(),
    }
}
