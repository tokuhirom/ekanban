// Rust 側の `unsafe_code = "forbid"` に当たるもの。
//
// TypeScript には「安全でない操作」という 1 つの入口が無いので、型検査
// (`tsc --strict`) と、型を無かったことにする書き方をここで止めるのとで
// 代えます（`docs/TAURI-MIGRATION.md` §9）。

import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default [
  // `dist/` はビルドの出力、`src/ipc/types/` は ts-rs が Rust から書き出した
  // もの。どちらも手で書かないので、直せない指摘を出しても意味がない。
  { ignores: ["dist/**", "src/ipc/types/**"] },

  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  // 7.x は `configs["recommended-latest"]` を旧形式のまま出す。flat 用はこちら。
  reactHooks.configs.flat["recommended-latest"],

  {
    // 型を見るルールは、型検査の対象にしているものにだけ掛ける。
    // この設定ファイル自身は `tsconfig.json` の外にある。
    files: ["src/**/*.ts", "src/**/*.tsx", "vite.config.ts", "vitest.config.ts"],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // 型を無かったことにする書き方は通さない。境界の外から来るものは
      // `ts-rs` が書き出した型で受けている。
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unsafe-assignment": "error",
      "@typescript-eslint/no-unsafe-member-access": "error",
      "@typescript-eslint/no-unsafe-call": "error",
      "@typescript-eslint/no-unsafe-return": "error",
      // Promise を投げっぱなしにしない。失敗が黙って消えると、webview の
      // 不具合を追う手段がなくなる（§9）。捨てるなら `void` と書く。
      "@typescript-eslint/no-floating-promises": "error",
      // 数を文字列に混ぜるのは日常の書き方で、危なくない。
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
    },
  },

  { files: ["eslint.config.js"], ...tseslint.configs.disableTypeChecked },
];
