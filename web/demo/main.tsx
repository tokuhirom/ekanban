// ブラウザ版の入口（[ADR 0035]）。
//
// 配るアプリの入口は `src/main.tsx` です。違うのは 3 つだけで、**盤面は同じ
// `Board` がそのまま描きます**。
//
// 1. Rust を呼ぶ口が wasm（`src/ipc/wasm.ts`）。動いているのは本物の
//    `ekanban-core` で、SQLite のファイルは `localStorage` にあります
// 2. メニューバーをページが描く（`src/shell/MenuBar.tsx`）。ブラウザに OS の
//    メニューバーが無いためで、構成は Rust から受け取ります
// 3. どの OS かをページが名乗る。`wasm32-unknown-unknown` はどの OS でもない
//    ので、Rust がコンパイル時に知る手が使えません
//
// [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Board } from "../src/board/Board";
import { setIpc } from "../src/ipc";
import type { Platform } from "../src/ipc/types/Platform";
import type { WebSection } from "../src/ipc/types/WebSection";
import { fireAppAction, startWasmIpc, wasmIpc, wasmMenuSections } from "../src/ipc/wasm";
import { MenuBar, useMenuAccelerators } from "../src/shell/MenuBar";
import { hardenBoard } from "../src/shell/harden";
import "../src/styles.css";
import "./demo.css";

/// どの OS で開かれているか。
///
/// **ここだけはブラウザに訊きます。** 配るアプリでは Rust がコンパイル時に
/// 知っていて `StartupState.platform` で渡します（[ADR 0009]）が、
/// `wasm32-unknown-unknown` は macOS でも Windows でもないので、その手が
/// ありません。取り違えると `secondary` が Cmd か Ctrl かを間違え、割り当てが
/// 丸ごと効かなくなります。
///
/// `userAgentData.platform` を先に見るのは、そこだけが「OS を訊く」ための
/// API だからです。無いブラウザでは `userAgent` に落ちます。**当たらなかった
/// ときは Linux 扱い**——キーの割り当てが Ctrl 側になるだけで、押せなくなる
/// 項目はありません。
///
/// [ADR 0009]: ../../docs/adr/0009-per-platform-key-bindings.md
function detectPlatform(): Platform {
  const data: unknown = (navigator as { userAgentData?: unknown }).userAgentData;
  const reported =
    typeof data === "object" && data !== null && "platform" in data
      ? String(data.platform)
      : navigator.userAgent;
  if (/mac/i.test(reported)) return "macos";
  if (/win/i.test(reported)) return "windows";
  return "linux";
}

function Demo({
  sections,
  platform,
}: {
  sections: WebSection[];
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

async function main(): Promise<void> {
  const root = document.getElementById("root");
  if (root === null) throw new Error("#root がない");

  const platform = detectPlatform();
  setIpc(wasmIpc);
  hardenBoard();

  try {
    await startWasmIpc(platform);
  } catch (error: unknown) {
    // 起動に失敗したら、白い画面で終わらせない。ここで出せる先はここだけです
    // （`crates/core` の `diagnostics` には、ブラウザで書ける記録先がありません）。
    root.textContent = `起動できませんでした: ${String(error)}`;
    return;
  }

  const sections = wasmMenuSections(platform);

  // 未捕捉の例外は Rust 側と同じ経路に流す（`docs/DESIGN.md`「アプリが伝えること」）。
  // ブラウザではログファイルに書けないので、コンソールに出て終わります。
  const report = (what: string, detail: unknown): void => {
    void wasmIpc.logFrontendError(`${what}: ${String(detail)}`).catch(() => {
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

void main();
