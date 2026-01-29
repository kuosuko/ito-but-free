import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

// Sub-components
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import Workspace from "./components/Workspace";
import LogsPanel from "./components/LogsPanel";

function App() {
  // Core states
  const [isRecording, setIsRecording] = useState<boolean>(false);
  const [transcription, setTranscription] = useState<string>("");
  const [logs, setLogs] = useState<any[]>([]);

  // Settings states
  const [apiKey, setApiKey] = useState<string>("");
  const [hotkey, setHotkey] = useState<string>("CommandOrControl+Shift+R");
  const [triggerMode, setTriggerMode] = useState<string>("hold");
  const [autoTypeEnabled, setAutoTypeEnabled] = useState<boolean>(true);
  const [typingSpeed, setTypingSpeed] = useState<number>(0);
  const [fnKeyEnabled, setFnKeyEnabled] = useState<boolean>(false);
  const [hasAccessibilityPermission, setHasAccessibilityPermission] = useState<boolean>(false);

  // Load initial state and setup listeners
  useEffect(() => {
    let unlistenLog: undefined | (() => void);
    let unlistenTx: undefined | (() => void);

    (async () => {
      // Setup event listeners
      unlistenLog = await listen("log", (event) => {
        setLogs((prev) => [event.payload, ...prev].slice(0, 500));
      });

      unlistenTx = await listen("transcription", async (event) => {
        const text = event.payload as string;
        setTranscription(text);
        try {
          await invoke("write_clipboard", { text });
        } catch {}
      });

      // Load all settings
      try {
        const status = await invoke<boolean>("recording_status");
        setIsRecording(status);
        
        const key = await invoke<string | null>("get_groq_api_key");
        setApiKey(key ?? "");
        
        const hk = await invoke<string>("get_hotkey");
        setHotkey(hk);
        
        const mode = await invoke<string>("get_trigger_mode");
        setTriggerMode(mode);
        
        const autoType = await invoke<boolean>("get_auto_type_enabled");
        setAutoTypeEnabled(autoType);
        
        const speed = await invoke<number>("get_type_speed_ms");
        setTypingSpeed(speed);
        
        const fnEnabled = await invoke<boolean>("fn_key_listening_status");
        setFnKeyEnabled(fnEnabled);
        
        const hasAccess = await invoke<boolean>("check_accessibility_permission");
        setHasAccessibilityPermission(hasAccess);
      } catch (err) {
        console.error("Failed to load settings:", err);
      }
    })();

    return () => {
      unlistenLog?.();
      unlistenTx?.();
    };
  }, []);

  // Action handlers
  async function toggleRecording() {
    if (isRecording) {
      const text = await invoke<string>("stop_and_transcribe");
      setIsRecording(false);
      setTranscription(text);
    } else {
      await invoke("start_recording");
      setIsRecording(true);
    }
  }

  async function handleSaveApiKey(key: string) {
    try {
      await invoke("set_groq_api_key", { apiKey: key });
      setApiKey(key);
    } catch (err) {
      console.error("Failed to save API key:", err);
    }
  }

  async function handleSaveHotkey(hk: string) {
    try {
      await invoke("set_hotkey", { hotkey: hk });
      setHotkey(hk);
    } catch (err) {
      console.error("Failed to save hotkey:", err);
    }
  }

  async function handleSetTriggerMode(mode: string) {
    try {
      await invoke("set_trigger_mode", { mode });
      setTriggerMode(mode);
    } catch (err) {
      console.error("Failed to set trigger mode:", err);
    }
  }

  async function handleToggleAutoType(enabled: boolean) {
    try {
      await invoke("set_auto_type_enabled", { enabled });
      setAutoTypeEnabled(enabled);
    } catch (err) {
      console.error("Failed to toggle auto-type:", err);
    }
  }

  async function handleSetTypingSpeed(speed: number) {
    try {
      await invoke("set_type_speed_ms", { ms: speed });
      setTypingSpeed(speed);
    } catch (err) {
      console.error("Failed to set typing speed:", err);
    }
  }

  async function handleToggleFnKey(enabled: boolean) {
    try {
      if (enabled) {
        await invoke("enable_fn_key_listening");
      } else {
        await invoke("disable_fn_key_listening");
      }
      setFnKeyEnabled(enabled);
    } catch (err) {
      console.error("Failed to toggle Fn key:", err);
    }
  }

  async function handleRequestAccessibilityPermission() {
    try {
      await invoke("request_accessibility_permission");
      // Recheck after a short delay
      setTimeout(async () => {
        const hasAccess = await invoke<boolean>("check_accessibility_permission");
        setHasAccessibilityPermission(hasAccess);
      }, 500);
    } catch (err) {
      console.error("Failed to request permission:", err);
    }
  }

  return (
    <div className="app-container">
      <TitleBar appName="GroqBara" version="0.1.0" />

      <div className="content">
        <Sidebar
          apiKey={apiKey}
          onSaveApiKey={handleSaveApiKey}
          hotkey={hotkey}
          onSaveHotkey={handleSaveHotkey}
          triggerMode={triggerMode}
          onSetTriggerMode={handleSetTriggerMode}
          autoTypeEnabled={autoTypeEnabled}
          onToggleAutoType={handleToggleAutoType}
          typingSpeed={typingSpeed}
          onSetTypingSpeed={handleSetTypingSpeed}
          fnKeyEnabled={fnKeyEnabled}
          onToggleFnKey={handleToggleFnKey}
          hasAccessibilityPermission={hasAccessibilityPermission}
          onRequestPermission={handleRequestAccessibilityPermission}
        />

        <div className="center-panel">
          <Workspace
            isRecording={isRecording}
            onToggleRecording={toggleRecording}
            transcription={transcription}
            onSetTranscription={setTranscription}
          />

          <LogsPanel logs={logs} onClearLogs={() => setLogs([])} />
        </div>
      </div>
    </div>
  );
}

export default App;
