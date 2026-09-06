// ダイアログ。
//
// 出すのは、使う人が手を打たないと直らない失敗と、Undo で戻せない・1 操作で
// 複数件が消える確認だけです（[ADR 0016]、`docs/DESIGN.md`）。すべての削除に
// 確認を出すと、読まずに押す確認になって役に立ちません。
//
// ネイティブでは `window.open_alert_dialog` でしたが、webview の `alert()` と
// `confirm()` は使いません——見た目がテーマから外れるうえ、WebKitGTK では
// webview 全体を止めるので、その間に届いたイベントが溜まります。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import { useEffect, useId, useRef, type ReactNode } from "react";

interface ShellProps {
  title: string;
  onCancel: () => void;
  children: ReactNode;
}

/// 共通の枠。`Escape` で閉じ、開いた瞬間に中の最初のコントロールへ焦点を移す。
function Shell({ title, onCancel, children }: ShellProps) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 焦点を中へ移す。外に残ったままだと、盤面の割り当てがダイアログの裏で
    // 効いてしまう（`boardShortcutsDisabled` は入力欄しか見ない）。
    const focusable = box.current?.querySelector<HTMLElement>("input, button");
    focusable?.focus();
  }, []);

  return (
    <div
      className="dialog-backdrop"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onCancel();
        }
      }}
    >
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} ref={box}>
        <h2 className="dialog-title" id={titleId}>
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}

interface AlertProps {
  title: string;
  detail: string;
  onDismiss: () => void;
}

/// 失敗の知らせ。読んで閉じるだけなので、ボタンは 1 つ。
export function AlertDialog({ title, detail, onDismiss }: AlertProps) {
  return (
    <Shell title={title} onCancel={onDismiss}>
      <p className="dialog-detail">{detail}</p>
      <div className="dialog-buttons">
        <button type="button" className="primary" onClick={onDismiss}>
          OK
        </button>
      </div>
    </Shell>
  );
}

interface ConfirmProps {
  title: string;
  description: string;
  /** 確定ボタンの文言。「削除」「アーカイブ」のように、何が起きるかを書く。 */
  okText: string;
  onOk: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({ title, description, okText, onOk, onCancel }: ConfirmProps) {
  return (
    <Shell title={title} onCancel={onCancel}>
      <p className="dialog-detail">{description}</p>
      <div className="dialog-buttons">
        <button type="button" className="secondary" onClick={onCancel}>
          キャンセル
        </button>
        {/* `danger`（赤）を使ってよいのは、ダイアログの確定ボタンと
            メニューの削除項目だけ（`docs/DESIGN.md`）。 */}
        <button type="button" className="danger" onClick={onOk}>
          {okText}
        </button>
      </div>
    </Shell>
  );
}

interface PromptProps {
  title: string;
  label: string;
  placeholder: string;
  value: string;
  /** 入力欄の脇に出す理由。Rust が `Validation` で返したもの。 */
  error: string | null;
  okText: string;
  onChange: (value: string) => void;
  onOk: () => void;
  onCancel: () => void;
}

/// 1 行を打たせるダイアログ。ボードの名前がこれです（#91）。
///
/// **1 行の入力欄なので `Enter` で確定します**（`docs/DESIGN.md`）。打ち終わりに
/// 確定ボタンまで手を伸ばさせません。
export function PromptDialog({
  title,
  label,
  placeholder,
  value,
  error,
  okText,
  onChange,
  onOk,
  onCancel,
}: PromptProps) {
  const inputId = useId();
  return (
    <Shell title={title} onCancel={onCancel}>
      <label className="field-label" htmlFor={inputId}>
        {label}
      </label>
      <input
        id={inputId}
        className="dialog-input"
        value={value}
        placeholder={placeholder}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || value.trim() === "") return;
          event.preventDefault();
          onOk();
        }}
      />
      {error !== null && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      <div className="dialog-buttons">
        <button type="button" className="secondary" onClick={onCancel}>
          キャンセル
        </button>
        <button type="button" className="primary" disabled={value.trim() === ""} onClick={onOk}>
          {okText}
        </button>
      </div>
    </Shell>
  );
}
