use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::{DateTime, Local};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionLog {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub input_path: String,
    pub output_path: String,
    pub preset_id: Option<String>,
    pub advanced_options: Option<String>,
    pub ffmpeg_command: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub entries: Vec<LogEntry>,
}

impl ConversionLog {
    pub fn new(input_path: &str, output_path: &str, preset_id: Option<&str>, advanced_options: Option<String>, ffmpeg_command: &str) -> Self {
        let now: DateTime<Local> = Local::now();
        Self {
            id: format!("{}", now.timestamp_millis()),
            started_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            ended_at: None,
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            preset_id: preset_id.map(|s| s.to_string()),
            advanced_options,
            ffmpeg_command: ffmpeg_command.to_string(),
            success: false,
            error_message: None,
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, level: LogLevel, message: &str, context: Option<&str>) {
        let now: DateTime<Local> = Local::now();
        self.entries.push(LogEntry {
            timestamp: now.format("%H:%M:%S%.3f").to_string(),
            level,
            message: message.to_string(),
            context: context.map(|s| s.to_string()),
        });
    }

    pub fn finish(&mut self, success: bool, error_message: Option<String>) {
        let now: DateTime<Local> = Local::now();
        self.ended_at = Some(now.format("%Y-%m-%d %H:%M:%S").to_string());
        self.success = success;
        self.error_message = error_message;
    }
}

/// Format a single conversion log for file output
fn format_log_for_file(log: &ConversionLog) -> String {
    let mut output = String::new();
    output.push_str(&format!("=== Conversion {} ===\n", log.id));
    output.push_str(&format!("Started: {}\n", log.started_at));
    if let Some(ref ended) = log.ended_at {
        output.push_str(&format!("Ended: {}\n", ended));
    }
    output.push_str(&format!("Input: {}\n", log.input_path));
    output.push_str(&format!("Output: {}\n", log.output_path));
    if let Some(ref preset) = log.preset_id {
        output.push_str(&format!("Preset: {}\n", preset));
    }
    if let Some(ref advanced) = log.advanced_options {
        output.push_str(&format!("Advanced: {}\n", advanced));
    }
    output.push_str(&format!("Command: {}\n", log.ffmpeg_command));
    output.push_str(&format!("Success: {}\n", log.success));
    if let Some(ref error) = log.error_message {
        output.push_str(&format!("Error: {}\n", error));
    }
    output.push_str("\n--- Log Entries ---\n");
    for entry in &log.entries {
        let level_str = match entry.level {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        };
        output.push_str(&format!("[{}] [{}] {}", entry.timestamp, level_str, entry.message));
        if let Some(ref ctx) = entry.context {
            output.push_str(&format!(" ({})", ctx));
        }
        output.push('\n');
    }
    output.push_str("\n\n");
    output
}

/// Global log storage (in-memory and optional file in system log dir)
pub struct LogStore {
    logs: Mutex<Vec<ConversionLog>>,
    max_logs: usize,
    log_dir: Mutex<Option<PathBuf>>,
}

impl LogStore {
    pub fn new(max_logs: usize, log_dir: Option<PathBuf>) -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
            max_logs,
            log_dir: Mutex::new(log_dir),
        }
    }

    pub fn add_log(&self, log: ConversionLog) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(log.clone());
        // Keep only the last max_logs entries
        while logs.len() > self.max_logs {
            logs.remove(0);
        }
        drop(logs);

        // Append to log file in system folder if configured
        if let Ok(guard) = self.log_dir.lock() {
            if let Some(ref dir) = *guard {
                let path = dir.join("conversion_log.txt");
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    let _ = std::io::Write::write_all(&mut f, format_log_for_file(&log).as_bytes());
                }
            }
        }
    }

    pub fn get_logs(&self) -> Vec<ConversionLog> {
        self.logs.lock().unwrap().clone()
    }

    pub fn get_last_log(&self) -> Option<ConversionLog> {
        self.logs.lock().unwrap().last().cloned()
    }

    pub fn clear_logs(&self) {
        self.logs.lock().unwrap().clear();
    }

    pub fn export_logs(&self) -> String {
        let logs = self.logs.lock().unwrap();
        let mut output = String::new();
        
        for log in logs.iter() {
            output.push_str(&format!("=== Conversion {} ===\n", log.id));
            output.push_str(&format!("Started: {}\n", log.started_at));
            if let Some(ref ended) = log.ended_at {
                output.push_str(&format!("Ended: {}\n", ended));
            }
            output.push_str(&format!("Input: {}\n", log.input_path));
            output.push_str(&format!("Output: {}\n", log.output_path));
            if let Some(ref preset) = log.preset_id {
                output.push_str(&format!("Preset: {}\n", preset));
            }
            if let Some(ref advanced) = log.advanced_options {
                output.push_str(&format!("Advanced: {}\n", advanced));
            }
            output.push_str(&format!("Command: {}\n", log.ffmpeg_command));
            output.push_str(&format!("Success: {}\n", log.success));
            if let Some(ref error) = log.error_message {
                output.push_str(&format!("Error: {}\n", error));
            }
            output.push_str("\n--- Log Entries ---\n");
            for entry in &log.entries {
                let level_str = match entry.level {
                    LogLevel::Info => "INFO",
                    LogLevel::Warning => "WARN",
                    LogLevel::Error => "ERROR",
                    LogLevel::Debug => "DEBUG",
                };
                output.push_str(&format!("[{}] [{}] {}", entry.timestamp, level_str, entry.message));
                if let Some(ref ctx) = entry.context {
                    output.push_str(&format!(" ({})", ctx));
                }
                output.push('\n');
            }
            output.push_str("\n\n");
        }
        
        output
    }

    /// Path to the log file in the system log folder, if file logging is enabled
    pub fn get_log_file_path(&self) -> Option<PathBuf> {
        self.log_dir.lock().ok().and_then(|g| g.as_ref().cloned()).map(|d| d.join("conversion_log.txt"))
    }
}

impl Default for LogStore {
    fn default() -> Self {
        Self::new(50, None) // Keep last 50 conversion logs, no file logging by default
    }
}
