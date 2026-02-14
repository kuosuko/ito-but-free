import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import "./RecordingOverlay.css";

type RecordingState = "idle" | "recording" | "processing";

const RecordingOverlay = () => {
  const [state, setState] = useState<RecordingState>("recording");

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    (async () => {
      unlisten = await listen<string>("recording_state", (event) => {
        setState(event.payload as RecordingState);
      });
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  const label = state === "recording" ? "Recording..." : "Transcribing...";

  return (
    <div className="overlay-container">
      <div className="overlay-pill">
        <div className={`overlay-dot ${state}`} />
        <span className="overlay-label">{label}</span>
      </div>
    </div>
  );
};

export default RecordingOverlay;
