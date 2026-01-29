mod convert;
mod presets;
mod probe;

use convert::{
    check_ffmpeg, generate_output_path, start_conversion, AdvancedOptions, ConvertOptions,
    ConvertResult, StreamSelection,
};
use presets::{get_all_presets, Preset};
use probe::{check_ffprobe, probe_file, MediaInfo};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::Mutex;

/// Get the path to a sidecar binary (bundled FFmpeg/FFprobe)
pub fn get_sidecar_path(app: &tauri::AppHandle, name: &str) -> Option<std::path::PathBuf> {
    // Production: Tauri places sidecars in the MacOS directory (same as the main executable)
    // and strips the target suffix
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let prod_binary_name = if cfg!(target_os = "windows") {
                format!("{}.exe", name)
            } else {
                name.to_string()
            };
            let prod_path = exe_dir.join(&prod_binary_name);
            if prod_path.exists() {
                return Some(prod_path);
            }
        }
    }
    
    // Development: look in src-tauri/binaries with target suffix
    let target_suffix = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    
    let dev_binary_name = if cfg!(target_os = "windows") {
        format!("{}-{}.exe", name, target_suffix)
    } else {
        format!("{}-{}", name, target_suffix)
    };
    
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(&dev_binary_name);
    if dev_path.exists() {
        return Some(dev_path);
    }
    
    None
}

/// Shared state for cancellation
pub struct AppState {
    cancel_flag: Arc<AtomicBool>,
    converting: Arc<Mutex<bool>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            converting: Arc::new(Mutex::new(false)),
        }
    }
}

// ===== Tauri Commands =====

/// Get all available presets
#[tauri::command]
fn get_presets() -> Vec<Preset> {
    get_all_presets()
}

/// Check if ffmpeg is installed and return version
#[tauri::command]
fn check_ffmpeg_installed(app: tauri::AppHandle) -> Result<String, String> {
    let sidecar_path = get_sidecar_path(&app, "ffmpeg");
    check_ffmpeg(sidecar_path.as_deref()).map_err(|e| e.to_string())
}

/// Check if ffprobe is installed and return version
#[tauri::command]
fn check_ffprobe_installed(app: tauri::AppHandle) -> Result<String, String> {
    let sidecar_path = get_sidecar_path(&app, "ffprobe");
    check_ffprobe(sidecar_path.as_deref()).map_err(|e| e.to_string())
}

/// Probe a media file for info
#[tauri::command]
fn probe_media_file(app: tauri::AppHandle, path: String) -> Result<MediaInfo, String> {
    let sidecar_path = get_sidecar_path(&app, "ffprobe");
    probe_file(&path, sidecar_path.as_deref()).map_err(|e| e.to_string())
}

/// Generate output path from input and preset
#[tauri::command]
fn get_output_path(input_path: String, preset_id: Option<String>, format: Option<String>) -> String {
    generate_output_path(&input_path, preset_id.as_deref(), format.as_deref())
}

/// Start conversion
#[tauri::command]
async fn start_convert(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    output_path: String,
    preset_id: Option<String>,
    advanced: Option<AdvancedOptions>,
    stream_selection: Option<StreamSelection>,
) -> Result<ConvertResult, String> {
    // Check if already converting
    let mut converting = state.converting.lock().await;
    if *converting {
        return Err("A conversion is already in progress".to_string());
    }
    *converting = true;
    
    // Reset cancel flag
    state.cancel_flag.store(false, Ordering::Relaxed);
    
    let options = ConvertOptions {
        input_path,
        output_path,
        preset_id,
        advanced,
        stream_selection,
    };
    
    let cancel_flag = state.cancel_flag.clone();
    
    // Get sidecar paths
    let ffmpeg_path = get_sidecar_path(&app_handle, "ffmpeg");
    let ffprobe_path = get_sidecar_path(&app_handle, "ffprobe");
    
    // Run conversion
    let result = start_conversion(app_handle, options, cancel_flag, ffmpeg_path, ffprobe_path).await;
    
    // Mark as not converting
    *converting = false;
    
    result.map_err(|e| e.to_string())
}

/// Cancel the current conversion
#[tauri::command]
async fn cancel_convert(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::Relaxed);
    Ok(())
}

/// Check if a conversion is in progress
#[tauri::command]
async fn is_converting(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(*state.converting.lock().await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_presets,
            check_ffmpeg_installed,
            check_ffprobe_installed,
            probe_media_file,
            get_output_path,
            start_convert,
            cancel_convert,
            is_converting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
