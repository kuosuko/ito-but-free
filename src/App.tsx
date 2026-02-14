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
  // Core states: "idle" | "recording" | "processing"
  type RecordingState = "idle" | "recording" | "processing";
  const [recordingState, setRecordingState] = useState<RecordingState>("idle");
  const [transcription, setTranscription] = useState<string>("");
  const [logs, setLogs] = useState<{ level: string; message: string; timestamp: string }[]>([]);

  // Settings states
  const [apiKey, setApiKey] = useState<string>("");
  const [hotkey, setHotkey] = useState<string>("CommandOrControl+Shift+R");
  const [triggerMode, setTriggerMode] = useState<string>("hold");
  const [autoTypeEnabled, setAutoTypeEnabled] = useState<boolean>(true);
  const [typingSpeed, setTypingSpeed] = useState<number>(0);
  const [fnKeyEnabled, setFnKeyEnabled] = useState<boolean>(false);
  const [hasAccessibilityPermission, setHasAccessibilityPermission] = useState<boolean>(false);
  const [refineOutputEnabled, setRefineOutputEnabled] = useState<boolean>(false);
  const [refinementPrompt, setRefinementPrompt] = useState<string>("");
  const [refinementModel, setRefinementModel] = useState<string>("qwen/qwen3-32b");

  // Load initial state and setup listeners
  useEffect(() => {
    let unlistenLog: undefined | (() => void);
    let unlistenTx: undefined | (() => void);
    let unlistenState: undefined | (() => void);

    (async () => {
      // Setup event listeners
      unlistenLog = await listen<{ level: string; message: string }>("log", (event) => {
        const entry = {
          ...event.payload,
          timestamp: new Date().toLocaleTimeString(),
        };
        setLogs((prev) => [entry, ...prev].slice(0, 500));
      });

      unlistenTx = await listen("transcription", async (event) => {
        const text = event.payload as string;
        setTranscription(text);
        try {
          await invoke("write_clipboard", { text });
        } catch {}
      });

      unlistenState = await listen<string>("recording_state", (event) => {
        const state = event.payload as RecordingState;
        setRecordingState(state);
      });

      // Load all settings
      try {
        const status = await invoke<boolean>("recording_status");
        if (status) setRecordingState("recording");
        
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
        
        const refineEnabled = await invoke<boolean>("get_refine_output_enabled");
        setRefineOutputEnabled(refineEnabled);
        
        const refinePrompt = await invoke<string>("get_refinement_prompt");
        setRefinementPrompt(refinePrompt);
        
        const refineModel = await invoke<string>("get_refinement_model");
        setRefinementModel(refineModel);
      } catch (err) {
        console.error("Failed to load settings:", err);
      }
    })();

    return () => {
      unlistenLog?.();
      unlistenTx?.();
      unlistenState?.();
    };
  }, []);

  // Action handlers
  async function toggleRecording() {
    if (recordingState === "recording") {
      setRecordingState("processing");
      try {
        const text = await invoke<string>("stop_and_transcribe");
        setTranscription(text);
      } catch (err) {
        console.error("Transcription failed:", err);
      }
      setRecordingState("idle");
    } else if (recordingState === "idle") {
      await invoke("start_recording");
      setRecordingState("recording");
    }
    // If "processing", ignore clicks
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

  async function handleToggleRefineOutput(enabled: boolean) {
    try {
      await invoke("set_refine_output_enabled", { enabled });
      setRefineOutputEnabled(enabled);
    } catch (err) {
      console.error("Failed to toggle refine output:", err);
    }
  }

  async function handleSetRefinementPrompt(prompt: string) {
    try {
      await invoke("set_refinement_prompt", { prompt });
      setRefinementPrompt(prompt);
    } catch (err) {
      console.error("Failed to set refinement prompt:", err);
    }
  }

  async function handleSetRefinementModel(model: string) {
    try {
      await invoke("set_refinement_model", { model });
      setRefinementModel(model);
    } catch (err) {
      console.error("Failed to set refinement model:", err);
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
          refineOutputEnabled={refineOutputEnabled}
          onToggleRefineOutput={handleToggleRefineOutput}
          refinementPrompt={refinementPrompt}
          onSetRefinementPrompt={handleSetRefinementPrompt}
          refinementModel={refinementModel}
          onSetRefinementModel={handleSetRefinementModel}
        />

        <div className="center-panel">
          <Workspace
            recordingState={recordingState}
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
