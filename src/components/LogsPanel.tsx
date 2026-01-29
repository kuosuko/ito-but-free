import "./LogsPanel.css";
import { Terminal } from "lucide-react";
import { useEffect, useRef } from "react";

interface LogsPanelProps {
  logs: any[];
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
          CLEAR
        </button>
      </div>

      <div className="logs-content">
        {logs.length === 0 && (
          <span className="log-empty">No active events</span>
        )}
        {logs.map((log, i) => {
          const logStr = typeof log === "string" ? log : JSON.stringify(log);
          const timestamp = new Date().toLocaleTimeString();
          return (
            <div key={i} className="log-entry">
              <span className="log-timestamp">[{timestamp}]</span>
              <span className={`log-message ${logStr.includes("Error") ? "error" : ""}`}>
                {logStr}
              </span>
            </div>
          );
        })}
        <div ref={logsEndRef} />
      </div>
    </div>
  );
};

export default LogsPanel;
