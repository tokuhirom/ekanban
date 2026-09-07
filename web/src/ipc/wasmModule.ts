// 生成された wasm の口に、型を付けて受け直す。
//
// `wasm-bindgen` が書き出す `.d.ts` は、引数も答えも `any` です。**`any` の
// まま画面へ流さない**ために、ここで `unknown` にしてから外に出します
// （`docs/DESIGN.md`「境界を越える値」、ESLint の `no-unsafe-*`）。
//
// 生成物は `web/wasm/` に出ます。git には入りません——`make web-wasm` が
// `crates/web` から組み立てます。

import init, { invoke as rawInvoke, start as rawStart } from "../../wasm/ekanban_web.js";

let ready = false;

/// wasm を読み込み、`localStorage` にある盤面で起動する。
export async function startWasm(platform: string): Promise<unknown> {
  if (!ready) {
    await init();
    ready = true;
  }
  return rawStart(platform) as unknown;
}

/// コマンドを呼ぶ。**同期です**——SQLite はこのページの中で動いています。
export function invoke(command: string, args: unknown): unknown {
  return rawInvoke(command, args) as unknown;
}
