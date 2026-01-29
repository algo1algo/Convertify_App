use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub category: PresetCategory,
    pub extension: String,
    pub format: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PresetCategory {
    Video,
    Audio,
    Image,
}

impl Preset {
    /// Build ffmpeg arguments for this preset
    pub fn build_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        
        // Output format
        if let Some(ref format) = self.format {
            args.push("-f".to_string());
            args.push(format.clone());
        }
        
        // Video codec
        if let Some(ref vcodec) = self.video_codec {
            args.push("-c:v".to_string());
            args.push(vcodec.clone());
        }
        
        // Audio codec
        if let Some(ref acodec) = self.audio_codec {
            args.push("-c:a".to_string());
            args.push(acodec.clone());
        }
        
        // Extra arguments
        args.extend(self.extra_args.clone());
        
        args
    }
}

/// Get all available presets
pub fn get_all_presets() -> Vec<Preset> {
    vec![
        // ===== VIDEO PRESETS =====
        Preset {
            id: "mp4_h264".to_string(),
            name: "MP4 (H.264)".to_string(),
            category: PresetCategory::Video,
            extension: "mp4".to_string(),
            format: Some("mp4".to_string()),
            video_codec: Some("libx264".to_string()),
            audio_codec: Some("aac".to_string()),
            extra_args: vec![
                "-preset".to_string(), "medium".to_string(),
                "-crf".to_string(), "23".to_string(),
            ],
        },
        Preset {
            id: "mp4_h265".to_string(),
            name: "MP4 (H.265/HEVC)".to_string(),
            category: PresetCategory::Video,
            extension: "mp4".to_string(),
            format: Some("mp4".to_string()),
            video_codec: Some("libx265".to_string()),
            audio_codec: Some("aac".to_string()),
            extra_args: vec![
                "-preset".to_string(), "medium".to_string(),
                "-crf".to_string(), "28".to_string(),
            ],
        },
        Preset {
            id: "webm_vp9".to_string(),
            name: "WebM (VP9)".to_string(),
            category: PresetCategory::Video,
            extension: "webm".to_string(),
            format: Some("webm".to_string()),
            video_codec: Some("libvpx-vp9".to_string()),
            audio_codec: Some("libopus".to_string()),
            extra_args: vec![
                "-crf".to_string(), "30".to_string(),
                "-b:v".to_string(), "0".to_string(),
            ],
        },
        Preset {
            id: "avi".to_string(),
            name: "AVI".to_string(),
            category: PresetCategory::Video,
            extension: "avi".to_string(),
            format: Some("avi".to_string()),
            video_codec: Some("mpeg4".to_string()),
            audio_codec: Some("mp3".to_string()),
            extra_args: vec![
                "-q:v".to_string(), "5".to_string(),
            ],
        },
        Preset {
            id: "mkv".to_string(),
            name: "MKV (H.264)".to_string(),
            category: PresetCategory::Video,
            extension: "mkv".to_string(),
            format: Some("matroska".to_string()),
            video_codec: Some("libx264".to_string()),
            audio_codec: Some("aac".to_string()),
            extra_args: vec![
                "-preset".to_string(), "medium".to_string(),
                "-crf".to_string(), "23".to_string(),
            ],
        },
        Preset {
            id: "mov".to_string(),
            name: "MOV (ProRes)".to_string(),
            category: PresetCategory::Video,
            extension: "mov".to_string(),
            format: Some("mov".to_string()),
            video_codec: Some("prores_ks".to_string()),
            audio_codec: Some("pcm_s16le".to_string()),
            extra_args: vec![
                "-profile:v".to_string(), "3".to_string(),
            ],
        },
        Preset {
            id: "gif".to_string(),
            name: "GIF (Animated)".to_string(),
            category: PresetCategory::Video,
            extension: "gif".to_string(),
            format: Some("gif".to_string()),
            video_codec: None,
            audio_codec: None,
            extra_args: vec![
                "-vf".to_string(), 
                "fps=15,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".to_string(),
            ],
        },
        
        // ===== AUDIO PRESETS =====
        Preset {
            id: "mp3".to_string(),
            name: "MP3".to_string(),
            category: PresetCategory::Audio,
            extension: "mp3".to_string(),
            format: Some("mp3".to_string()),
            video_codec: None,
            audio_codec: Some("libmp3lame".to_string()),
            extra_args: vec![
                "-q:a".to_string(), "2".to_string(),
                "-vn".to_string(),
            ],
        },
        Preset {
            id: "aac".to_string(),
            name: "AAC (M4A)".to_string(),
            category: PresetCategory::Audio,
            extension: "m4a".to_string(),
            format: Some("ipod".to_string()),
            video_codec: None,
            audio_codec: Some("aac".to_string()),
            extra_args: vec![
                "-b:a".to_string(), "192k".to_string(),
                "-vn".to_string(),
            ],
        },
        Preset {
            id: "flac".to_string(),
            name: "FLAC (Lossless)".to_string(),
            category: PresetCategory::Audio,
            extension: "flac".to_string(),
            format: Some("flac".to_string()),
            video_codec: None,
            audio_codec: Some("flac".to_string()),
            extra_args: vec!["-vn".to_string()],
        },
        Preset {
            id: "opus".to_string(),
            name: "Opus".to_string(),
            category: PresetCategory::Audio,
            extension: "opus".to_string(),
            format: Some("opus".to_string()),
            video_codec: None,
            audio_codec: Some("libopus".to_string()),
            extra_args: vec![
                "-b:a".to_string(), "128k".to_string(),
                "-vn".to_string(),
            ],
        },
        Preset {
            id: "wav".to_string(),
            name: "WAV (PCM)".to_string(),
            category: PresetCategory::Audio,
            extension: "wav".to_string(),
            format: Some("wav".to_string()),
            video_codec: None,
            audio_codec: Some("pcm_s16le".to_string()),
            extra_args: vec!["-vn".to_string()],
        },
        
        // ===== IMAGE PRESETS =====
        Preset {
            id: "png".to_string(),
            name: "PNG".to_string(),
            category: PresetCategory::Image,
            extension: "png".to_string(),
            format: Some("image2".to_string()),
            video_codec: Some("png".to_string()),
            audio_codec: None,
            extra_args: vec![
                "-frames:v".to_string(), "1".to_string(),
                "-an".to_string(),
            ],
        },
        Preset {
            id: "jpg".to_string(),
            name: "JPEG".to_string(),
            category: PresetCategory::Image,
            extension: "jpg".to_string(),
            format: Some("image2".to_string()),
            video_codec: Some("mjpeg".to_string()),
            audio_codec: None,
            extra_args: vec![
                "-frames:v".to_string(), "1".to_string(),
                "-q:v".to_string(), "2".to_string(),
                "-an".to_string(),
            ],
        },
        Preset {
            id: "webp".to_string(),
            name: "WebP".to_string(),
            category: PresetCategory::Image,
            extension: "webp".to_string(),
            format: Some("webp".to_string()),
            video_codec: Some("libwebp".to_string()),
            audio_codec: None,
            extra_args: vec![
                "-frames:v".to_string(), "1".to_string(),
                "-quality".to_string(), "80".to_string(),
                "-an".to_string(),
            ],
        },
    ]
}

/// Find a preset by ID
pub fn find_preset(id: &str) -> Option<Preset> {
    get_all_presets().into_iter().find(|p| p.id == id)
}
