import "./LogsPanel.css";
import { Terminal } from "lucide-react";
import { useEffect, useRef } from "react";

interface LogEntry {
  level: string;
  message: string;
  timestamp: string;
}

interface LogsPanelProps {
  logs: LogEntry[];
  onClearLogs: () => void;
}

const LogsPanel = ({ logs, onClearLogs }: LogsPanelProps) => {
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  return (
    <div className="logs-panel">
      <div className="logs-header">
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <Terminal size={12} />
          <span>Console</span>
        </div>
        <button onClick={onClearLogs} className="clear-button">
          Clear
        </button>
      </div>

      <div className="logs-content">
        {logs.length === 0 && (
          <span className="log-empty">No events yet</span>
        )}
        {logs.map((log, i) => (
          <div key={i} className="log-entry">
            <span className="log-timestamp">{log.timestamp}</span>
            <span className={`log-level ${log.level}`}>{log.level}</span>
            <span className={`log-message ${log.level}`}>
              {log.message}
            </span>
          </div>
        ))}
        <div ref={logsEndRef} />
      </div>
    </div>
  );
};

export default LogsPanel;
