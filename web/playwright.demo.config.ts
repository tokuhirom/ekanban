// ブラウザ版（ADR 0035）の e2e。
//
// **設定を分けてあります。** `playwright.config.ts` はハーネス（`crates/harness`）
// を相手にしていて、そちらの `webServer` は wasm を要りません。1 つにまとめると、
// wasm を組み立てていない状態で `make e2e` を打ったときに、デモ用のサーバが
// 上がらずに全部落ちます。
//
// 見に行くのは **`dist-demo` に出した本物の成果物**です（`vite preview`）。
// 開発サーバではなく、GitHub Pages に置くのと同じものを叩きます。

import { defineConfig, devices } from "@playwright/test";

const PORT = 4173;

export default defineConfig({
  testDir: "e2e-demo",
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "list" : "line",
  use: { baseURL: `http://127.0.0.1:${PORT}` },
  webServer: {
    command: `npx vite preview --config vite.demo.config.ts --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}/`,
    reuseExistingServer: true,
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe",
  },
  // 1 つの系統だけにしてあります。エンジンごとの差は `e2e/` が 2 系統で見て
  // いて、ここで確かめるのは配る形になったときの違いだからです。
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
