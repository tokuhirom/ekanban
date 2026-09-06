import { defineConfig, devices } from "@playwright/test";

// §6 の受け入れ条件を、2 つのエンジンで確かめます。
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
    url: "http://127.0.0.1:1420",
    reuseExistingServer: true,
    timeout: 60_000,
  },
  projects: [
    // WebView2 (Windows) と同じ系統。
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    // WKWebView (macOS) と WebKitGTK (Linux) と同じ系統。
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
