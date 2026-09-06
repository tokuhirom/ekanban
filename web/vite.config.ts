import { resolve } from "node:path";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri が開発サーバを決め打ちで見に行くので、ポートは固定します
// (`crates/app/tauri.conf.json` の `devUrl`)。空いていなければ黙って別の
// ポートに移らず、落ちてほしい。
//
// **結び先も決め打ちにします。** 既定の `localhost` は、名前解決の順番次第で
// `::1` になります。IPv6 の `localhost` を持つ機械（GitHub の runner がそう）
// では、`127.0.0.1` を見に行った側が誰にも会えません。Playwright の
// `webServer` がそれで 60 秒待って落ちました。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1420, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // webview は 1 つのバージョンしか相手にしないので、古いブラウザ向けの
    // 変換は要りません。ネットワークに出ない約束があるので、外部の読み込みも
    // 作りません (`docs/DESIGN.md`「画面の作り」)。
    target: "es2022",
    assetsInlineLimit: 0,
    // 窓ごとにエントリポイントを分けます。1 行を放り込むだけの
    // クイックキャプチャに、盤面と D&D の一式を読ませる理由がありません。
    rollupOptions: {
      input: {
        index: resolve(import.meta.dirname, "index.html"),
        capture: resolve(import.meta.dirname, "capture.html"),
      },
    },
  },
});
