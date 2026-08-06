import path from "path"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  base: './',
  plugins: [react()],
  server: {
    port: 5173,
    watch: {
      // Exclude Rust build artifacts from Vite's file watcher
      // to prevent EBUSY errors on Windows during cargo tauri dev
      ignored: ['**/src-tauri/**'],
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
