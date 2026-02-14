import "./Workspace.css";
import { Mic, Square, Copy, Trash2, Loader } from "lucide-react";
import { useState, useEffect, useRef } from "react";

type RecordingState = "idle" | "recording" | "processing";

interface WorkspaceProps {
  recordingState: RecordingState;
  onToggleRecording: () => void;
  transcription: string;
  onSetTranscription: (text: string) => void;
}

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

const Workspace = ({
  recordingState,
  onToggleRecording,
  transcription,
  onSetTranscription,
}: WorkspaceProps) => {
  const [elapsed, setElapsed] = useState(0);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    if (recordingState === "recording") {
      setElapsed(0);
      intervalRef.current = window.setInterval(() => {
        setElapsed((prev) => prev + 1);
      }, 1000);
    } else {
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    }
    return () => {
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
      }
    };
  }, [recordingState]);

  const handleClear = () => onSetTranscription("");

  const handleCopy = () => {
    navigator.clipboard.writeText(transcription);
  };

  const statusLabel =
    recordingState === "recording"
      ? formatDuration(elapsed)
      : recordingState === "processing"
        ? "Transcribing..."
        : "Ready";

  return (
    <div className="workspace">
      {/* Toolbar */}
      <div className="workspace-toolbar">
        <div className="workspace-toolbar-left">
          <button
            onClick={onToggleRecording}
            disabled={recordingState === "processing"}
            className={`record-button ${recordingState}`}
          >
            {recordingState === "recording" ? (
              <>
                <Square size={12} fill="currentColor" />
                <span>Stop</span>
              </>
            ) : recordingState === "processing" ? (
              <>
                <Loader size={14} className="spinner" />
                <span>Transcribing...</span>
              </>
            ) : (
              <>
                <Mic size={14} />
                <span>Record</span>
              </>
            )}
          </button>

          <div className="status-indicator">
            <div className={`status-dot ${recordingState}`} />
            <span className="status-text">{statusLabel}</span>
          </div>
        </div>

        <div className="workspace-toolbar-right">
          <button
            onClick={handleCopy}
            className="toolbar-action"
            title="Copy to clipboard"
            disabled={!transcription}
          >
            <Copy size={14} />
          </button>
          <button
            onClick={handleClear}
            className="toolbar-action"
            title="Clear"
            disabled={!transcription}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      {/* Transcription Area */}
      <div className="workspace-content">
        <textarea
          className="workspace-textarea"
          value={transcription}
          onChange={(e) => onSetTranscription(e.target.value)}
          placeholder="Press your hotkey to start dictating..."
          spellCheck={false}
        />
      </div>
    </div>
  );
};

export default Workspace;
