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
// **1 行には期限とタグも書けます**（[ADR 0051]）。切り分けるのは
// `model/quickCapture.ts` で、ここはその結果を 1 回の `addCardWithDetails` に
// 渡すだけです。タグ名から ID を引くのはカードの編集パネルと同じ道
// （`findTagByName`、無ければその場で作る。色は渡さない、[ADR 0044]）。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0044]: ../../../docs/adr/0044-tags-get-their-colour-automatically.md
// [ADR 0051]: ../../../docs/adr/0051-typing-a-due-date-and-tags-in-quick-capture.md

import { useEffect, useMemo, useRef, useState } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { BoardDocument } from "../ipc/types/BoardDocument";
import { addCardWithDetails, addTag, cloneDocument } from "../model/board";
import type { BoardDocument as ModelDocument } from "../model/board";
import type { CaptureDestination } from "../model/capture";
import { resolveCaptureTarget } from "../model/capture";
import { dueDatePreview } from "../model/due";
import { readQuickCapture } from "../model/quickCapture";
import type { QuickCaptureRead } from "../model/quickCapture";
import { AUTO_TAG_COLOR, findTagByName } from "../panel/tags";
import { DEFAULT_DAY_BOUNDARY_HOUR, localDay } from "../state/day";
import { describeBoardError } from "../state/errors";
import { isComposing } from "../shell/ime";

/** 何も打っていないときのヒント。記法の入り口は placeholder に置く。 */
const HINT = "Enter で追加、Escape で閉じる";

/// 打った 1 行がどう読まれたかを、確定する前に出す（[ADR 0031]、[ADR 0051]）。
///
/// **記法を使ったときだけ出します。** 普通の 1 行しか打たない人の画面は動かない
/// ほうがよいので、期限もタグも取れていなければ何も返しません。
///
/// [ADR 0031]: ../../../docs/adr/0031-typing-a-due-date.md
/// [ADR 0051]: ../../../docs/adr/0051-typing-a-due-date-and-tags-in-quick-capture.md
function readBack(read: QuickCaptureRead, today: string): string | null {
  if (read.dueDate === null && read.tagNames.length === 0) return null;
  const parts = [read.title];
  // 曜日を付けるのは期限の欄と同じ道。ロケールで言葉が変わらない。
  const due = read.dueDate === null ? null : dueDatePreview(read.dueDate, today);
  if (due !== null) parts.push(due.label);
  parts.push(...read.tagNames.map((name) => `#${name}`));
  return `→ ${parts.join(" ／ ")}`;
}

export function Capture() {
  const ipc = useIpc();
  /** 打たれた 1 行そのもの。タイトルはここから切り出す（`model/quickCapture.ts`）。 */
  const [line, setLine] = useState("");
  const [documents, setDocuments] = useState<readonly BoardDocument[]>([]);
  const [target, setTarget] = useState<CaptureDestination | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  // 保存を頼んで待っている間は `true`。`Enter` の二重押しを受けない。
  const [saving, setSaving] = useState(false);
  // 期限を読む基準日（[ADR 0048]）。**この窓は短命なので、日付が変わるのを
  // 見張りません**——開いた時点の 1 つで足ります。境界時刻が届くまでは既定。
  const [today, setToday] = useState(() => localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR));
  const input = useRef<HTMLInputElement>(null);

  // **窓を開くたびに読み直します。** 閉じている間にボードの窓が書いている
  // ことがあるので、前に開いたときの写しは使えません（`docs/DESIGN.md`）。
  //
  // 入れ先と日付の境界時刻は `startup_state` が両方とも持っているので、
  // 1 回で受けます。
  useEffect(() => {
    let cancelled = false;
    Promise.all([ipc.loadDocuments(), ipc.startupState()])
      .then(([fresh, startup]) => {
        if (cancelled) return;
        setDocuments(fresh);
        setTarget(resolveCaptureTarget(fresh, startup.captureTarget));
        setToday(localDay(new Date(), startup.dayBoundaryHour));
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(describeFailure(error).detail);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc]);

  // 打鍵のたびに切り分ける。盤面は手元にあるので往復しない。
  const read = useMemo(() => readQuickCapture(line, today), [line, today]);

  // 入れ先が届くまで入力欄は `disabled` で、`autoFocus` は無効な要素には効かない。
  // ホットキーを押した人がそのまま 1 行打てることがこの窓の存在理由なので、
  // 使えるようになった時点でこちらから焦点を移す。
  useEffect(() => {
    if (target !== null) input.current?.focus();
  }, [target]);

  async function save() {
    if (saving || target === null || read.title === "") return;
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

    // 打たれたタグ名を ID にする。無ければその場で作る（ADR 0027、ADR 0050）。
    // 突き合わせは編集パネルと同じ `findTagByName` なので、「Rust」と「rust」が
    // 別々にできることはない。作った端から `document` に入るので、同じ名前を
    // 2 度打っても 1 つ。
    const tagIds: number[] = [];
    for (const name of read.tagNames) {
      let tagId = findTagByName(document.board.tags, name)?.id ?? null;
      if (tagId === null) {
        const created = addTag(document, name, AUTO_TAG_COLOR);
        if (!created.ok) {
          setFailure(describeBoardError(created.error).detail);
          setSaving(false);
          return;
        }
        tagId = created.value;
      }
      if (!tagIds.includes(tagId)) tagIds.push(tagId);
    }

    // **1 回で足します。** 足してから期限とタグを当てると操作が 3 件積まれる。
    const outcome = addCardWithDetails(
      document,
      target.columnId,
      read.title,
      "",
      read.dueDate,
      tagIds,
      [],
    );
    if (!outcome.ok) {
      setFailure(describeBoardError(outcome.error).detail);
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

  // 記号しか打たれていない 1 行は足せない。**押す前に言います**——`Enter` を
  // 押しても何も起きない理由が、それまで画面に無い。
  const hint =
    line.trim() !== "" && read.title === ""
      ? "タイトルを入力してください"
      : (readBack(read, today) ?? HINT);

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
        value={line}
        placeholder="思いついたことを 1 行で（@today #タグ）"
        aria-label="キャプチャするカードのタイトル"
        disabled={target === null}
        onChange={(event) => {
          setLine(event.target.value);
        }}
      />
      <p className={failure === null ? "capture-hint" : "capture-hint failure"} role="status">
        {failure ?? (saving ? "保存中…" : hint)}
      </p>
    </div>
  );
}
