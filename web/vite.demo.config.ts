// ブラウザ版の組み立て（ADR 0035）。
//
// 配るアプリのほうは `vite.config.ts` です。分けてあるのは、**wasm の一式
// （2 MB ほど）をアプリの実行ファイルに入れないため**で、入口もエントリも
// 別のものになります。
//
// `base` を相対にしてあるのは、GitHub Pages が `/<リポジトリ名>/` の下に
// 置くためです。絶対パスにすると、その 1 階層ぶんだけ読み込みが外れます。

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  root: "demo",
  base: "./",
  plugins: [react()],
  clearScreen: false,
  build: {
    outDir: "../dist-demo",
    emptyOutDir: true,
    target: "es2022",
    // wasm は Vite に資産として運ばせます。data URL に畳まれると、読み込みが
    // 始まるまでに 2 MB のパースが挟まります。
    assetsInlineLimit: 0,
  },
});
