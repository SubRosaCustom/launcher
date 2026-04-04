import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

declare const process: {
  cwd(): string;
  env: Record<string, string | undefined>;
};

type EnvRequirement = {
  purpose: string;
  setIn: string;
};

const requiredFrontendEnv: Record<string, EnvRequirement> = {};

const currentWorkingDir = process.cwd();
const projectRootDir = currentWorkingDir.replace(/[\\/]src-tauri$/, "");

function validateRequiredFrontendEnv(env: Record<string, string | undefined>) {
  const missing = Object.entries(requiredFrontendEnv).filter(
    ([name]) => !env[name]?.trim(),
  );

  if (missing.length === 0) {
    return;
  }

  const details = missing
    .map(
      ([name, requirement]) =>
        `- ${name}: set this in ${requirement.setIn}. Purpose: ${requirement.purpose}`,
    )
    .join("\n");

  throw new Error(
    `SRCLauncher is missing required Vite environment variable${missing.length === 1 ? "" : "s"}:\n${details}`,
  );
}

// https://vite.dev/config/
export default defineConfig(async ({ mode }) => {
  const env = loadEnv(mode, projectRootDir, "");
  const host = env.TAURI_DEV_HOST?.trim() || process.env.TAURI_DEV_HOST;

  validateRequiredFrontendEnv(env);

  return {
    root: projectRootDir,
    envDir: projectRootDir,
    plugins: [react()],

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
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
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
