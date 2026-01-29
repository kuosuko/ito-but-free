import "./Workspace.css";
import { Mic, Square, Activity } from "lucide-react";

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
                <Square size={14} fill="currentColor" />
                <span>STOP</span>
              </>
            ) : (
              <>
                <Mic size={14} />
                <span>RECORD</span>
              </>
            )}
          </button>

          <div className="status-indicator">
            <div className={`status-dot ${isRecording ? "listening" : "idle"}`} />
            <span className="status-text">
              {isRecording ? "LISTENING" : "IDLE"}
            </span>
          </div>
        </div>
      </div>

      {/* Content Area */}
      <div className="workspace-content">
        <div className="workspace-header">
          <div className="workspace-title">
            <Activity size={12} />
            <span>Buffer Preview</span>
          </div>
          <div className="workspace-actions">
            <button onClick={handleClear} className="workspace-action">
              Clear
            </button>
            <button onClick={handleCopy} className="workspace-action">
              Copy
            </button>
          </div>
        </div>

        <textarea
          className="workspace-textarea"
          value={transcription}
          onChange={(e) => onSetTranscription(e.target.value)}
          placeholder="// Ready to transcribe..."
          spellCheck={false}
        />
      </div>
    </div>
  );
};

export default Workspace;
