import { defineConfig, devices } from "@playwright/test";

// `docs/DESIGN.md`「ドラッグ＆ドロップ」の受け入れ条件を、2 つのエンジンで確かめます。
//
// **ここで動かすのは本物の webview ではありません。** Playwright が繋がるのは
// Chromium と WebKit であって、WebView2・WKWebView・WebKitGTK ではない
// （ADR 0021）。それでも動かすのは、**エンジンの系統**が 2 つしかないから
// です——WebView2 は Chromium、WKWebView と WebKitGTK は WebKit。系統ごとの
// 差はここで出ます。出ないのは各 platform 層の差（macOS の慣性スクロールや
// ゴムのような跳ね返り）で、そこは手での確認に残ります。
export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "list" : "line",
  use: { baseURL: "http://127.0.0.1:1420" },
  // 画面を出すのは Vite。コマンドを出すのは `ekanban-harness` で、そちらは
  // テストの中で盤面ごとに上げ下げします（`e2e/drag.spec.ts`）。
  webServer: {
    command: "npm run dev",
    // `vite.config.ts` が `127.0.0.1` に結んでいます。`localhost` と書くと、
    // 名前解決の順番次第で会えない機械があります。
    url: "http://127.0.0.1:1420",
    reuseExistingServer: true,
    // 冷えた runner では、Vite の初回が依存の下ごしらえで少しかかる。
    timeout: 120_000,
    // 立ち上がらなかったときに、理由が読めるように。黙って 60 秒待って
    // 落ちるのは、いちばん直しにくい失敗です。
    stdout: "pipe",
    stderr: "pipe",
  },
  projects: [
    // WebView2 (Windows) と同じ系統。
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    // WKWebView (macOS) と WebKitGTK (Linux) と同じ系統。
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
