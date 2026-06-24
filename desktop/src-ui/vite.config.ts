import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  define: {
    // Expose VITE_API_URL so transport.ts can reach coral-server in web mode.
    // Set in .env: VITE_API_URL=http://localhost:8080
    __VITE_API_URL__: JSON.stringify(process.env.VITE_API_URL ?? ""),
  },
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 5173,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
