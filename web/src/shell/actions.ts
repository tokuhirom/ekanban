// メニューが押されたことを、画面の中の持ち主に配る（`docs/TAURI-MIGRATION.md` §7）。
//
// メニューは OS が描き、押されると Rust が `app:action` を投げます。**受け取った
// あとに何をするかを決めるのは画面**です——「保存」は開いているパネルの下書きを
// 保存することで、その下書きを持っているのはパネルだからです。Rust に「いま何が
// 開いているか」を持たせると、同じ判断が 2 か所になります。
//
// 配り方は、部品ごとに自分のぶんを取る形にしてあります。`Board` から props で
// 配ると、`saveEdit` を渡すためだけに `CardPanel` へ合図の数値を流す、といった
// 配線が増えます。

import { useEffect, useRef } from "react";

import { useIpc } from "../ipc";
import type { AppAction } from "../ipc/types/AppAction";

type Handler = () => void;
type Listener = (action: AppAction) => void;

const listeners = new Set<Listener>();

/// メニューの操作を、いま聞いている部品に配る。
export function dispatchAppAction(action: AppAction): void {
  // 走っている最中に部品が畳まれても崩れないよう、写しを回す。
  for (const listener of [...listeners]) listener(action);
}

/// この部品が引き受ける操作を登録する。
///
/// 渡した表は毎回作り直してかまいません。購読するのは 1 回だけで、呼ぶときに
/// いちばん新しい表を見ます。
export function useAppActions(handlers: Partial<Record<AppAction, Handler>>): void {
  const latest = useRef(handlers);
  useEffect(() => {
    latest.current = handlers;
  });
  useEffect(() => {
    const listener: Listener = (action) => {
      latest.current[action]?.();
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);
}

/// Rust からの `app:action` を受けて配りはじめる。画面全体で 1 回だけ呼ぶ。
export function useAppActionSource(): void {
  const ipc = useIpc();
  useEffect(() => ipc.onAppAction(dispatchAppAction), [ipc]);
}
