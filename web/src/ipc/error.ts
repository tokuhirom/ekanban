// コマンドが投げた `AppError` を見分ける。
//
// Tauri の `invoke` もハーネスも、コマンドの `Err` をそのままの形で投げます。
// ネットワークが切れたときのように、そうでないものも来るので見分けます。
//
// 行き先の決め方は [ADR 0016] のままです——`Validation` は入力欄の脇、それ以外は
// ダイアログ。ここはその材料を取り出すだけで、決めるのは `state/board.ts` の
// `run()` です。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import type { AppError } from "./types/AppError";

export function asAppError(error: unknown): AppError | null {
  if (error === null || typeof error !== "object") return null;
  const candidate = error as Partial<AppError>;
  return typeof candidate.kind === "string" &&
    typeof candidate.title === "string" &&
    typeof candidate.detail === "string"
    ? (error as AppError)
    : null;
}

/// 投げられたものを、ダイアログに出す見出しと本文にする。
export function describeFailure(error: unknown): { title: string; detail: string } {
  const failure = asAppError(error);
  return failure === null
    ? { title: "操作できませんでした", detail: String(error) }
    : { title: failure.title, detail: failure.detail };
}
