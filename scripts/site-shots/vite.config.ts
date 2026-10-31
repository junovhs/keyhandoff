import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const mock = fileURLToPath(new URL("./tauri-mock.ts", import.meta.url));

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react()],
  server: { port: 1430, strictPort: true, fs: { allow: ["../.."] } },
  // index.html renders one screen (?s=scene); board.html lays every screen out.
  build: {
    rollupOptions: {
      input: {
        index: fileURLToPath(new URL("./index.html", import.meta.url)),
        board: fileURLToPath(new URL("./board.html", import.meta.url)),
      },
    },
  },
  resolve: {
    alias: {
      "@tauri-apps/api/core": mock,
      "@tauri-apps/api/event": mock,
      "@tauri-apps/api/window": mock,
    },
  },
});
