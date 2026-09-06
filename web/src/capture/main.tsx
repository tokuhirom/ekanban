// クイックキャプチャの窓（`docs/DESIGN.md`「画面の作り」の `capture/`）。
//
// **ボードとは別のエントリポイント**です。1 行を放り込むだけの窓に、盤面の
// 描画と D&D の一式を積む理由がありません。読むものが少ないほど、ホットキーを
// 押してから打てるようになるまでが短くなります。

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { setIpc } from "../ipc";
import { harnessIpc, harnessUrl } from "../ipc/harness";
import { tauriIpc } from "../ipc/tauri";
import "../styles.css";
import { Capture } from "./Capture";

const harness = harnessUrl();
setIpc(harness === null ? tauriIpc : harnessIpc(harness));

const root = document.getElementById("root");
if (root === null) throw new Error("#root がない");
createRoot(root).render(
  <StrictMode>
    <Capture />
  </StrictMode>,
);
