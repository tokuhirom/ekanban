// ページが描くメニューバー（[ADR 0035]）。
//
// **配るアプリでは、これは使いません。** そちらのメニューを描くのは OS です
// （[ADR 0015]）。ブラウザには OS のメニューバーが無いので、同じ構成をページが
// 描きます。
//
// **構成は `shell/menu.ts` の 1 つだけ**です（[ADR 0043]）。ここは受け取った
// 並びを描き、押されたことを配るだけ——項目をここで足したら、殻のメニューに
// 無いものがブラウザ版にだけ出ます。
//
// [ADR 0015]: ../../docs/adr/0015-a-menu-bar-on-every-platform.md
// [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md
// [ADR 0043]: ../../docs/adr/0043-the-menu-is-described-by-the-webview.md

import { useEffect, useRef, useState } from "react";

import type { AppAction } from "../ipc/types/AppAction";
import type { Platform } from "../ipc/types/Platform";
import { formatAccelerator, matchesAccelerator } from "./accelerator";
import type { Item, Section } from "./menu";

interface Props {
  sections: Section[];
  platform: Platform;
  onAction: (action: AppAction) => void;
  /** メニューバーの右端に置く一言。ここがどこなのかを伝えるのに使います。 */
  note?: React.ReactNode;
}

export function MenuBar({ sections, platform, onAction, note }: Props): React.JSX.Element {
  const [open, setOpen] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);

  // 開いている間だけ、外を押したときと Escape で閉じる。
  useEffect(() => {
    if (open === null) return;
    const onPointerDown = (event: PointerEvent): void => {
      if (!(event.target instanceof Node)) return;
      if (bar.current?.contains(event.target) === true) return;
      setOpen(null);
    };
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") setOpen(null);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="menu-bar" ref={bar} role="menubar">
      {sections.map((section) => (
        <div className="menu-bar-section" key={section.name}>
          <button
            type="button"
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === section.name}
            className="menu-bar-title"
            onClick={() => {
              setOpen(open === section.name ? null : section.name);
            }}
            // 1 つ開いてから横に移るのは、メニューバーの普通の動き。
            onPointerEnter={() => {
              if (open !== null) setOpen(section.name);
            }}
          >
            {section.name}
          </button>
          {open === section.name && (
            <div className="menu-bar-list" role="menu" aria-label={section.name}>
              {section.items.map((item, index) => (
                <MenuBarItem
                  // 区切り線には id が無い。並びは Rust が決めていて動かない。
                  key={itemKey(item, index)}
                  item={item}
                  platform={platform}
                  onPick={(action) => {
                    setOpen(null);
                    onAction(action);
                  }}
                />
              ))}
            </div>
          )}
        </div>
      ))}
      {note !== undefined && <span className="menu-bar-note">{note}</span>}
    </div>
  );
}

function itemKey(item: Item, index: number): string {
  return item.kind === "app" ? item.action : `separator-${String(index)}`;
}

interface ItemProps {
  item: Item;
  platform: Platform;
  onPick: (action: AppAction) => void;
}

function MenuBarItem({ item, platform, onPick }: ItemProps): React.JSX.Element {
  // ページが描くのは `webSections` が通した項目だけ（`shell/menu.ts`）。
  // OS の項目とウィンドウの操作は、そこで落ちています。
  if (item.kind !== "app") return <div className="menu-bar-separator" role="separator" />;
  return (
    <button
      type="button"
      role="menuitem"
      className="menu-bar-item"
      disabled={!item.enabled}
      onClick={() => {
        onPick(item.action);
      }}
    >
      <span>{item.label}</span>
      {item.accelerator !== null && (
        <span className="menu-bar-accelerator">
          {formatAccelerator(item.accelerator, platform)}
        </span>
      )}
    </button>
  );
}

/// メニューに付いている割り当てを、ページで受ける。
///
/// 配るアプリではこれを OS のメニューバーがやります（`docs/DESIGN.md`
/// 「メニューとキー割り当て」）。**ブラウザが自分で取ってしまう組み合わせは
/// 届きません**——`Ctrl+N`・`Ctrl+T`・`Ctrl+W` あたりがそうで、止める手立ては
/// ありません。その項目はメニューから押します。
export function useMenuAccelerators(
  sections: Section[],
  platform: Platform,
  onAction: (action: AppAction) => void,
): void {
  // 押されるたびに購読を張り直さないよう、いちばん新しいものを持ち回る。
  const latest = useRef({ sections, platform, onAction });
  useEffect(() => {
    latest.current = { sections, platform, onAction };
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const { sections: current, platform: os, onAction: act } = latest.current;
      for (const section of current) {
        for (const item of section.items) {
          if (item.kind !== "app" || !item.enabled || item.accelerator === null) continue;
          if (!matchesAccelerator(event, item.accelerator, os)) continue;
          // ブラウザの既定（`Cmd+S` の保存、`Cmd+F` の検索）を止める。
          event.preventDefault();
          act(item.action);
          return;
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);
}
