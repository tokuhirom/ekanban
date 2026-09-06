// 1 行入力の窓（§9）。
//
// `Enter` で足して閉じ、`Escape` で閉じます。**失敗したときは閉じません**——
// 打った 1 行が、閉じたことで消えるのを避けるためです（gpui 版と同じ）。
//
// 入れ先は「〇〇ボード / △△カラム」として常に見せます。どこに入るのか分からない
// まま放り込ませない、というのが元からの決めごとです。

import { useEffect, useState } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { CaptureTarget } from "../ipc/types/CaptureTarget";

export function Capture() {
  const ipc = useIpc();
  const [title, setTitle] = useState("");
  const [target, setTarget] = useState<CaptureTarget | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  // 保存を頼んで待っている間は `true`。`Enter` の二重押しを受けない。
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    ipc
      .captureTarget()
      .then((found) => {
        if (!cancelled) setTarget(found);
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(describeFailure(error).detail);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc]);

  async function save() {
    if (saving || title.trim() === "") return;
    setSaving(true);
    try {
      await ipc.captureCard(title);
      // 書けたら閉じる。ボードは `board:changed` で受け取っている。
      await ipc.closeCaptureWindow(true);
    } catch (error: unknown) {
      // 閉じない。打った 1 行を残したまま理由を出す。
      setFailure(describeFailure(error).detail);
      setSaving(false);
    }
  }

  return (
    <div
      className="capture"
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (event.key === "Enter") {
          event.preventDefault();
          void save();
        } else if (event.key === "Escape") {
          event.preventDefault();
          void ipc.closeCaptureWindow(true);
        }
      }}
    >
      <p className="capture-destination">
        {target === null
          ? "入れ先のカラムがありません"
          : `${target.boardName} / ${target.columnName}`}
      </p>
      <input
        className="field-input capture-input"
        value={title}
        placeholder="思いついたことを 1 行で"
        aria-label="キャプチャするカードのタイトル"
        autoFocus
        disabled={target === null}
        onChange={(event) => {
          setTitle(event.target.value);
        }}
      />
      <p className={failure === null ? "capture-hint" : "capture-hint failure"} role="status">
        {failure ?? (saving ? "保存中…" : "Enter で追加、Escape で閉じる")}
      </p>
    </div>
  );
}
