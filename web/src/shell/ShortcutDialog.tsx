// クイックキャプチャの割り当てを記録するダイアログ（`docs/DESIGN.md`「クイックキャプチャ」、ADR 0012）。
//
// 押されたキーをそのまま割り当てにします。**組み合わせは Rust が組み立てます**
// （`shortcut.rs`）——受け付けられる修飾キーとキーの範囲は、登録する側にしか
// 分からないからです。ここが送るのは `KeyboardEvent` の中身だけです。
//
// 登録できなかった理由は、閉じずにその場に出します。打ち直す先から離れた
// ところに理由を出さない、という [ADR 0016] の分け方のとおりです。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import { useEffect, useRef, useState } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { KeyPress } from "../ipc/types/KeyPress";

interface Props {
  /** いま保存されている割り当て。無ければ `null`。 */
  current: string | null;
  /** 使えない環境なら、その理由。 */
  unavailable: string | null;
  onChanged: (shortcut: string | null) => void;
  onClose: () => void;
}

/// 修飾キーだけを押している途中は、まだ組み合わせが決まっていない。
const MODIFIER_CODES = /^(Control|Alt|Shift|Meta)(Left|Right)$/;

export function ShortcutDialog({ current, unavailable, onChanged, onClose }: Props) {
  const ipc = useIpc();
  const [failure, setFailure] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  // 押されたキーを受けるので、開いた瞬間に焦点をここへ移す。`autoFocus` は
  // 入力欄にしか効かないので、自分で動かす（`Dialog.tsx` の `Shell` と同じ）。
  useEffect(() => {
    box.current?.focus();
  }, []);

  async function apply(press: KeyPress | null) {
    try {
      const stored = await ipc.setQuickCaptureShortcut(press);
      onChanged(stored);
      onClose();
    } catch (error: unknown) {
      setFailure(describeFailure(error).detail);
    }
  }

  return (
    <div className="dialog-backdrop">
      <div
        className="dialog shortcut-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="クイックキャプチャのショートカット"
        tabIndex={-1}
        ref={box}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return;
          // 修飾キーなしの Escape は「やめる」。割り当てにはしない。
          if (event.key === "Escape" && !(event.ctrlKey || event.altKey || event.metaKey)) {
            event.stopPropagation();
            onClose();
            return;
          }
          if (MODIFIER_CODES.test(event.nativeEvent.code)) return;
          event.preventDefault();
          if (unavailable !== null) return;
          void apply({
            ctrl: event.ctrlKey,
            alt: event.altKey,
            shift: event.shiftKey,
            meta: event.metaKey,
            // 押された物理キー。`key` は修飾キーと配列で変わる。
            code: event.nativeEvent.code,
          });
        }}
      >
        <h2 className="dialog-title">クイックキャプチャのショートカット</h2>
        {unavailable === null ? (
          <p className="dialog-detail">
            割り当てたいキーの組み合わせを押してください。いまの割り当ては
            {current === null ? "ありません" : `「${current}」です`}。
          </p>
        ) : (
          <p className="dialog-detail">{unavailable}</p>
        )}
        {failure !== null && (
          <p className="field-error" role="alert">
            {failure}
          </p>
        )}
        <div className="dialog-buttons">
          <button
            type="button"
            className="secondary clear-shortcut"
            disabled={current === null}
            onClick={() => void apply(null)}
          >
            解除
          </button>
          <button type="button" className="primary" onClick={onClose}>
            閉じる
          </button>
        </div>
      </div>
    </div>
  );
}
