import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Board } from "./board/Board";
import { setIpc } from "./ipc";
import { detectPlatform, localIpc, usesLocalStore } from "./ipc/local";
import { tauriIpc } from "./ipc/tauri";
import { hardenWebview } from "./shell/harden";
import "./styles.css";

// ふつうのブラウザで開いたときは、置き場所を `localStorage` に差します
// （`?store=local`、[ADR 0041]）。**盤面のコードは同じ**で、違うのは差した
// 置き場所だけです。Tauri の中では `tauri.ts` から SQLite を相手にします。
//
// [ADR 0041]: ../../docs/adr/0041-one-layer-of-screen-tests.md
const ipc = usesLocalStore() ? localIpc(detectPlatform()) : tauriIpc;
setIpc(ipc);
hardenWebview();

// webview の未捕捉例外を、Rust 側と同じログに落とす（`docs/DESIGN.md`「アプリが伝えること」）。黙って消えると、
// 原因を追う手段がなくなる。
function report(what: string, detail: unknown): void {
  void ipc.logFrontendError(`${what}: ${String(detail)}`).catch(() => {
    // 記録すら通らないなら、これ以上できることはない。
  });
}
window.addEventListener("error", (event) => {
  report("uncaught", event.error ?? event.message);
});
window.addEventListener("unhandledrejection", (event) => {
  report("unhandled rejection", event.reason);
});

const root = document.getElementById("root");
if (root === null) throw new Error("#root がない");
createRoot(root).render(
  <StrictMode>
    <Board />
  </StrictMode>,
);
