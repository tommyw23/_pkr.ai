import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// Detect if we’re running under Tauri (env vars present when `tauri dev` runs)
const isTauri =
  !!process.env.TAURI_DEV_HOST ||
  !!process.env.TAURI_PLATFORM ||
  !!process.env.TAURI_ARCH;

const DEFAULT_WEB_PORT = Number(process.env.VITE_PORT) || 5173;

export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  clearScreen: false,
  server: isTauri
    ? {
        // Tauri mode: fixed ports, fail fast if taken
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
          ? {
              protocol: "ws",
              host,
              port: 1421,
            }
          : undefined,
        watch: { ignored: ["**/src-tauri/**"] },
      }
    : {
        // Browser mode: friendly ports, auto-fallback if busy
        port: DEFAULT_WEB_PORT,     // 5173 by default
        strictPort: false,          // <-- let Vite pick another free port
        host: true,
      },
}));
