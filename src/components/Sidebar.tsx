import "./Sidebar.css";
import { Settings, Key, Eye, EyeOff } from "lucide-react";
import { useState } from "react";

interface SidebarProps {
  apiKey: string;
  onSaveApiKey: (key: string) => void;
  hotkey: string;
  onSaveHotkey: (hk: string) => void;
  triggerMode: string;
  onSetTriggerMode: (mode: string) => void;
  autoTypeEnabled: boolean;
  onToggleAutoType: (enabled: boolean) => void;
  typingSpeed: number;
  onSetTypingSpeed: (speed: number) => void;
  fnKeyEnabled: boolean;
  onToggleFnKey: (enabled: boolean) => void;
  hasAccessibilityPermission: boolean;
  onRequestPermission: () => void;
  refineOutputEnabled: boolean;
  onToggleRefineOutput: (enabled: boolean) => void;
  refinementPrompt: string;
  onSetRefinementPrompt: (prompt: string) => void;
  refinementModel: string;
  onSetRefinementModel: (model: string) => void;
}

const Sidebar = ({
  apiKey,
  onSaveApiKey,
  hotkey,
  onSaveHotkey,
  triggerMode,
  onSetTriggerMode,
  autoTypeEnabled,
  onToggleAutoType,
  typingSpeed,
  onSetTypingSpeed,
  fnKeyEnabled,
  onToggleFnKey,
  hasAccessibilityPermission,
  onRequestPermission,
  refineOutputEnabled,
  onToggleRefineOutput,
  refinementPrompt,
  onSetRefinementPrompt,
  refinementModel,
  onSetRefinementModel,
}: SidebarProps) => {
  const [localApiKey, setLocalApiKey] = useState(apiKey);
  const [localHotkey, setLocalHotkey] = useState(hotkey);
  const [localRefinementPrompt, setLocalRefinementPrompt] = useState(refinementPrompt);
  const [localRefinementModel, setLocalRefinementModel] = useState(refinementModel);
  const [showKey, setShowKey] = useState(false);

  return (
    <div className="sidebar">
      <h2 className="sidebar-header">
        <Settings className="icon" size={14} /> Settings
      </h2>

      {/* Groq API */}
      <div className="sidebar-group">
        <h3>Groq API Key</h3>
        <div className="input-group">
          <input
            className="input"
            type={showKey ? "text" : "password"}
            placeholder="gsk_..."
            value={localApiKey}
            onChange={(e) => setLocalApiKey(e.target.value)}
            onBlur={() => onSaveApiKey(localApiKey)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onSaveApiKey(localApiKey);
            }}
          />
          <button
            className="icon-button"
            onClick={() => setShowKey(!showKey)}
            title={showKey ? "Hide API key" : "Show API key"}
          >
            {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
          </button>
        </div>
      </div>

      {/* Input Trigger */}
      <div className="sidebar-group">
        <h3>Input Trigger</h3>

        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={fnKeyEnabled}
            onChange={(e) => onToggleFnKey(e.target.checked)}
          />
          Use Fn key
        </label>

        {!fnKeyEnabled && (
          <>
            <label className="label">Hotkey</label>
            <input
              className="input"
              type="text"
              value={localHotkey}
              onChange={(e) => setLocalHotkey(e.target.value)}
              onBlur={() => onSaveHotkey(localHotkey)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onSaveHotkey(localHotkey);
              }}
              placeholder="CommandOrControl+Shift+R"
            />
          </>
        )}

        <label className="label">Trigger Mode</label>
        <div className="segmented-control">
          <button
            className={`segment ${triggerMode === "hold" ? "active" : ""}`}
            onClick={() => onSetTriggerMode("hold")}
          >
            Hold
          </button>
          <button
            className={`segment ${triggerMode === "toggle" ? "active" : ""}`}
            onClick={() => onSetTriggerMode("toggle")}
          >
            Toggle
          </button>
        </div>
      </div>

      {/* Output Injection */}
      <div className="sidebar-group">
        <h3>Output</h3>

        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={autoTypeEnabled}
            onChange={(e) => onToggleAutoType(e.target.checked)}
          />
          Auto-type into active app
        </label>

        {autoTypeEnabled && (
          <>
            <label className="label">
              Typing delay: {typingSpeed}ms
            </label>
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={typingSpeed}
              onChange={(e) => onSetTypingSpeed(Number(e.target.value))}
              className="slider"
            />

            {!hasAccessibilityPermission && (
              <button
                className="permission-button"
                onClick={onRequestPermission}
              >
                <Key size={12} /> Grant Accessibility Permission
              </button>
            )}
          </>
        )}
      </div>

      {/* Refinement Settings */}
      <div className="sidebar-group">
        <h3>Refinement</h3>

        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={refineOutputEnabled}
            onChange={(e) => onToggleRefineOutput(e.target.checked)}
          />
          Refine with LLM (Groq)
        </label>

        {refineOutputEnabled && (
          <>
            <label className="label">Model</label>
            <input
              className="input"
              type="text"
              value={localRefinementModel}
              onChange={(e) => setLocalRefinementModel(e.target.value)}
              onBlur={() => onSetRefinementModel(localRefinementModel)}
              placeholder="qwen/qwen3-32b"
            />

            <label className="label">Prompt</label>
            <textarea
              className="input textarea"
              rows={3}
              value={localRefinementPrompt}
              onChange={(e) => setLocalRefinementPrompt(e.target.value)}
              onBlur={() => onSetRefinementPrompt(localRefinementPrompt)}
              placeholder="Style or language preferences (e.g., formal tone, Traditional Chinese)"
            />
          </>
        )}
      </div>
    </div>
  );
};

export default Sidebar;
