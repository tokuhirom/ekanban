// 生成された wasm の口を、型を付けて受け直す。
//
// 口の形は `ekanban_web.d.ts` にあります。**生成物は git に入らない**ので
// （`web/wasm/` は `.gitignore`、`make web-demo` が組み立てます）、型検査が
// wasm を組まない場でも通るように、宣言だけを別に置いてあります。
//
// 通す値は `unknown` です。`ts-rs` が書き出した型で受け直すのは呼ぶ側
// （`wasm.ts`）で、ここは運ぶだけにします（`docs/DESIGN.md`「境界を越える値」）。

import init, { invoke as rawInvoke, start as rawStart } from "../../wasm/ekanban_web.js";

let ready = false;

/// wasm を読み込み、`localStorage` にある盤面で起動する。
export async function startWasm(platform: string): Promise<unknown> {
  if (!ready) {
    await init();
    ready = true;
  }
  return rawStart(platform);
}

/// コマンドを呼ぶ。**同期です**——SQLite はこのページの中で動いています。
export function invoke(command: string, args: unknown): unknown {
  return rawInvoke(command, args);
}
