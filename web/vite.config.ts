import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri が開発サーバを決め打ちで見に行くので、ポートは固定します
// (`crates/app/tauri.conf.json` の `devUrl`)。空いていなければ黙って別の
// ポートに移らず、落ちてほしい。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // webview は 1 つのバージョンしか相手にしないので、古いブラウザ向けの
    // 変換は要りません。ネットワークに出ない約束があるので、外部の読み込みも
    // 作りません (`docs/TAURI-MIGRATION.md` §9)。
    target: "es2022",
    assetsInlineLimit: 0,
  },
});
