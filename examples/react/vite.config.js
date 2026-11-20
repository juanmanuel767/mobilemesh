import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// MobileMesh usa memoria compartida: la página necesita estas dos cabeceras.
export default defineConfig({
  plugins: [react()],
  server: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  optimizeDeps: { exclude: ["mobilemesh"] },
});
