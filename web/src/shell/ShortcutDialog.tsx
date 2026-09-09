// クイックキャプチャの割り当てを記録するダイアログ（`docs/DESIGN.md`「クイックキャプチャ」、ADR 0012、[ADR 0030]）。
//
// 押されたキーをそのまま割り当てにします。**組み合わせは Rust が組み立てます**
// （`shortcut.rs`）——受け付けられる修飾キーとキーの範囲は、登録する側にしか
// 分からないからです。ここが送るのは `KeyboardEvent` の中身だけです。
//
// **押されているキーをその場に映します**（[ADR 0030]）。映さないと、押したのに
// 何も起きないとき、キーが届いていないのか、届いて断られたのかが分かりません。
// 同じ理由で、**割り当てたあともダイアログを閉じません**——閉じてしまうと、
// 何が登録されたのかを読む間がありません。
//
// 開いている間は、メニューがキーを取らない状態にします
// （`set_menu_accelerators_active`）。そのままだと `Cmd+N` のような組み合わせを
// メニューが先に取り、`keydown` がここまで届きません。macOS で「まったく効かない」
// ように見えていた理由です。
//
// 登録できなかった理由は、閉じずにその場に出します。打ち直す先から離れた
// ところに理由を出さない、という [ADR 0016] の分け方のとおりです。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md
// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md

import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { useIpc, type Ipc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { KeyPress } from "./shortcut";
import { readKeyPress } from "./shortcut";
import type { Platform } from "../ipc/types/Platform";
import { isComposing } from "./ime";
import {
  describeHeld,
  describeStored,
  heldOnKeyDown,
  heldOnKeyUp,
  isHolding,
  MODIFIER_CODES,
  NOTHING_HELD,
  type Held,
} from "./pressed";

/// メニューの付け外しを 1 本の列にする。
///
/// **投げっぱなしにできません。** React の `StrictMode` は effect を「張る・
/// 畳む・張る」と 2 度走らせるので、3 回の呼び出しが同時に飛びます。届く順が
/// 入れ替わると、ダイアログが開いているのにメニューが割り当てを持ったままに
/// なり、直そうとしていた不具合がそのまま残ります。
let pending: Promise<unknown> = Promise.resolve();

function queueMenuAccelerators(ipc: Ipc, active: boolean): void {
  pending = pending.then(
    () => ipc.setMenuAcceleratorsActive(active),
    () => ipc.setMenuAcceleratorsActive(active),
  );
}

interface Props {
  /** いま保存されている割り当て。無ければ `null`。 */
  current: string | null;
  /** 使えない環境なら、その理由。 */
  unavailable: string | null;
  /** 保存されているのに登録できていない理由。効いているなら `null`。 */
  failure: string | null;
  /** どの OS か。修飾キーの見せ方が変わる（ADR 0009）。 */
  platform: Platform;
  onChanged: (shortcut: string | null) => void;
  onClose: () => void;
}

export function ShortcutDialog({
  current,
  unavailable,
  failure,
  platform,
  onChanged,
  onClose,
}: Props) {
  const ipc = useIpc();
  const [held, setHeld] = useState<Held>(NOTHING_HELD);
  const [rejected, setRejected] = useState<string | null>(null);
  /** この場で割り当てたもの。何が登録されたのかを読めるように残す。 */
  const [assigned, setAssigned] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  // 押されたキーを受けるので、開いた瞬間に焦点をここへ移す。`autoFocus` は
  // 入力欄にしか効かないので、自分で動かす（`Dialog.tsx` の `Shell` と同じ）。
  //
  // **`useEffect` では遅すぎます。** あれが走るのは描画のあとなので、ダイアログが
  // 出ていて焦点はまだ外、という 1 フレームが空きます。キーを受けるためだけに
  // 出す窓で、そこに打たれた 1 打が黙って消えます。`useLayoutEffect` なら
  // 描かれる前に移るので、その隙間ができません。
  useLayoutEffect(() => {
    box.current?.focus();
  }, []);

  // 開いている間だけ、メニューがキーを取らない状態にする。閉じたら必ず戻す。
  useEffect(() => {
    queueMenuAccelerators(ipc, false);
    return () => {
      queueMenuAccelerators(ipc, true);
    };
  }, [ipc]);

  // 窓ごとフォーカスを失ったら、押されているキーを空にする。
  //
  // **要素の `blur` だけでは足りません。** 割り当てたホットキーがその場で効いて
  // キャプチャの窓が前に出ると、`keyup` はそちらへ行き、この窓には届きません。
  // 数えたままにすると、離したキーが押しっぱなしに見えます。
  useEffect(() => {
    function forget() {
      setHeld(NOTHING_HELD);
    }
    window.addEventListener("blur", forget);
    return () => {
      window.removeEventListener("blur", forget);
    };
  }, []);

  /// 押されたキーを割り当てにする。`null` で解除。
  ///
  /// **文字列にするのはここ**です（`shell/shortcut.ts`、ADR 0039）。受け付け
  /// られない組み合わせはここで断り、理由をその場に出します——打ち直せば直る
  /// ものなので、ダイアログを閉じません（ADR 0016）。登録できるかどうかは
  /// 別の話で、そちらは殻が答えます。
  async function apply(press: KeyPress | null) {
    let shortcut: string | null = null;
    if (press !== null) {
      const read = readKeyPress(press);
      if (!read.ok) {
        setRejected(read.reason);
        return;
      }
      shortcut = read.shortcut;
    }
    try {
      const stored = await ipc.setQuickCaptureShortcut(shortcut);
      onChanged(stored);
      setAssigned(stored);
      setRejected(null);
    } catch (error: unknown) {
      setRejected(describeFailure(error).detail);
    }
  }

  const pressed = describeHeld(held, platform);
  // 割り当てが済んでいれば、その形。まだなら保存されているもの。
  const shown = assigned ?? current;

  return (
    <div className="dialog-backdrop">
      <div
        className="dialog shortcut-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="クイックキャプチャのショートカット"
        tabIndex={-1}
        ref={box}
        onBlur={() => {
          // 焦点が外れると `keyup` が届かない。押しっぱなしに見せない。
          setHeld(NOTHING_HELD);
        }}
        onKeyUp={(event) => {
          setHeld((previous) => heldOnKeyUp(previous, event.nativeEvent));
        }}
        onKeyDown={(event) => {
          if (isComposing(event.nativeEvent)) return;
          // 修飾キーなしの Escape は「やめる」。割り当てにはしない。
          if (event.key === "Escape" && !(event.ctrlKey || event.altKey || event.metaKey)) {
            event.stopPropagation();
            onClose();
            return;
          }
          // 盤面の割り当てに漏らさない。ダイアログの裏で Undo が走ると、
          // 割り当てを決めたつもりで盤面が巻き戻る。
          event.stopPropagation();
          event.preventDefault();
          setHeld(heldOnKeyDown(event.nativeEvent));
          if (MODIFIER_CODES.test(event.nativeEvent.code)) return;
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
          <p className="dialog-detail">割り当てたいキーの組み合わせを押してください。</p>
        ) : (
          <p className="dialog-detail">{unavailable}</p>
        )}
        <p className="pressed-keys" aria-live="polite" aria-label="いま押されているキー">
          {isHolding(held) ? (
            // 並びは押されているキーそのもので、入れ替わりも消えもしない。
            pressed.map((key, index) => <kbd key={index}>{key}</kbd>)
          ) : (
            <span className="pressed-keys-empty">キーが押されていません</span>
          )}
        </p>
        <p className="dialog-detail shortcut-current">
          {shown === null ? (
            "いまの割り当てはありません。"
          ) : (
            <>
              {assigned === null ? "いまの割り当て: " : "割り当てました: "}
              {describeStored(shown, platform).map((key, index) => (
                <kbd key={index}>{key}</kbd>
              ))}
              <span className="shortcut-stored">（{shown}）</span>
            </>
          )}
        </p>
        {/* 出すのは、保存されている割り当てをまだ触っていない間だけ。触れば
            その結果のほうが新しく、解除すれば効かない割り当てそのものが無い。 */}
        {failure !== null && assigned === null && current !== null && (
          <p className="field-error" role="alert">
            この割り当ては、いま登録できていません: {failure}
          </p>
        )}
        {rejected !== null && (
          <p className="field-error" role="alert">
            {rejected}
          </p>
        )}
        <div className="dialog-buttons">
          <button
            type="button"
            className="secondary clear-shortcut"
            disabled={shown === null}
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
