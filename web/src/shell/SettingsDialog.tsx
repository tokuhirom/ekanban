// アプリ全体の設定（[ADR 0047]、`docs/DESIGN.md`「画面の作り」）。
//
// **ここに置くのは、開いているボードに紐づかないものだけ**です。ボードのもの
// （タグの整理、繰り返しの定義）は右のパネルで、そちらは「いま開いている
// ボードを編む場所」です。混ぜると、どのボードの設定なのかが読めなくなります。
//
// 中身は 4 つ——テーマ、日付の切り替わり、クイックキャプチャの割り当て、その入れ先。
//
// **保存ボタンはありません。** 触った時点で確定します（カードの欄ごとの確定と
// 同じ流儀、[ADR 0032]）。
//
// 割り当ての捕捉は、もとは専用のダイアログでした（`ShortcutDialog`）。同じ
// 手当てをここへ持ち込んでいます——押されているキーをその場に出し、割り当てた
// あとも閉じず、メニューがキーを取らない状態にする（[ADR 0030]）。
//
// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
// [ADR 0032]: ../../../docs/adr/0032-committing-a-card-field-by-field.md
// [ADR 0047]: ../../../docs/adr/0047-app-settings-live-in-a-settings-dialog.md

import { useEffect, useId, useState } from "react";

import { useIpc, type Ipc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { Platform } from "../ipc/types/Platform";
import type { ThemePreference } from "../ipc/types/ThemePreference";
import type { CaptureDestination } from "../model/capture";
import { Shell } from "./Dialog";
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
import type { KeyPress } from "./shortcut";
import { readKeyPress } from "./shortcut";

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

/// テーマの選択肢。並びは「表示」メニューと同じ。
const THEMES: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "ライトモード" },
  { value: "dark", label: "ダークモード" },
  { value: "system", label: "システムに合わせる" },
];

/// 日付の切り替わりに選べる時刻。分は持たないので 0〜23 の 24 択（ADR 0048）。
const BOUNDARY_HOURS = Array.from({ length: 24 }, (_, hour) => hour);

interface Props {
  /** 選ばれているテーマ。 */
  theme: ThemePreference;
  setTheme: (theme: ThemePreference) => void;
  /** 日付が変わる時刻（0〜23）。 */
  dayBoundaryHour: number;
  setDayBoundaryHour: (hour: number) => void;
  /** いま保存されている割り当て。無ければ `null`。 */
  shortcut: string | null;
  /** 割り当てを使えない環境なら、その理由。 */
  unavailable: string | null;
  /** 保存されているのに登録できていない理由。効いているなら `null`。 */
  failure: string | null;
  onShortcutChanged: (shortcut: string | null) => void;
  /** クイックキャプチャの入れ先。**出すだけ**で、選ぶのはカラムの `…`。 */
  captureTarget: CaptureDestination | null;
  /** どの OS か。修飾キーの見せ方が変わる（ADR 0009）。 */
  platform: Platform;
  onClose: () => void;
}

export function SettingsDialog({
  theme,
  setTheme,
  dayBoundaryHour,
  setDayBoundaryHour,
  shortcut,
  unavailable,
  failure,
  onShortcutChanged,
  captureTarget,
  platform,
  onClose,
}: Props) {
  const ipc = useIpc();
  const themeName = useId();
  const boundaryName = useId();

  // 開いているあいだ、メニューがキーを取らない状態にする。閉じたら必ず戻す。
  //
  // **捕捉の欄に焦点があるあいだだけ、にはしません**（ADR 0047）。焦点の移動は
  // 同期ですが `setMenuAcceleratorsActive` は往復なので、焦点が移った直後の
  // 1 打がメニューに取られる隙間ができます。ダイアログが開いているあいだは
  // アクセラレータの届く先がそもそも無いので、広く外して困りません。
  useEffect(() => {
    queueMenuAccelerators(ipc, false);
    return () => {
      queueMenuAccelerators(ipc, true);
    };
  }, [ipc]);

  return (
    <Shell title="設定" onCancel={onClose} className="settings-dialog">
      <div className="settings">
        <section className="settings-section">
          <h3 className="settings-heading">テーマ</h3>
          <div className="settings-choices" role="radiogroup" aria-label="テーマ">
            {THEMES.map((choice) => (
              <label className="settings-choice" key={choice.value}>
                <input
                  type="radio"
                  name={themeName}
                  value={choice.value}
                  checked={theme === choice.value}
                  onChange={() => {
                    setTheme(choice.value);
                  }}
                />
                {choice.label}
              </label>
            ))}
          </div>
          <p className="settings-note">「表示」メニューからも切り替えられます。</p>
        </section>

        {/* 日付の切り替わり（ADR 0048）。**基準日はこれ 1 つから作ります**
            ——カードの `⚠` もボード一覧の件数も、ここで選んだ時刻からその日が
            始まります。 */}
        <section className="settings-section">
          <h3 className="settings-heading" id={boundaryName}>
            日付の切り替わり
          </h3>
          {/* 名前は見出しから引きます。同じ言葉を 2 つ並べると、読み上げが
              「日付の切り替わり 日付の切り替わり」になります。 */}
          <select
            className="settings-select"
            aria-labelledby={boundaryName}
            value={dayBoundaryHour}
            onChange={(event) => {
              setDayBoundaryHour(Number(event.target.value));
            }}
          >
            {BOUNDARY_HOURS.map((hour) => (
              <option key={hour} value={hour}>
                {hour}:00
              </option>
            ))}
          </select>
          <p className="settings-note">
            この時刻から次の日が始まります。既定の 4:00 なら、午前 3 時はまだ前の日で、
            期限の「今日」もそのまま前の日を指します。
          </p>
        </section>

        <ShortcutSection
          current={shortcut}
          unavailable={unavailable}
          failure={failure}
          platform={platform}
          onChanged={onShortcutChanged}
        />

        <section className="settings-section">
          <h3 className="settings-heading">クイックキャプチャの入れ先</h3>
          {/* **選ばせません。** 「そのカラムを指す」操作なので、カラムの `…` に
              置いたままにします（ADR 0047）。ここは、いまどこなのかを読む場所。 */}
          <p className="settings-value">
            {captureTarget === null
              ? "入れ先のカラムがありません"
              : `${captureTarget.boardName} / ${captureTarget.columnName}`}
          </p>
          <p className="settings-note">
            入れ先は、カラムの「…」メニューの「クイックキャプチャ先にする」で選びます。
          </p>
        </section>
      </div>

      <div className="dialog-buttons">
        <button type="button" className="primary" onClick={onClose}>
          閉じる
        </button>
      </div>
    </Shell>
  );
}

interface ShortcutProps {
  current: string | null;
  unavailable: string | null;
  failure: string | null;
  platform: Platform;
  onChanged: (shortcut: string | null) => void;
}

/// クイックキャプチャの割り当てを捕まえる欄（[ADR 0030]）。
///
/// 押されたキーをそのまま割り当てにします。**組み合わせは Rust が組み立てます**
/// （`shortcut.rs`）——受け付けられる修飾キーとキーの範囲は、登録する側にしか
/// 分からないからです。ここが送るのは `KeyboardEvent` の中身だけです。
///
/// **押されているキーをその場に映します。** 映さないと、押したのに何も起きない
/// とき、キーが届いていないのか、届いて断られたのかが分かりません。割り当てた
/// あとも、登録された組み合わせをその場に残します。
///
/// キーを受けるのは**この欄に焦点があるあいだだけ**です。設定には触れる先が
/// ほかにもあるので、ダイアログ全体でキーを飲むわけにいきません。
///
/// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
function ShortcutSection({ current, unavailable, failure, platform, onChanged }: ShortcutProps) {
  const ipc = useIpc();
  const [held, setHeld] = useState<Held>(NOTHING_HELD);
  const [rejected, setRejected] = useState<string | null>(null);
  /** この場で割り当てたもの。何が登録されたのかを読めるように残す。 */
  const [assigned, setAssigned] = useState<string | null>(null);
  const [capturing, setCapturing] = useState(false);

  // 窓ごとフォーカスを失ったら、押されているキーを空にする。
  //
  // **要素の `blur` だけでは足りません。** 割り当てたホットキーがその場で効いて
  // キャプチャの窓が前に出ると、`keyup` はそちらへ行き、この窓には届きません。
  // 数えたままにすると、離したキーが押しっぱなしに見えます。
  useEffect(() => {
    function forget() {
      setHeld(NOTHING_HELD);
      setCapturing(false);
    }
    window.addEventListener("blur", forget);
    return () => {
      window.removeEventListener("blur", forget);
    };
  }, []);

  /// 押されたキーを割り当てにする。`null` で解除。
  ///
  /// **文字列にするのはここ**です（`shell/shortcut.ts`、ADR 0039）。受け付け
  /// られない組み合わせはここで断り、理由をその場に出します。登録できるか
  /// どうかは別の話で、そちらは殻が答えます。
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
  const disabled = unavailable !== null;

  return (
    <section className="settings-section">
      <h3 className="settings-heading">クイックキャプチャのショートカット</h3>
      <div
        className={`shortcut-capture${disabled ? " is-disabled" : ""}${
          capturing ? " is-capturing" : ""
        }`}
        role="group"
        aria-label="クイックキャプチャのショートカット"
        aria-disabled={disabled}
        // 使えない環境では焦点も当てません。押せない欄にキーを打たせません。
        tabIndex={disabled ? undefined : 0}
        onFocus={() => {
          setCapturing(true);
        }}
        onBlur={() => {
          // 焦点が外れると `keyup` が届かない。押しっぱなしに見せない。
          setHeld(NOTHING_HELD);
          setCapturing(false);
        }}
        onKeyUp={(event) => {
          if (disabled) return;
          setHeld((previous) => heldOnKeyUp(previous, event.nativeEvent));
        }}
        onKeyDown={(event) => {
          if (disabled) return;
          if (isComposing(event.nativeEvent)) return;
          // 修飾キーなしの Escape は「やめる」。割り当てにはせず、ダイアログを
          // 閉じる側（`Dialog.tsx` の `Shell`）へ渡します。
          if (event.key === "Escape" && !(event.ctrlKey || event.altKey || event.metaKey)) return;
          // Tab は焦点を送るキーとして残す。捕まえると、この欄から出られない。
          if (event.key === "Tab") return;
          // 盤面の割り当てに漏らさない。設定を触っているつもりで Undo が走ると、
          // 下の盤面が巻き戻る。
          event.stopPropagation();
          event.preventDefault();
          setHeld(heldOnKeyDown(event.nativeEvent));
          if (MODIFIER_CODES.test(event.nativeEvent.code)) return;
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
        <p className="settings-note">
          {disabled ? unavailable : "この欄を選んでから、割り当てたいキーの組み合わせを押してください。"}
        </p>
        {!disabled && (
          <p className="pressed-keys" aria-live="polite" aria-label="いま押されているキー">
            {isHolding(held) ? (
              // 並びは押されているキーそのもので、入れ替わりも消えもしない。
              pressed.map((key, index) => <kbd key={index}>{key}</kbd>)
            ) : (
              <span className="pressed-keys-empty">
                {capturing ? "キーが押されていません" : "キーを受け付けていません"}
              </span>
            )}
          </p>
        )}
        <p className="settings-value shortcut-current">
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
      </div>
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
      <button
        type="button"
        className="secondary clear-shortcut"
        disabled={disabled || shown === null}
        onClick={() => void apply(null)}
      >
        解除
      </button>
    </section>
  );
}
