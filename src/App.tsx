import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import "./App.css";

// Types
interface Preset {
  id: string;
  name: string;
  category: "video" | "audio" | "image";
  extension: string;
}

interface StreamInfo {
  index: number;
  stream_type: "video" | "audio" | "subtitle" | "data" | "attachment" | "unknown";
  codec_name: string | null;
  codec_long_name: string | null;
  width: number | null;
  height: number | null;
  frame_rate: string | null;
  sample_rate: string | null;
  channels: number | null;
  language: string | null;
  title: string | null;
}

interface MediaInfo {
  path: string;
  filename: string;
  format: {
    format_name: string;
    format_long_name: string;
    duration: number | null;
    size: number | null;
    bit_rate: number | null;
  };
  streams: StreamInfo[];
  has_video: boolean;
  has_audio: boolean;
  has_subtitles: boolean;
}

interface StreamSelection {
  include_video: boolean;
  include_audio: boolean;
  include_subtitles: boolean;
}

interface AdvancedOptions {
  format: string | null;
  video_codec: string | null;
  audio_codec: string | null;
  extra_args: string | null;
}

interface ConvertProgress {
  percent: number;
  time_secs: number;
  speed: string | null;
  bitrate: string | null;
  size_kb: number | null;
}

interface ConvertResult {
  success: boolean;
  output_path: string;
  duration_secs: number;
  message: string | null;
}

// FFmpeg format options
const FORMAT_OPTIONS = [
  { value: "", label: "Select format..." },
  { value: "mp4", label: "MP4" },
  { value: "mkv", label: "MKV (Matroska)" },
  { value: "webm", label: "WebM" },
  { value: "avi", label: "AVI" },
  { value: "mov", label: "MOV (QuickTime)" },
  { value: "flv", label: "FLV (Flash Video)" },
  { value: "wmv", label: "WMV" },
  { value: "mpeg", label: "MPEG" },
  { value: "mpegts", label: "MPEG-TS" },
  { value: "3gp", label: "3GP" },
  { value: "ogg", label: "OGG" },
  { value: "mp3", label: "MP3" },
  { value: "wav", label: "WAV" },
  { value: "flac", label: "FLAC" },
  { value: "aac", label: "AAC" },
  { value: "m4a", label: "M4A" },
  { value: "opus", label: "Opus" },
  { value: "gif", label: "GIF" },
  { value: "image2", label: "Image sequence" },
  { value: "rawvideo", label: "Raw video" },
  { value: "null", label: "Null (discard)" },
];

// Video codec options
const VIDEO_CODEC_OPTIONS = [
  { value: "", label: "Select video codec..." },
  { value: "copy", label: "Copy (no re-encode)" },
  { value: "libx264", label: "H.264 (libx264)" },
  { value: "libx265", label: "H.265/HEVC (libx265)" },
  { value: "libvpx", label: "VP8 (libvpx)" },
  { value: "libvpx-vp9", label: "VP9 (libvpx-vp9)" },
  { value: "libaom-av1", label: "AV1 (libaom)" },
  { value: "libsvtav1", label: "AV1 (SVT-AV1)" },
  { value: "mpeg4", label: "MPEG-4" },
  { value: "mpeg2video", label: "MPEG-2" },
  { value: "mpeg1video", label: "MPEG-1" },
  { value: "mjpeg", label: "MJPEG" },
  { value: "huffyuv", label: "HuffYUV (lossless)" },
  { value: "ffv1", label: "FFV1 (lossless)" },
  { value: "prores", label: "ProRes" },
  { value: "prores_ks", label: "ProRes (Kostya)" },
  { value: "dnxhd", label: "DNxHD" },
  { value: "png", label: "PNG" },
  { value: "libwebp", label: "WebP" },
  { value: "gif", label: "GIF" },
  { value: "rawvideo", label: "Raw video" },
  { value: "none", label: "No video (-vn)" },
];

// Audio codec options
const AUDIO_CODEC_OPTIONS = [
  { value: "", label: "Select audio codec..." },
  { value: "copy", label: "Copy (no re-encode)" },
  { value: "aac", label: "AAC" },
  { value: "libmp3lame", label: "MP3 (LAME)" },
  { value: "libopus", label: "Opus" },
  { value: "libvorbis", label: "Vorbis" },
  { value: "flac", label: "FLAC (lossless)" },
  { value: "alac", label: "ALAC (Apple lossless)" },
  { value: "pcm_s16le", label: "PCM 16-bit LE" },
  { value: "pcm_s24le", label: "PCM 24-bit LE" },
  { value: "pcm_s32le", label: "PCM 32-bit LE" },
  { value: "pcm_f32le", label: "PCM 32-bit Float" },
  { value: "ac3", label: "AC3 (Dolby Digital)" },
  { value: "eac3", label: "E-AC3 (Dolby Digital Plus)" },
  { value: "dts", label: "DTS" },
  { value: "wmav2", label: "WMA v2" },
  { value: "libfdk_aac", label: "AAC (Fraunhofer FDK)" },
  { value: "none", label: "No audio (-an)" },
];

// Common extra argument presets
const EXTRA_ARGS_OPTIONS = [
  { value: "", label: "Select preset or leave empty..." },
  { value: "-crf 18", label: "High quality (CRF 18)" },
  { value: "-crf 23", label: "Medium quality (CRF 23)" },
  { value: "-crf 28", label: "Low quality (CRF 28)" },
  { value: "-preset ultrafast", label: "Ultrafast encoding" },
  { value: "-preset fast", label: "Fast encoding" },
  { value: "-preset medium", label: "Medium encoding (default)" },
  { value: "-preset slow", label: "Slow encoding (better compression)" },
  { value: "-preset veryslow", label: "Very slow (best compression)" },
  { value: "-b:v 5M", label: "Video bitrate 5 Mbps" },
  { value: "-b:v 10M", label: "Video bitrate 10 Mbps" },
  { value: "-b:v 20M", label: "Video bitrate 20 Mbps" },
  { value: "-b:a 128k", label: "Audio bitrate 128 kbps" },
  { value: "-b:a 192k", label: "Audio bitrate 192 kbps" },
  { value: "-b:a 320k", label: "Audio bitrate 320 kbps" },
  { value: "-r 24", label: "Frame rate 24 fps" },
  { value: "-r 30", label: "Frame rate 30 fps" },
  { value: "-r 60", label: "Frame rate 60 fps" },
  { value: "-vf scale=1920:1080", label: "Scale to 1080p" },
  { value: "-vf scale=1280:720", label: "Scale to 720p" },
  { value: "-vf scale=640:480", label: "Scale to 480p" },
  { value: "-ss 00:00:00 -t 00:01:00", label: "First 1 minute" },
  { value: "-af volume=2.0", label: "Double audio volume" },
  { value: "-af loudnorm", label: "Normalize audio loudness" },
  { value: "-metadata title=", label: "Clear metadata" },
];

function App() {
  // State
  const [ffmpegError, setFfmpegError] = useState<string | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);
  
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [mediaInfo, setMediaInfo] = useState<MediaInfo | null>(null);
  const [outputPath, setOutputPath] = useState<string>("");
  
  const [streamSelection, setStreamSelection] = useState<StreamSelection>({
    include_video: true,
    include_audio: true,
    include_subtitles: true,
  });
  
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [advancedOptions, setAdvancedOptions] = useState<AdvancedOptions>({
    format: null,
    video_codec: null,
    audio_codec: null,
    extra_args: null,
  });
  
  const [isConverting, setIsConverting] = useState(false);
  const [progress, setProgress] = useState<ConvertProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  // Initialize
  useEffect(() => {
    checkFfmpeg();
    loadPresets();
    
    // Listen for progress events
    const unlistenProgress = listen<ConvertProgress>("convert-progress", (event) => {
      setProgress(event.payload);
    });
    
    const unlistenDone = listen<ConvertResult>("convert-done", (event) => {
      setIsConverting(false);
      setProgress(null);
      setSuccessMessage(`Conversion completed in ${event.payload.duration_secs.toFixed(1)}s`);
    });
    
    const unlistenError = listen<string>("convert-error", (event) => {
      setIsConverting(false);
      setProgress(null);
      setError(event.payload);
    });

    // Listen for Tauri drag-drop events
    const unlistenDragDrop = listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
      setIsDragging(false);
      if (event.payload.paths && event.payload.paths.length > 0) {
        loadFile(event.payload.paths[0]);
      }
    });

    const unlistenDragEnter = listen("tauri://drag-enter", () => {
      setIsDragging(true);
    });

    const unlistenDragLeave = listen("tauri://drag-leave", () => {
      setIsDragging(false);
    });
    
    return () => {
      unlistenProgress.then(f => f());
      unlistenDone.then(f => f());
      unlistenError.then(f => f());
      unlistenDragDrop.then(f => f());
      unlistenDragEnter.then(f => f());
      unlistenDragLeave.then(f => f());
    };
  }, []);

  // Check FFmpeg installation
  async function checkFfmpeg() {
    try {
      await invoke<string>("check_ffmpeg_installed");
      setFfmpegError(null);
    } catch (e) {
      setFfmpegError(String(e));
    }
  }

  // Load presets
  async function loadPresets() {
    try {
      const presetList = await invoke<Preset[]>("get_presets");
      setPresets(presetList);
      if (presetList.length > 0) {
        setSelectedPreset(presetList[0].id);
      }
    } catch (e) {
      console.error("Failed to load presets:", e);
    }
  }

  // Handle file selection
  const handleSelectFile = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Media Files", extensions: ["mp4", "mkv", "avi", "mov", "webm", "mp3", "wav", "flac", "aac", "ogg", "m4a", "png", "jpg", "jpeg", "gif", "webp"] },
          { name: "All Files", extensions: ["*"] }
        ]
      });
      
      if (selected) {
        await loadFile(selected as string);
      }
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Load file and probe it
  async function loadFile(path: string) {
    setError(null);
    setSuccessMessage(null);
    setInputPath(path);
    
    try {
      const info = await invoke<MediaInfo>("probe_media_file", { path });
      setMediaInfo(info);
      
      // Update stream selection based on available streams
      setStreamSelection({
        include_video: info.has_video,
        include_audio: info.has_audio,
        include_subtitles: info.has_subtitles,
      });
      
      // Generate default output path
      if (selectedPreset) {
        const outPath = await invoke<string>("get_output_path", {
          inputPath: path,
          presetId: selectedPreset,
        });
        setOutputPath(outPath);
      }
    } catch (e) {
      setError(`Failed to probe file: ${e}`);
      setMediaInfo(null);
    }
  }

  // Handle preset change
  async function handlePresetChange(presetId: string) {
    setSelectedPreset(presetId);
    
    if (inputPath) {
      const outPath = await invoke<string>("get_output_path", {
        inputPath,
        presetId,
      });
      setOutputPath(outPath);
    }
  }

  // Update output path when advanced format changes
  async function updateOutputPathForFormat(format: string | null) {
    if (inputPath && format) {
      const outPath = await invoke<string>("get_output_path", {
        inputPath,
        presetId: null,
        format,
      });
      setOutputPath(outPath);
    }
  }

  // Handle output path selection
  async function handleSelectOutput() {
    const preset = presets.find(p => p.id === selectedPreset);
    const selected = await save({
      defaultPath: outputPath,
      filters: preset ? [{ name: preset.name, extensions: [preset.extension] }] : undefined
    });
    
    if (selected) {
      setOutputPath(selected);
    }
  }

  // Start conversion
  async function startConversion() {
    if (!inputPath || !outputPath) return;
    
    setError(null);
    setSuccessMessage(null);
    setIsConverting(true);
    setProgress({ percent: 0, time_secs: 0, speed: null, bitrate: null, size_kb: null });
    
    try {
      await invoke<ConvertResult>("start_convert", {
        inputPath,
        outputPath,
        presetId: showAdvanced ? null : selectedPreset,
        advanced: showAdvanced ? {
          format: advancedOptions.format || null,
          video_codec: advancedOptions.video_codec || null,
          audio_codec: advancedOptions.audio_codec || null,
          extra_args: advancedOptions.extra_args || null,
        } : null,
        streamSelection: {
          include_video: streamSelection.include_video,
          include_audio: streamSelection.include_audio,
          include_subtitles: streamSelection.include_subtitles,
        },
      });
    } catch (e) {
      setIsConverting(false);
      setProgress(null);
      setError(String(e));
    }
  }

  // Cancel conversion
  async function cancelConversion() {
    try {
      await invoke("cancel_convert");
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }

  // Drag and drop handlers
  function handleDragOver(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(true);
  }

  function handleDragLeave(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(false);
  }

  async function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(false);
    
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      // Note: In Tauri, we need the actual path which may not be directly available
      // For now, prompt user to use the file picker
      setError("Please use the file picker to select files");
    }
  }

  // Format duration
  function formatDuration(seconds: number | null): string {
    if (seconds === null) return "Unknown";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  // Format file size
  function formatSize(bytes: number | null): string {
    if (bytes === null) return "Unknown";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  // Group presets by category
  const videoPresets = presets.filter(p => p.category === "video");
  const audioPresets = presets.filter(p => p.category === "audio");
  const imagePresets = presets.filter(p => p.category === "image");

  return (
    <main className="app">
      <header className="header">
        <h1>Convertify</h1>
        <p className="subtitle">Convert your files anyway you want - no restrictions</p>
      </header>

      {ffmpegError && (
        <div className="alert alert-error">
          <strong>FFmpeg not found!</strong> Please install FFmpeg to use this application.
          <br />
          <small>{ffmpegError}</small>
        </div>
      )}

      <div className="credits">
        <small>algo1algo made this</small>
      </div>

      <section className="section">
        <h2>1. Select Input File</h2>
        <div 
          className={`drop-zone ${isDragging ? "dragging" : ""} ${inputPath ? "has-file" : ""}`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={handleSelectFile}
        >
          {inputPath ? (
            <div className="file-info">
              <span className="file-icon">📁</span>
              <span className="file-name">{mediaInfo?.filename || inputPath}</span>
              <button className="btn-small" onClick={(e) => { e.stopPropagation(); handleSelectFile(); }}>
                Change
              </button>
            </div>
          ) : (
            <>
              <span className="drop-icon">📂</span>
              <span>Click to add or drag & drop</span>
            </>
          )}
        </div>

        {mediaInfo && (
          <div className="media-details">
            <div className="media-stats">
              <div className="stat">
                <span className="stat-label">Format</span>
                <span className="stat-value" title={mediaInfo.format.format_name}>
                  {mediaInfo.format.format_name.split(',')[0].toUpperCase()}
                </span>
              </div>
              <div className="stat">
                <span className="stat-label">Duration</span>
                <span className="stat-value">{formatDuration(mediaInfo.format.duration)}</span>
              </div>
              <div className="stat">
                <span className="stat-label">Size</span>
                <span className="stat-value">{formatSize(mediaInfo.format.size)}</span>
              </div>
              {mediaInfo.format.bit_rate && (
                <div className="stat">
                  <span className="stat-label">Bitrate</span>
                  <span className="stat-value">{Math.round(mediaInfo.format.bit_rate / 1000)} kbps</span>
                </div>
              )}
            </div>

            <div className="streams">
              <h4>Streams</h4>
              {mediaInfo.streams.map((stream) => (
                <div key={stream.index} className={`stream stream-${stream.stream_type}`}>
                  <span className="stream-type">
                    {stream.stream_type === "video" && "🎬"}
                    {stream.stream_type === "audio" && "🔊"}
                    {stream.stream_type === "subtitle" && "💬"}
                    {stream.stream_type === "data" && "📊"}
                    {" "}{stream.stream_type}
                  </span>
                  <span className="stream-codec">{stream.codec_name || "unknown"}</span>
                  {stream.width && stream.height && (
                    <span className="stream-detail">{stream.width}x{stream.height}</span>
                  )}
                  {stream.channels && (
                    <span className="stream-detail">{stream.channels}ch {stream.sample_rate}Hz</span>
                  )}
                  {stream.language && (
                    <span className="stream-detail">[{stream.language}]</span>
                  )}
                </div>
              ))}
            </div>

            <div className="stream-selection">
              <h4>Include Streams</h4>
              <div className="toggles">
                {mediaInfo.has_video && (
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={streamSelection.include_video}
                      onChange={(e) => setStreamSelection(s => ({ ...s, include_video: e.target.checked }))}
                      disabled={isConverting}
                    />
                    <span>Video</span>
                  </label>
                )}
                {mediaInfo.has_audio && (
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={streamSelection.include_audio}
                      onChange={(e) => setStreamSelection(s => ({ ...s, include_audio: e.target.checked }))}
                      disabled={isConverting}
                    />
                    <span>Audio</span>
                  </label>
                )}
                {mediaInfo.has_subtitles && (
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={streamSelection.include_subtitles}
                      onChange={(e) => setStreamSelection(s => ({ ...s, include_subtitles: e.target.checked }))}
                      disabled={isConverting}
                    />
                    <span>Subtitles</span>
                  </label>
                )}
              </div>
            </div>
          </div>
        )}
      </section>

      <section className="section">
        <h2>2. Choose Output Format</h2>
        
        <div className="mode-toggle">
          <button 
            className={`mode-btn ${!showAdvanced ? "active" : ""}`}
            onClick={() => setShowAdvanced(false)}
            disabled={isConverting}
          >
            Presets
          </button>
          <button 
            className={`mode-btn ${showAdvanced ? "active" : ""}`}
            onClick={() => setShowAdvanced(true)}
            disabled={isConverting}
          >
            Advanced
          </button>
        </div>

        {!showAdvanced ? (
          <div className="presets">
            <div className="preset-group">
              <h4>Video</h4>
              <div className="preset-buttons">
                {videoPresets.map(preset => (
                  <button
                    key={preset.id}
                    className={`preset-btn ${selectedPreset === preset.id ? "selected" : ""}`}
                    onClick={() => handlePresetChange(preset.id)}
                    disabled={isConverting}
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
            </div>
            <div className="preset-group">
              <h4>Audio</h4>
              <div className="preset-buttons">
                {audioPresets.map(preset => (
                  <button
                    key={preset.id}
                    className={`preset-btn ${selectedPreset === preset.id ? "selected" : ""}`}
                    onClick={() => handlePresetChange(preset.id)}
                    disabled={isConverting}
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
            </div>
            <div className="preset-group">
              <h4>Image</h4>
              <div className="preset-buttons">
                {imagePresets.map(preset => (
                  <button
                    key={preset.id}
                    className={`preset-btn ${selectedPreset === preset.id ? "selected" : ""}`}
                    onClick={() => handlePresetChange(preset.id)}
                    disabled={isConverting}
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="advanced-options">
            <div className="form-group">
              <label>Output Format</label>
              <select
                value={advancedOptions.format || ""}
                onChange={(e) => {
                  const format = e.target.value || null;
                  setAdvancedOptions(o => ({ ...o, format }));
                  updateOutputPathForFormat(format);
                }}
                disabled={isConverting}
              >
                {FORMAT_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
            <div className="form-group">
              <label>Video Codec</label>
              <select
                value={advancedOptions.video_codec || ""}
                onChange={(e) => setAdvancedOptions(o => ({ ...o, video_codec: e.target.value || null }))}
                disabled={isConverting}
              >
                {VIDEO_CODEC_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
            <div className="form-group">
              <label>Audio Codec</label>
              <select
                value={advancedOptions.audio_codec || ""}
                onChange={(e) => setAdvancedOptions(o => ({ ...o, audio_codec: e.target.value || null }))}
                disabled={isConverting}
              >
                {AUDIO_CODEC_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
            <div className="form-group">
              <label>Extra Options</label>
              <select
                value={advancedOptions.extra_args || ""}
                onChange={(e) => setAdvancedOptions(o => ({ ...o, extra_args: e.target.value || null }))}
                disabled={isConverting}
              >
                {EXTRA_ARGS_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
          </div>
        )}
      </section>

      <section className="section">
        <h2>3. Output</h2>
        <p className="output-hint">Output will be saved in the same folder as input file</p>
        <div className="output-path">
          <input
            type="text"
            value={outputPath}
            onChange={(e) => setOutputPath(e.target.value)}
            placeholder="Output path will be generated automatically..."
            disabled={isConverting}
            readOnly
          />
          <button onClick={handleSelectOutput} disabled={isConverting || !inputPath}>
            Change
          </button>
        </div>
      </section>

      {error && (
        <div className="alert alert-error">
          {error}
          <button className="alert-close" onClick={() => setError(null)}>×</button>
        </div>
      )}

      {successMessage && (
        <div className="alert alert-success">
          {successMessage}
          <button className="alert-close" onClick={() => setSuccessMessage(null)}>×</button>
        </div>
      )}

      {isConverting && progress && (
        <div className="progress-section">
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${progress.percent}%` }}
            />
          </div>
          <div className="progress-stats">
            <span>{progress.percent.toFixed(1)}%</span>
            {progress.speed && <span>Speed: {progress.speed}</span>}
            {progress.bitrate && <span>Bitrate: {progress.bitrate}</span>}
            {progress.size_kb && <span>Size: {formatSize(progress.size_kb * 1024)}</span>}
          </div>
        </div>
      )}

      <div className="actions">
        {!isConverting ? (
          <button 
            className="btn-primary btn-large"
            onClick={startConversion}
            disabled={!inputPath || !outputPath || !!ffmpegError}
          >
            Convert
          </button>
        ) : (
          <button 
            className="btn-danger btn-large"
            onClick={cancelConversion}
          >
            Cancel
          </button>
        )}
      </div>
    </main>
  );
}

export default App;
