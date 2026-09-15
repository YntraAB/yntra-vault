import path from "path"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  base: './',
  plugins: [react()],
  server: {
    port: 5173,
    headers: {
      'Cache-Control': 'no-store',
    },
    watch: {
      // Exclude Rust build artifacts and database files from Vite's file watcher
      // to prevent EBUSY locks on Windows during cargo tauri dev and atomic saves
      ignored: ['**/src-tauri/**', '**/target/**', '**/*.vdb*', '**/crates/**'],
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
