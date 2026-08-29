import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root,
  plugins: [react({ jsxImportSource: "@diskvista/i18n" })],
  resolve: {
    alias: {
      "@diskvista/i18n": fileURLToPath(new URL("./src/i18n", import.meta.url)),
    },
  },
  optimizeDeps: {
    exclude: ["@diskvista/i18n/jsx-runtime", "@diskvista/i18n/jsx-dev-runtime"],
  },
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: { target: "es2021", sourcemap: false },
  clearScreen: false,
});
