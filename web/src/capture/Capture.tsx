// 1 行入力の窓（`docs/DESIGN.md`「クイックキャプチャ」）。
//
// `Enter` で足して閉じ、`Escape` で閉じます。**失敗したときは閉じません**——
// 打った 1 行が、閉じたことで消えるのを避けるためです。
//
// 入れ先は「〇〇ボード / △△カラム」として常に見せます。どこに入るのか分からない
// まま放り込ませない、というのが元からの決めごとです。
//
// **書くのはボードの窓と同じ経路**です（[ADR 0039]）。盤面をここでも読み、
// モデルで 1 枚足して `save_document` で書きます——Undo に積まれ、`created` が
// 1 件残るところまで同じです。書いたことはボードの窓に `board:changed` で
// 届き、そちらが読み直します。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { useEffect, useRef, useState } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { BoardDocument } from "../ipc/types/BoardDocument";
import { addCard, cloneDocument } from "../model/board";
import type { BoardDocument as ModelDocument } from "../model/board";
import type { CaptureDestination } from "../model/capture";
import { resolveCaptureTarget } from "../model/capture";
import { isComposing } from "../shell/ime";

export function Capture() {
  const ipc = useIpc();
  const [title, setTitle] = useState("");
  const [documents, setDocuments] = useState<readonly BoardDocument[]>([]);
  const [target, setTarget] = useState<CaptureDestination | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  // 保存を頼んで待っている間は `true`。`Enter` の二重押しを受けない。
  const [saving, setSaving] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  // **窓を開くたびに読み直します。** 閉じている間にボードの窓が書いている
  // ことがあるので、前に開いたときの写しは使えません（`docs/DESIGN.md`）。
  useEffect(() => {
    let cancelled = false;
    Promise.all([ipc.loadDocuments(), ipc.captureTarget()])
      .then(([fresh, stored]) => {
        if (cancelled) return;
        setDocuments(fresh);
        setTarget(resolveCaptureTarget(fresh, stored));
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(describeFailure(error).detail);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc]);

  // 入れ先が届くまで入力欄は `disabled` で、`autoFocus` は無効な要素には効かない。
  // ホットキーを押した人がそのまま 1 行打てることがこの窓の存在理由なので、
  // 使えるようになった時点でこちらから焦点を移す。
  useEffect(() => {
    if (target !== null) input.current?.focus();
  }, [target]);

  async function save() {
    if (saving || target === null || title.trim() === "") return;
    const stored = documents.find((document) => document.board.id === target.boardId);
    if (stored === undefined) return;
    setSaving(true);

    // ボードの窓と同じ経路。モデルに当ててから、`save_document` で書く。
    const document: ModelDocument = cloneDocument({
      ...stored,
      pendingEvents: [],
      undoStack: [],
      redoStack: [],
    });
    const outcome = addCard(document, target.columnId, title, "");
    if (!outcome.ok) {
      setFailure("タイトルを入力してください");
      setSaving(false);
      return;
    }

    try {
      await ipc.saveDocument(document, document.pendingEvents);
      // 書けたら閉じる。ボードの窓には `board:changed` が届いている。
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
        if (isComposing(event.nativeEvent)) return;
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
        ref={input}
        className="field-input capture-input"
        value={title}
        placeholder="思いついたことを 1 行で"
        aria-label="キャプチャするカードのタイトル"
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
