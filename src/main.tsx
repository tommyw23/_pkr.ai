import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AuthProvider } from './context/AuthContext';
import Overlay from "./components/Overlay";
import CalibrationOverlay from "./components/CalibrationOverlay";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Determine which component to render based on window label
async function renderApp() {
  const root = ReactDOM.createRoot(document.getElementById("root")!);

  try {
    const currentWindow = getCurrentWindow();
    const label = currentWindow.label;

    if (label === "capture-overlay") {
      root.render(
        <React.StrictMode>
          <Overlay />
        </React.StrictMode>
      );
    } else if (label === "calibration-overlay") {
      root.render(
        <React.StrictMode>
          <CalibrationOverlay />
        </React.StrictMode>
      );
    } else {
      // Main window
      root.render(
        <React.StrictMode>
          <AuthProvider>
            <App />
          </AuthProvider>
        </React.StrictMode>
      );
    }
  } catch (error) {
    // Fallback to main app if window detection fails
    console.error("Failed to detect window label:", error);
    root.render(
      <React.StrictMode>
        <AuthProvider>
          <App />
        </AuthProvider>
      </React.StrictMode>
    );
  }
}

renderApp();
