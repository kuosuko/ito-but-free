import "./Sidebar.css";
import { Settings, Save, Key } from "lucide-react";
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
}: SidebarProps) => {
  const [localApiKey, setLocalApiKey] = useState(apiKey);
  const [localHotkey, setLocalHotkey] = useState(hotkey);
  const [showKey, setShowKey] = useState(false);

  return (
    <div className="sidebar">
      <h2 className="sidebar-header">
        <Settings className="icon" size={14} /> Configuration
      </h2>

      {/* Groq API */}
      <div className="sidebar-group">
        <h3>Groq API</h3>
        <div className="input-group">
          <input
            className="input"
            type={showKey ? "text" : "password"}
            placeholder="gsk_..."
            value={localApiKey}
            onChange={(e) => setLocalApiKey(e.target.value)}
            onBlur={() => onSaveApiKey(localApiKey)}
          />
          <button
            className="icon-button"
            onClick={() => setShowKey(!showKey)}
            title={showKey ? "Hide" : "Show"}
          >
            {showKey ? "👁️" : "🔒"}
          </button>
        </div>
        <button
          className="save-button"
          onClick={() => onSaveApiKey(localApiKey)}
        >
          <Save size={12} /> Save Key
        </button>
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
          Use Fn Key
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
              placeholder="CommandOrControl+Shift+R"
            />
          </>
        )}

        <label className="label">Trigger Mode</label>
        <div className="radio-group">
          <label className="radio-label">
            <input
              type="radio"
              value="hold"
              checked={triggerMode === "hold"}
              onChange={() => onSetTriggerMode("hold")}
            />
            Hold
          </label>
          <label className="radio-label">
            <input
              type="radio"
              value="toggle"
              checked={triggerMode === "toggle"}
              onChange={() => onSetTriggerMode("toggle")}
            />
            Toggle
          </label>
        </div>
      </div>

      {/* Output Injection */}
      <div className="sidebar-group">
        <h3>Output Injection</h3>
        
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={autoTypeEnabled}
            onChange={(e) => onToggleAutoType(e.target.checked)}
          />
          Auto-Type Result
        </label>

        {autoTypeEnabled && (
          <>
            <label className="label">
              Typing Speed: {typingSpeed}ms
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
                <Key size={12} /> Grant Accessibility
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default Sidebar;
