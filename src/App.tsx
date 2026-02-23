import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
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
  has_data: boolean;
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

interface LogEntry {
  timestamp: string;
  level: "Info" | "Warning" | "Error" | "Debug";
  message: string;
  context: string | null;
}

interface ConversionLog {
  id: string;
  started_at: string;
  ended_at: string | null;
  input_path: string;
  output_path: string;
  preset_id: string | null;
  advanced_options: string | null;
  ffmpeg_command: string;
  success: boolean;
  error_message: string | null;
  entries: LogEntry[];
}

interface QueueItem {
  id: string;
  inputPath: string;
  inputFilename: string;
  mediaInfo: MediaInfo | null;
  presetId: string;
  outputName: string;
  outputFolder: string;
  status: "pending" | "converting" | "done" | "error";
  error?: string;
  outputPath?: string;
  progress?: ConvertProgress;
}

function App() {
  const [ffmpegError, setFfmpegError] = useState<string | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [defaultPresetId, setDefaultPresetId] = useState<string>("mp4");

  // Queue state
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [batchConverting, setBatchConverting] = useState(false);
  const [batchSummary, setBatchSummary] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Ref to track currently converting item id for event routing
  const currentItemIdRef = useRef<string | null>(null);

  // Log viewer state
  const [showLogs, setShowLogs] = useState(false);
  const [logs, setLogs] = useState<ConversionLog[]>([]);
  const [selectedLog, setSelectedLog] = useState<ConversionLog | null>(null);
  const [logFilePath, setLogFilePath] = useState<string | null>(null);

  useEffect(() => {
    checkFfmpeg();
    loadPresets();

    const unlistenProgress = listen<ConvertProgress>("convert-progress", (event) => {
      const itemId = currentItemIdRef.current;
      if (itemId) {
        setQueue(q => q.map(item =>
          item.id === itemId ? { ...item, progress: event.payload } : item
        ));
      }
    });

    const unlistenDragDrop = listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
      setIsDragging(false);
      if (event.payload.paths && event.payload.paths.length > 0) {
        addFilesToQueue(event.payload.paths);
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
      unlistenDragDrop.then(f => f());
      unlistenDragEnter.then(f => f());
      unlistenDragLeave.then(f => f());
    };
  }, []);

  async function checkFfmpeg() {
    try {
      await invoke<string>("check_ffmpeg_installed");
      setFfmpegError(null);
    } catch (e) {
      setFfmpegError(String(e));
    }
  }

  async function loadPresets() {
    try {
      const presetList = await invoke<Preset[]>("get_presets");
      setPresets(presetList);
      if (presetList.length > 0) {
        setDefaultPresetId(presetList[0].id);
      }
    } catch (e) {
      console.error("Failed to load presets:", e);
    }
  }

  function getPresetExtension(presetId: string): string {
    const preset = presets.find(p => p.id === presetId);
    return preset?.extension || "mp4";
  }

  function extractStem(filepath: string): string {
    const filename = filepath.split("/").pop() || filepath;
    const lastDot = filename.lastIndexOf(".");
    return lastDot > 0 ? filename.substring(0, lastDot) : filename;
  }

  function extractFolder(filepath: string): string {
    const lastSlash = filepath.lastIndexOf("/");
    return lastSlash > 0 ? filepath.substring(0, lastSlash) : ".";
  }

  function extractFilename(filepath: string): string {
    return filepath.split("/").pop() || filepath;
  }

  const handleAddFiles = useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          { name: "Media Files", extensions: ["mp4", "mkv", "avi", "mov", "webm", "mp3", "wav", "flac", "aac", "ogg", "m4a", "png", "jpg", "jpeg", "gif", "webp"] },
          { name: "All Files", extensions: ["*"] }
        ]
      });

      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        await addFilesToQueue(paths as string[]);
      }
    } catch (e) {
      setError(String(e));
    }
  }, [defaultPresetId, presets]);

  async function addFilesToQueue(paths: string[]) {
    setError(null);
    setBatchSummary(null);

    const newItems: QueueItem[] = [];
    for (const path of paths) {
      let mediaInfo: MediaInfo | null = null;
      try {
        mediaInfo = await invoke<MediaInfo>("probe_media_file", { path });
      } catch {
        // probe failed, still add to queue with null info
      }

      const stem = extractStem(path);
      const folder = extractFolder(path);

      newItems.push({
        id: crypto.randomUUID(),
        inputPath: path,
        inputFilename: extractFilename(path),
        mediaInfo,
        presetId: defaultPresetId,
        outputName: stem + "_Convertified",
        outputFolder: folder,
        status: "pending",
      });
    }

    setQueue(q => [...q, ...newItems]);
  }

  function updateQueueItem(id: string, updates: Partial<QueueItem>) {
    setQueue(q => q.map(item => item.id === id ? { ...item, ...updates } : item));
  }

  function removeQueueItem(id: string) {
    setQueue(q => q.filter(item => item.id !== id));
  }

  function clearQueue() {
    setQueue([]);
    setBatchSummary(null);
  }

  async function handleChangeOutputFolder(itemId: string) {
    const item = queue.find(i => i.id === itemId);
    if (!item) return;

    const selected = await open({
      directory: true,
      defaultPath: item.outputFolder,
    });

    if (selected) {
      updateQueueItem(itemId, { outputFolder: selected as string });
    }
  }

  async function startBatchConversion() {
    if (queue.length === 0) return;

    setBatchConverting(true);
    setBatchSummary(null);
    setError(null);

    // Reset all pending items
    setQueue(q => q.map(item =>
      item.status === "pending" || item.status === "error"
        ? { ...item, status: "pending" as const, error: undefined, progress: undefined, outputPath: undefined }
        : item
    ));

    let doneCount = 0;
    let errorCount = 0;
    const itemsToProcess = queue.filter(item => item.status === "pending" || item.status === "error");

    for (const item of itemsToProcess) {
      currentItemIdRef.current = item.id;
      const ext = getPresetExtension(item.presetId);
      const fullOutputPath = `${item.outputFolder}/${item.outputName}.${ext}`;

      updateQueueItem(item.id, {
        status: "converting",
        progress: { percent: 0, time_secs: 0, speed: null, bitrate: null, size_kb: null },
      });

      try {
        const result = await invoke<ConvertResult>("start_convert", {
          inputPath: item.inputPath,
          outputPath: fullOutputPath,
          presetId: item.presetId,
          advanced: null,
          streamSelection: {
            include_video: true,
            include_audio: true,
            include_subtitles: true,
            include_data: true,
          },
        });

        updateQueueItem(item.id, {
          status: "done",
          outputPath: result.output_path,
          progress: undefined,
        });
        doneCount++;
      } catch (e) {
        updateQueueItem(item.id, {
          status: "error",
          error: String(e),
          progress: undefined,
        });
        errorCount++;
      }
    }

    currentItemIdRef.current = null;
    setBatchConverting(false);

    const total = doneCount + errorCount;
    if (total > 0) {
      setBatchSummary(
        errorCount === 0
          ? `All ${doneCount} conversion${doneCount > 1 ? "s" : ""} completed successfully!`
          : `${doneCount}/${total} succeeded, ${errorCount} failed.`
      );
    }
  }

  async function cancelConversion() {
    try {
      await invoke("cancel_convert");
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }

  // Log viewer functions
  async function fetchLogs() {
    try {
      const fetchedLogs = await invoke<ConversionLog[]>("get_conversion_logs");
      setLogs(fetchedLogs);
      if (fetchedLogs.length > 0 && !selectedLog) {
        setSelectedLog(fetchedLogs[fetchedLogs.length - 1]);
      }
    } catch (e) {
      console.error("Failed to fetch logs:", e);
    }
  }

  async function exportLogs() {
    try {
      const exportedLogs = await invoke<string>("export_conversion_logs");
      await writeText(exportedLogs);
      alert("Logs copied to clipboard!");
    } catch (e) {
      console.error("Failed to export logs:", e);
      alert("Failed to export logs: " + String(e));
    }
  }

  async function clearLogs() {
    try {
      await invoke("clear_conversion_logs");
      setLogs([]);
      setSelectedLog(null);
    } catch (e) {
      console.error("Failed to clear logs:", e);
    }
  }

  async function openLogViewer() {
    fetchLogs();
    try {
      const path = await invoke<string | null>("get_log_file_path");
      setLogFilePath(path);
    } catch {
      setLogFilePath(null);
    }
    setShowLogs(true);
  }

  function formatSize(bytes: number | null): string {
    if (bytes === null) return "?";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function formatDuration(seconds: number | null): string {
    if (seconds === null) return "?";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  const pendingCount = queue.filter(i => i.status === "pending").length;
  const doneCount = queue.filter(i => i.status === "done").length;
  const errorQueueCount = queue.filter(i => i.status === "error").length;

  return (
    <main className="app">
      <header className="header">
        <div className="header-left">
          <h1>Convertify</h1>
          <p className="subtitle">Convert your files anyway you want - no restrictions</p>
        </div>
        <div className="header-center">
          <span className="header-branding">algo1algo made this</span>
        </div>
        <div className="header-right">
          <button className="btn-view-logs" onClick={openLogViewer}>View Logs</button>
        </div>
      </header>

      {ffmpegError && (
        <div className="alert alert-error">
          <strong>FFmpeg not found!</strong> Please install FFmpeg to use this application.
          <br />
          <small>{ffmpegError}</small>
        </div>
      )}

      {/* Section 1: Add Files */}
      <section className="section">
        <h2>
          1. Add Files
          {queue.length > 0 && <span className="queue-badge">{queue.length}</span>}
        </h2>
        <div
          className={`drop-zone ${isDragging ? "dragging" : ""}`}
          onClick={handleAddFiles}
        >
          <span className="drop-icon">📂</span>
          <span>Click to add files or drag & drop</span>
          <span className="drop-hint">You can select multiple files at once</span>
        </div>
      </section>

      {/* Section 2: Queue */}
      {queue.length > 0 && (
        <section className="section">
          <h2>2. Conversion Queue</h2>

          <div className="queue-table">
            <div className="queue-header-row">
              <span className="queue-col-status"></span>
              <span className="queue-col-input">Input</span>
              <span className="queue-col-format">Format</span>
              <span className="queue-col-name">Output Name</span>
              <span className="queue-col-folder">Output Folder</span>
              <span className="queue-col-actions"></span>
            </div>

            {queue.map(item => (
              <div key={item.id} className={`queue-row queue-row-${item.status}`}>
                <span className="queue-col-status">
                  {item.status === "pending" && <span className="queue-status-icon pending">&#9679;</span>}
                  {item.status === "converting" && <span className="queue-status-icon converting">&#9881;</span>}
                  {item.status === "done" && <span className="queue-status-icon done">&#10003;</span>}
                  {item.status === "error" && <span className="queue-status-icon error">&#10007;</span>}
                </span>

                <span className="queue-col-input" title={item.inputPath}>
                  <span className="queue-filename">{item.inputFilename}</span>
                  {item.mediaInfo && (
                    <span className="queue-file-meta">
                      {formatSize(item.mediaInfo.format.size)} &middot; {formatDuration(item.mediaInfo.format.duration)}
                    </span>
                  )}
                </span>

                <span className="queue-col-format">
                  <select
                    className="queue-select"
                    value={item.presetId}
                    onChange={(e) => {
                      updateQueueItem(item.id, { presetId: e.target.value });
                    }}
                    disabled={batchConverting}
                  >
                    {presets.map(p => (
                      <option key={p.id} value={p.id}>{p.name} (.{p.extension})</option>
                    ))}
                  </select>
                </span>

                <span className="queue-col-name">
                  <input
                    className="queue-input"
                    type="text"
                    value={item.outputName}
                    onChange={(e) => updateQueueItem(item.id, { outputName: e.target.value })}
                    disabled={batchConverting}
                  />
                  <span className="queue-ext">.{getPresetExtension(item.presetId)}</span>
                </span>

                <span className="queue-col-folder">
                  <span className="queue-folder-path" title={item.outputFolder}>
                    {item.outputFolder.split("/").pop() || item.outputFolder}
                  </span>
                  <button
                    className="btn-small"
                    onClick={() => handleChangeOutputFolder(item.id)}
                    disabled={batchConverting}
                  >
                    ...
                  </button>
                </span>

                <span className="queue-col-actions">
                  {item.status === "done" && item.outputPath && (
                    <button
                      className="btn-small"
                      title="Show in Folder"
                      onClick={() => revealItemInDir(item.outputPath!)}
                    >
                      📂
                    </button>
                  )}
                  <button
                    className="btn-small btn-remove"
                    onClick={() => removeQueueItem(item.id)}
                    disabled={batchConverting}
                    title="Remove"
                  >
                    ✕
                  </button>
                </span>

                {/* Per-item progress bar */}
                {item.status === "converting" && item.progress && (
                  <div className="queue-row-progress">
                    <div className="queue-progress-bar">
                      <div
                        className="queue-progress-fill"
                        style={{ width: `${item.progress.percent}%` }}
                      />
                    </div>
                    <span className="queue-progress-text">
                      {item.progress.percent.toFixed(0)}%
                      {item.progress.speed && ` \u00b7 ${item.progress.speed}`}
                    </span>
                  </div>
                )}

                {/* Error message */}
                {item.status === "error" && item.error && (
                  <div className="queue-row-error">{item.error}</div>
                )}
              </div>
            ))}
          </div>

          {/* Queue actions */}
          <div className="queue-actions">
            <div className="queue-actions-left">
              <button
                className="btn-secondary"
                onClick={clearQueue}
                disabled={batchConverting}
              >
                Clear Queue
              </button>
              <span className="queue-summary-text">
                {pendingCount} pending
                {doneCount > 0 && ` \u00b7 ${doneCount} done`}
                {errorQueueCount > 0 && ` \u00b7 ${errorQueueCount} failed`}
              </span>
            </div>
            <div className="queue-actions-right">
              {!batchConverting ? (
                <button
                  className="btn-primary btn-convert-all"
                  onClick={startBatchConversion}
                  disabled={pendingCount === 0 && errorQueueCount === 0 || !!ffmpegError}
                >
                  Convert All ({pendingCount + errorQueueCount})
                </button>
              ) : (
                <button
                  className="btn-danger btn-convert-all"
                  onClick={cancelConversion}
                >
                  Cancel
                </button>
              )}
            </div>
          </div>
        </section>
      )}

      {error && (
        <div className="alert alert-error">
          {error}
          <button className="alert-close" onClick={() => setError(null)}>×</button>
        </div>
      )}

      {batchSummary && (
        <div className={`alert ${batchSummary.includes("failed") ? "alert-error" : "alert-success"}`}>
          <span>{batchSummary}</span>
          <button className="alert-close" onClick={() => setBatchSummary(null)}>×</button>
        </div>
      )}

      <footer className="footer">
        Made with care by algo1algo
      </footer>

      {/* Log Viewer Modal */}
      {showLogs && (
        <div className="modal-overlay" onClick={() => setShowLogs(false)}>
          <div className="modal log-viewer" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>Conversion Logs</h2>
              <button className="modal-close" onClick={() => setShowLogs(false)}>×</button>
            </div>
            <div className="modal-body">
              <div className="log-actions">
                <button className="btn-secondary" onClick={fetchLogs}>Refresh</button>
                <button className="btn-secondary" onClick={exportLogs}>Copy to Clipboard</button>
                <button className="btn-danger" onClick={clearLogs}>Clear All</button>
              </div>
              {logFilePath && (
                <div className="log-file-path">
                  <small>
                    Logs are also saved to:{" "}
                    <button
                      type="button"
                      className="log-file-path-link"
                      onClick={() => revealItemInDir(logFilePath)}
                    >
                      {logFilePath}
                    </button>
                  </small>
                </div>
              )}
              {logs.length === 0 ? (
                <div className="no-logs">No conversion logs yet.</div>
              ) : (
                <div className="log-container">
                  <div className="log-list">
                    {logs.map((log) => (
                      <div
                        key={log.id}
                        className={`log-item ${selectedLog?.id === log.id ? "selected" : ""} ${log.success ? "success" : "failed"}`}
                        onClick={() => setSelectedLog(log)}
                      >
                        <div className="log-item-header">
                          <span className={`log-status ${log.success ? "success" : "error"}`}>
                            {log.success ? "\u2713" : "\u2717"}
                          </span>
                          <span className="log-time">{log.started_at}</span>
                        </div>
                        <div className="log-item-file">
                          {log.input_path.split("/").pop()}
                        </div>
                      </div>
                    ))}
                  </div>

                  {selectedLog && (
                    <div className="log-details">
                      <div className="log-detail-header">
                        <h3>Conversion Details</h3>
                        <span className={`log-status-badge ${selectedLog.success ? "success" : "error"}`}>
                          {selectedLog.success ? "Success" : "Failed"}
                        </span>
                      </div>

                      <div className="log-detail-info">
                        <div><strong>Started:</strong> {selectedLog.started_at}</div>
                        {selectedLog.ended_at && <div><strong>Ended:</strong> {selectedLog.ended_at}</div>}
                        <div><strong>Input:</strong> <code>{selectedLog.input_path}</code></div>
                        <div><strong>Output:</strong> <code>{selectedLog.output_path}</code></div>
                        {selectedLog.preset_id && <div><strong>Preset:</strong> {selectedLog.preset_id}</div>}
                        {selectedLog.advanced_options && (
                          <div><strong>Advanced:</strong> <code>{selectedLog.advanced_options}</code></div>
                        )}
                        {selectedLog.error_message && (
                          <div className="log-error-msg">
                            <strong>Error:</strong> {selectedLog.error_message}
                          </div>
                        )}
                      </div>

                      <div className="log-command">
                        <strong>FFmpeg Command:</strong>
                        <pre>{selectedLog.ffmpeg_command}</pre>
                      </div>

                      <div className="log-entries">
                        <strong>Log Entries ({selectedLog.entries.length}):</strong>
                        <div className="log-entries-list">
                          {selectedLog.entries.map((entry, idx) => (
                            <div key={idx} className={`log-entry log-entry-${entry.level.toLowerCase()}`}>
                              <span className="log-entry-time">{entry.timestamp}</span>
                              <span className={`log-entry-level ${entry.level.toLowerCase()}`}>{entry.level}</span>
                              <span className="log-entry-msg">{entry.message}</span>
                              {entry.context && <span className="log-entry-ctx">({entry.context})</span>}
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
