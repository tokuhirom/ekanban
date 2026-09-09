// クイックキャプチャの窓（`docs/DESIGN.md`「画面の作り」の `capture/`）。
//
// **ボードとは別のエントリポイント**です。1 行を放り込むだけの窓に、盤面の
// 描画と D&D の一式を積む理由がありません。読むものが少ないほど、ホットキーを
// 押してから打てるようになるまでが短くなります。

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { setIpc } from "../ipc";
import { detectPlatform, localIpc, usesLocalStore } from "../ipc/local";
import { tauriIpc } from "../ipc/tauri";
import "../styles.css";
import { Capture } from "./Capture";

// ボードの窓と同じ見分け方（`src/main.tsx`）。同じ生まれのページなので、
// `?store=local` のときは同じ `localStorage` の盤面を読みます。
setIpc(usesLocalStore() ? localIpc(detectPlatform()) : tauriIpc);

const root = document.getElementById("root");
if (root === null) throw new Error("#root がない");
createRoot(root).render(
  <StrictMode>
    <Capture />
  </StrictMode>,
);
