import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Board } from "./board/Board";
import { setIpc } from "./ipc";
import { harnessIpc, harnessUrl } from "./ipc/harness";
import { tauriIpc } from "./ipc/tauri";
import { hardenWebview } from "./shell/harden";
import "./styles.css";

// ふつうのブラウザで開いたときは、開発用ハーネス越しに Rust を呼びます
// （`?harness=http://127.0.0.1:1421`、`docs/TAURI-MIGRATION.md` §10）。
// Tauri の中では `tauri.ts` です。
const harness = harnessUrl();
const ipc = harness === null ? tauriIpc : harnessIpc(harness);
setIpc(ipc);
hardenWebview();

// webview の未捕捉例外を、Rust 側と同じログに落とす（§9）。黙って消えると、
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
