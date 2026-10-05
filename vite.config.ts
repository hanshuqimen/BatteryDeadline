import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      ignored: [
        "**/target/**",
        "**/.tools/**",
        "**/.artifacts/**",
        "**/.git/**",
        "**/src-tauri/**",
        "**/crates/**",
      ],
    },
    proxy: { "/__simulation": "http://127.0.0.1:1421" },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: "es2022", sourcemap: false },
  test: { include: ["tests/**/*.test.ts"] },
});
