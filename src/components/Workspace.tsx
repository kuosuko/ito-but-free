import "./Workspace.css";
import { Mic, Square, Copy, Trash2 } from "lucide-react";

interface WorkspaceProps {
  isRecording: boolean;
  onToggleRecording: () => void;
  transcription: string;
  onSetTranscription: (text: string) => void;
}

const Workspace = ({
  isRecording,
  onToggleRecording,
  transcription,
  onSetTranscription,
}: WorkspaceProps) => {
  const handleClear = () => onSetTranscription("");

  const handleCopy = () => {
    navigator.clipboard.writeText(transcription);
  };

  return (
    <div className="workspace">
      {/* Toolbar */}
      <div className="workspace-toolbar">
        <div className="workspace-toolbar-left">
          <button
            onClick={onToggleRecording}
            className={`record-button ${isRecording ? "recording" : "idle"}`}
          >
            {isRecording ? (
              <>
                <Square size={12} fill="currentColor" />
                <span>Stop</span>
              </>
            ) : (
              <>
                <Mic size={14} />
                <span>Record</span>
              </>
            )}
          </button>

          <div className="status-indicator">
            <div className={`status-dot ${isRecording ? "listening" : "idle"}`} />
            <span className="status-text">
              {isRecording ? "Listening..." : "Ready"}
            </span>
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
          placeholder="Transcribed text will appear here..."
          spellCheck={false}
        />
      </div>
    </div>
  );
};

export default Workspace;
