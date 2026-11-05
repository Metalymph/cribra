import { defineConfig } from "vite";

const appDir = process.env.CRIBRA_APP_DIR;
const outDir = process.env.CRIBRA_OUT_DIR;
if (!appDir || !outDir) {
  throw new Error("CRIBRA_APP_DIR and CRIBRA_OUT_DIR are required");
}

export default defineConfig({
  root: appDir,
  base: "/app/",
  optimizeDeps:
    process.env.CRIBRA_VITE_EXCLUDE === "1"
      ? { exclude: ["cribra-typescript"] }
      : undefined,
  build: {
    outDir,
    emptyOutDir: true,
    target: "es2022",
    sourcemap: false,
  },
  server: {
    host: "127.0.0.1",
    port: 0,
    strictPort: false,
  },
});
