// ブラウザ版の入口（[ADR 0042]）。
//
// 配るアプリの入口は `src/main.tsx` です。**盤面のコードは 1 文字も違いません**
// ——違うのは 3 つだけです。
//
// 1. 置き場所が `localStorage`（`src/store/`）。配るアプリでは SQLite です
// 2. メニューバーをページが描く（`src/shell/MenuBar.tsx`）。ブラウザに OS の
//    メニューバーが無いためで、構成は配るアプリと同じ `shell/menu.ts` です
// 3. どの OS かをページが名乗る。訊く相手の Rust がいないので、入口で 1 度
//    だけ見て配ります（`src/ipc/local.ts` の `detectPlatform`）
//
// [ADR 0042]: ../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Board } from "../src/board/Board";
import type { Platform } from "../src/ipc/types/Platform";
import { setIpc } from "../src/ipc";
import { fireAppAction } from "../src/ipc/browser";
import { detectPlatform, localIpc } from "../src/ipc/local";
import { MenuBar, useMenuAccelerators } from "../src/shell/MenuBar";
import type { Section } from "../src/shell/menu";
import { webSections } from "../src/shell/menu";
import { hardenBoard } from "../src/shell/harden";
import "../src/styles.css";
import "./demo.css";

function Demo({
  sections,
  platform,
}: {
  sections: Section[];
  platform: Platform;
}): React.JSX.Element {
  useMenuAccelerators(sections, platform, fireAppAction);
  return (
    <div className="demo">
      <MenuBar
        sections={sections}
        platform={platform}
        onAction={fireAppAction}
        note={
          <>
            ブラウザ版：盤面はこのブラウザの中だけに残ります。
            <a href="https://github.com/tokuhirom/ekanban#インストールと起動" rel="noreferrer">
              アプリ版
            </a>
          </>
        }
      />
      <Board />
    </div>
  );
}

function main(): void {
  const root = document.getElementById("root");
  if (root === null) throw new Error("#root がない");

  const platform = detectPlatform();
  const ipc = localIpc(platform);
  setIpc(ipc);
  hardenBoard();

  const sections = webSections(platform);

  // 未捕捉の例外は Rust 側と同じ経路に流す（`docs/DESIGN.md`「アプリが伝えること」）。
  // ブラウザではログファイルに書けないので、コンソールに出て終わります。
  const report = (what: string, detail: unknown): void => {
    void ipc.logFrontendError(`${what}: ${String(detail)}`).catch(() => {
      // 記録すら通らないなら、これ以上できることはない。
    });
  };
  window.addEventListener("error", (event) => {
    report("uncaught", event.error ?? event.message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    report("unhandled rejection", event.reason);
  });

  createRoot(root).render(
    <StrictMode>
      <Demo sections={sections} platform={platform} />
    </StrictMode>,
  );
}

main();
