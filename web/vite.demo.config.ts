// ブラウザ版の組み立て（ADR 0035、ADR 0042）。
//
// 配るアプリのほうは `vite.config.ts` です。分けてあるのは、**入口が違う**
// からだけです——盤面のコードは 1 文字も違わず、差し替わるのは置き場所
// （`localStorage`）と環境の口だけです。
//
// `base` を相対にしてあるのは、GitHub Pages が `/<リポジトリ名>/` の下に
// 置くためです。絶対パスにすると、その 1 階層ぶんだけ読み込みが外れます。

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/// 動いているアプリの版。出どころは根の `Cargo.toml`（`vite.config.ts` と同じ）。
function appVersion(): string {
  const manifest = readFileSync(resolve(import.meta.dirname, "..", "Cargo.toml"), "utf8");
  const found = /^version = "([^"]+)"/m.exec(manifest);
  if (found?.[1] === undefined) throw new Error("Cargo.toml に version の行がありません");
  return found[1];
}

export default defineConfig({
  root: "demo",
  base: "./",
  plugins: [react()],
  clearScreen: false,
  define: { __EKANBAN_VERSION__: JSON.stringify(appVersion()) },
  build: {
    outDir: "../dist-demo",
    emptyOutDir: true,
    target: "es2022",
  },
});
