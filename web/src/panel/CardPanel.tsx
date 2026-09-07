// カードの編集パネル。
//
// ボードに重ねず、右端に押し出して置きます。重ねると右端のカラムが隠れ、
// ドロップ先が見えなくなるためです。
//
// **下書きはここが持ちます**（`docs/DESIGN.md`「状態の持ち主」）。打っている間は
// Rust に渡さず、保存を押した 1 回だけ `add_card` か `update_card` を呼びます。
// 出す欄は新しいカードでも保存済みのカードでも同じで、どちらも下書きを丸ごと
// 渡します（#127）。違うのは呼ぶコマンドと、向ける先のあるカード操作（コピー・
// アーカイブ・削除）を出すかどうかだけです。
// **無題のカードが盤面に現れる経路がありません**——足してから引っこめる形を
// 取らないので、取り下げが履歴に残ることもありません。

import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import type { DragEndEvent } from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useEffect, useRef, useState } from "react";

import { useIpc } from "../ipc";
import type { AppError } from "../ipc/types/AppError";
import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { DueDatePreview } from "../ipc/types/DueDatePreview";
import type { Field } from "../ipc/types/Field";
import type { Platform } from "../ipc/types/Platform";
import type { Snapshot } from "../ipc/types/Snapshot";
import type { Tag } from "../ipc/types/Tag";
import { useAppActions } from "../shell/actions";
import { isComposing } from "../shell/ime";
import { Description } from "./Description";
import type { Editing } from "../state/board";
import {
  checklistToSend,
  deleteChecklistItem,
  draftIsSavable,
  draftOf,
  emptyDraft,
  insertChecklistItemAfter,
  moveChecklistItem,
  newChecklistItem,
  reorderChecklist,
  setChecklistText,
  toggleChecklistItem,
  toggleTag,
  type CardDraft,
  type DraftChecklistItem,
} from "./draft";
import { DEFAULT_TAG_COLOR, findTagByName, suggestTags } from "./tags";

interface Props {
  board: Board;
  editing: Editing;
  /** 説明の中のリンクを開く修飾キーを決めるのに使う（ADR 0002）。 */
  platform: Platform;
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
  onClose: () => void;
  /** 削除・アーカイブの確認を頼む。出すかどうかを決めるのは呼ぶ側。 */
  onDeleteCard: (cardId: number) => void;
  onArchiveCard: (cardId: number) => void;
}

export function CardPanel({
  board,
  editing,
  platform,
  run,
  onClose,
  onDeleteCard,
  onArchiveCard,
}: Props) {
  const ipc = useIpc();
  const card = editing.kind === "card" ? findCard(board, editing.cardId) : null;
  // 下書きは開いたときの 1 回だけ起こします。**そのあとは `card` を見ません**
  // ——保存のたびに新しいスナップショットが来るので、見ていると打っている内容が
  // 保存直後の値で上書きされます。対象が変わったときは `Board` が `key` で
  // この部品ごと作り直します。
  const [draft, setDraft] = useState<CardDraft>(() =>
    card === null ? emptyDraft() : draftOf(card),
  );
  const [failed, setFailed] = useState<AppError | null>(null);
  // 打った文字を Rust がどう読んだか（#134）。読み方は向こうに 1 つだけなので、
  // 往復するのは文字列と、読めた 1 日付だけです（`Description` と同じ考え方）。
  const [duePreview, setDuePreview] = useState<DueDatePreview | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  // 次にフォーカスを移すチェックリストの行（#138）。**添字ではなく鍵で指します**
  // ——並べ替えや削除で添字は別の行を指すようになります。当てたら、その行の
  // `onFocus` でここを空にします（effect の中で状態を書かないため）。
  const [focusChecklistKey, setFocusChecklistKey] = useState<string | null>(null);
  // 押しただけでドラッグが始まらないよう、盤面と同じだけ動かしてから掴んだと
  // 判定します。行には入力欄があるので、これが無いと文字を選ぶだけの操作が
  // ドラッグになります。
  const checklistSensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
  );

  const savable = draftIsSavable(draft);

  useEffect(() => {
    let cancelled = false;
    ipc
      .dueDatePreview(draft.dueDate)
      .then((read) => {
        if (!cancelled) setDuePreview(read);
      })
      .catch(() => {
        // 読めた日付が出ないだけなので、打つ手を止めない。
        if (!cancelled) setDuePreview(null);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc, draft.dueDate]);

  // メニューの「保存」「編集をキャンセル」は、開いているパネルのものです。
  // **下書きを持っているのはここ**なので、受けるのもここ（`shell/actions.ts`）。
  useAppActions({
    saveEdit: () => {
      void save();
    },
    cancelEdit: onClose,
  });

  async function save() {
    if (!savable) return;
    const failure = await run(() =>
      editing.kind === "new"
        ? ipc.addCard(
            editing.columnId,
            draft.title,
            draft.description,
            draft.dueDate,
            draft.tagIds,
            checklistToSend(draft.checklist),
          )
        : ipc.updateCard(
            editing.cardId,
            draft.title,
            draft.description,
            draft.dueDate,
            draft.tagIds,
            checklistToSend(draft.checklist),
          ),
    );
    setFailed(failure);
    // 断られた値は打ち直せるように残す。通ったときだけ閉じる。
    if (failure === null) onClose();
  }

  /// 打った名前のタグをその場で作り、この下書きに付ける（#115、ADR 0027）。
  ///
  /// 作った ID は `add_tag` が返すスナップショットから引きます。`run()` が返すのは
  /// `Validation` の失敗だけなので、盤面そのものはクロージャの中で受け取ります。
  /// 色は既定色で、整えるのはタグ整理パネルの仕事です。
  async function createTag(name: string): Promise<void> {
    const created: { id: number | null } = { id: null };
    const failure = await run(async () => {
      const snapshot = await ipc.addTag(name, DEFAULT_TAG_COLOR);
      created.id = findTagByName(snapshot.board.tags, name)?.id ?? null;
      return snapshot;
    });
    setFailed(failure);
    const tagId = created.id;
    if (failure !== null || tagId === null) return;
    setDraft((current) => ({
      ...current,
      tagIds: toggleTag(current.tagIds, tagId),
    }));
  }

  const columnName =
    editing.kind === "new"
      ? (board.columns.find((column) => column.id === editing.columnId)?.name ??
        "カラム不明")
      : (board.columns.find((column) =>
          column.cards.some((each) => each.id === editing.cardId),
        )?.name ?? "カラム不明");

  return (
    <aside
      className="panel card-panel"
      aria-label="カードの編集"
      // カラム名やタグ名の編集と同じく Escape で閉じます。パネル全体では Enter を
      // 取りません——説明が複数行なので、改行のほうを優先します。保存する Enter は
      // タイトル欄の中だけ（`docs/DESIGN.md`）。
      onKeyDown={(event) => {
        // 変換を取り消す Escape でパネルを閉じない。打ちかけの下書きが消える
        // （`shell/ime.ts`）。
        if (event.key !== "Escape" || isComposing(event.nativeEvent)) return;
        event.stopPropagation();
        onClose();
      }}
    >
      <header className="panel-header">
        <div className="panel-heading">
          <span className="panel-context">{columnName} のカード</span>
          <strong className="panel-title">
            {editing.kind === "new" ? "新しいカード" : `#${editing.cardId}`}
          </strong>
        </div>
        <div className="panel-actions">
          {/* 常用しない操作は畳む（`docs/DESIGN.md`）。新しいカードにはコピーも
              アーカイブも削除も向ける先が無いので、そもそも出しません。 */}
          {editing.kind === "card" && (
            <button
              type="button"
              className="ghost card-panel-menu-button"
              aria-label="カードの操作"
              aria-expanded={menuOpen}
              onClick={() => {
                setMenuOpen((open) => !open);
              }}
            >
              ⋮
            </button>
          )}
          <button
            type="button"
            className="ghost"
            aria-label="閉じる"
            onClick={onClose}
          >
            ✕
          </button>
        </div>
        {/* 重なりは CSS の積み重ね文脈で決まるので、#78（入力欄の下に潜る）と
            同じ壊れ方はしません。`.panel-header` に `z-index` を持たせてあります。 */}
        {menuOpen && editing.kind === "card" && (
          <div className="menu card-panel-menu">
            <button
              type="button"
              className="ghost"
              onClick={() => {
                setMenuOpen(false);
                void run(() => ipc.copyCard(editing.cardId));
              }}
            >
              コピー
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => {
                setMenuOpen(false);
                onArchiveCard(editing.cardId);
              }}
            >
              アーカイブ
            </button>
            <button
              type="button"
              className="danger-item"
              onClick={() => {
                setMenuOpen(false);
                onDeleteCard(editing.cardId);
              }}
            >
              削除
            </button>
          </div>
        )}
      </header>

      <div className="panel-body">
        <label className="field-label" htmlFor="card-title">
          タイトル
        </label>
        <input
          id="card-title"
          className="field-input card-title-input"
          value={draft.title}
          placeholder="カードのタイトル"
          autoFocus
          onChange={(event) => {
            setDraft({ ...draft, title: event.target.value });
          }}
          // 1 行の欄なので Enter は改行ではなく保存（`docs/DESIGN.md`）。
          // カードを足すときはタイトルを打つのが最後の操作なので、そのまま
          // 終われないと保存ボタンまで手が要ります。
          onKeyDown={(event) => {
            if (event.key !== "Enter" || isComposing(event.nativeEvent)) return;
            event.preventDefault();
            void save();
          }}
        />
        {draft.title.trim() === "" && (
          <FieldError message="タイトルを入力してください" />
        )}
        <FieldFailure failure={failed} field="cardTitle" />

        <label className="field-label" htmlFor="card-description">
          説明
        </label>
        <Description
          id="card-description"
          value={draft.description}
          platform={platform}
          onChange={(description) => {
            setDraft({ ...draft, description });
          }}
        />

        {/* 期限・チェックリスト・タグは、新しいカードにも出します（#127）。
            `add_card` が下書きを丸ごと受けるので、足してから開き直す往復が要りません。 */}
        <label className="field-label" htmlFor="card-due-date">
          期限
        </label>
        {/* 期限は文字で打ちます（#134、ADR 0031）。`type="date"` をやめたのは、
            カレンダーの見た目と操作が webview ごとに違い、キーボードから速く
            打てないためです。**読み方は Rust に 1 つだけ**——「明日」が何日かを
            ここでも数えると、`due_statuses` を出した判定と食い違います。 */}
        {/* 外す × は欄に重ねず右へ並べます。期限が入っているときだけ出すので、
            期限なしのカードでは欄が入力 1 つになります（#128）。 */}
        <div className="due-field">
          <input
            id="card-due-date"
            type="text"
            className="field-input card-due-input"
            placeholder="9/12、明日、金、+3"
            autoComplete="off"
            value={draft.dueDate}
            onChange={(event) => {
              setDraft({ ...draft, dueDate: event.target.value });
            }}
          />
          {draft.dueDate !== "" && (
            <button
              type="button"
              className="ghost due-clear"
              aria-label="期限を外す"
              title="期限を外す"
              onClick={() => {
                setDraft({ ...draft, dueDate: "" });
              }}
            >
              ×
            </button>
          )}
        </div>
        {/* 打った文字がどう読まれたかを、確定する前に見せます。読めない間は
            何も出しません——打っている途中の文字はまだ間違いではないので、
            断りは保存のときに欄の脇へ出ます。 */}
        {duePreview !== null && (
          <p className="field-note due-preview">→ {duePreview.label}</p>
        )}
        <FieldFailure failure={failed} field="dueDate" />

        <span className="field-label">チェックリスト</span>
        <FieldFailure failure={failed} field="checklistItem" />
        {/* 掴んで並べ替える（#113）。盤面とは別の `DndContext` です——
            パネルは盤面の外にあり、落とし先の候補が混ざる意味がありません。
            **何番目に落ちたかを決めるのは `draft.ts`** で、ライブラリに
            任せるのは掴む・追う・落とすまで（`docs/DESIGN.md`
            「ドラッグ＆ドロップ」）。動かすのは下書きの配列だけなので、
            落とした瞬間に Rust は呼びません。 */}
        <DndContext
          sensors={checklistSensors}
          collisionDetection={closestCenter}
          onDragEnd={(event: DragEndEvent) => {
            if (event.over === null) return;
            setDraft({
              ...draft,
              checklist: reorderChecklist(
                draft.checklist,
                String(event.active.id),
                String(event.over.id),
              ),
            });
          }}
        >
          <SortableContext
            items={draft.checklist.map((item) => item.key)}
            strategy={verticalListSortingStrategy}
          >
            {draft.checklist.map((item, index) => (
              <ChecklistRow
                key={item.key}
                item={item}
                index={index}
                count={draft.checklist.length}
                onToggle={() => {
                  setDraft({
                    ...draft,
                    checklist: toggleChecklistItem(draft.checklist, index),
                  });
                }}
                onChangeText={(text) => {
                  setDraft({
                    ...draft,
                    checklist: setChecklistText(draft.checklist, index, text),
                  });
                }}
                onMove={(direction) => {
                  setDraft({
                    ...draft,
                    checklist: moveChecklistItem(
                      draft.checklist,
                      index,
                      direction,
                    ),
                  });
                }}
                onDelete={() => {
                  setDraft({
                    ...draft,
                    checklist: deleteChecklistItem(draft.checklist, index),
                  });
                }}
                focused={focusChecklistKey === item.key}
                onFocused={() => {
                  setFocusChecklistKey(null);
                }}
                // `Enter` で次の行、末尾の空行なら畳んで抜ける（#138）。
                onSplit={() => {
                  const lastAndEmpty =
                    index + 1 === draft.checklist.length && item.text.trim() === "";
                  if (lastAndEmpty) {
                    setDraft({
                      ...draft,
                      checklist: deleteChecklistItem(draft.checklist, index),
                    });
                    setFocusChecklistKey(null);
                    return;
                  }
                  const inserted = insertChecklistItemAfter(draft.checklist, index);
                  setDraft({ ...draft, checklist: inserted.checklist });
                  setFocusChecklistKey(inserted.key);
                }}
                // 空の行で `Backspace` なら、その行を消して上の行の末尾へ。
                onBackspaceEmpty={() => {
                  setDraft({
                    ...draft,
                    checklist: deleteChecklistItem(draft.checklist, index),
                  });
                  setFocusChecklistKey(draft.checklist[index - 1]?.key ?? null);
                }}
              />
            ))}
          </SortableContext>
        </DndContext>
        <div className="button-row">
          <button
            type="button"
            className="secondary add-checklist-item"
            onClick={() => {
              // 名前を入れないままにした行は、保存のときに Rust が落とします
              // （#114）。消しにいかなくても保存できます。
              const item = newChecklistItem();
              setDraft({ ...draft, checklist: [...draft.checklist, item] });
              // 足した行にフォーカスを移します（#138）。押してから欄を押し直す
              // 往復が、続けて打つときにいちばん効きます。
              setFocusChecklistKey(item.key);
            }}
          >
            ＋ 項目を追加
          </button>
        </div>

        <span className="field-label" id="card-tags-label">
          タグ
        </span>
        <TagsInput
          tags={board.tags}
          selected={draft.tagIds}
          failure={failed}
          onToggle={(tagId) => {
            setDraft({ ...draft, tagIds: toggleTag(draft.tagIds, tagId) });
          }}
          onCreate={createTag}
        />
      </div>

      <footer className="panel-footer">
        <button type="button" className="secondary" onClick={onClose}>
          キャンセル
        </button>
        <button
          type="button"
          className="primary save-card"
          disabled={!savable}
          onClick={() => void save()}
        >
          保存
        </button>
      </footer>
    </aside>
  );
}

/// カードに付けるタグの欄。選んだタグのチップと、打ち込む欄（#115、ADR 0027）。
///
/// **打った名前が既にあるタグならそれを選び、無ければ作って選びます。** 大文字
/// 小文字と前後の空白は無視して突き合わせるので、同じ名前のタグが 2 つできる
/// ことはありません。作るところまでをここに置くのは、タグ整理パネルを開いて
/// 戻ってくる往復が、カードを書いている最中には重すぎるからです。名前の変更・
/// 色・削除は今までどおりタグ整理パネルにしか置きません。
///
/// チップの `✕` は「このカードから外す」で、タグそのものは残ります。
function TagsInput({
  tags,
  selected,
  failure,
  onToggle,
  onCreate,
}: {
  tags: readonly Tag[];
  selected: readonly number[];
  failure: AppError | null;
  onToggle: (tagId: number) => void;
  onCreate: (name: string) => Promise<void>;
}) {
  const [typed, setTyped] = useState("");
  const chips = selected
    .map((tagId) => tags.find((tag) => tag.id === tagId))
    .filter((tag): tag is Tag => tag !== undefined);
  const suggestions = suggestTags(tags, selected, typed);

  /// 打った名前を確定する。既にあれば選ぶだけ、無ければ作る。
  function commit() {
    const name = typed.trim();
    if (name === "") return;
    const existing = findTagByName(tags, name);
    setTyped("");
    if (existing !== null) {
      if (!selected.includes(existing.id)) onToggle(existing.id);
      return;
    }
    void onCreate(name);
  }

  return (
    <>
      <div className="tags-input">
        {chips.map((tag) => (
          <span
            key={tag.id}
            className="tag-chip tags-input-chip"
            style={{ background: tag.color }}
          >
            {tag.name}
            <button
              type="button"
              className="tags-input-remove"
              aria-label={`${tag.name} を外す`}
              onClick={() => {
                onToggle(tag.id);
              }}
            >
              ✕
            </button>
          </span>
        ))}
        <input
          className="tags-input-field"
          value={typed}
          placeholder={
            chips.length === 0 ? "タグを打って Enter（無ければ作ります）" : ""
          }
          aria-labelledby="card-tags-label"
          onChange={(event) => {
            setTyped(event.target.value);
          }}
          onKeyDown={(event) => {
            // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。IME の変換を
            // 確定する Enter でタグを作らないよう `shell/ime.ts` を通す。
            if (event.key === "Enter" && !isComposing(event.nativeEvent)) {
              event.preventDefault();
              commit();
              return;
            }
            // 空の欄での Backspace は末尾のチップを外す。打ち間違えたタグを、
            // チップまでポインタを運ばずに取り消せるようにする。
            const last = chips[chips.length - 1];
            if (
              event.key === "Backspace" &&
              typed === "" &&
              last !== undefined
            ) {
              event.preventDefault();
              onToggle(last.id);
            }
          }}
        />
      </div>
      <FieldFailure failure={failure} field="tagName" />
      {/* どんなタグがあるかを見せる道は残す。打つと候補が絞られる。 */}
      {suggestions.length > 0 && (
        <div className="button-row tag-suggestions">
          {suggestions.map((tag) => (
            <button
              key={tag.id}
              type="button"
              className="secondary"
              onClick={() => {
                setTyped("");
                onToggle(tag.id);
              }}
            >
              {tag.name}
            </button>
          ))}
        </div>
      )}
    </>
  );
}

/// チェックリストの 1 行。掴む場所（`⠿`）と、上下の矢印の両方を持ちます。
///
/// **矢印は残します**（#113）——キーボードだけで並べ替える道を消さないためです。
/// 掴む場所を行全体にしないのは、行の中に入力欄があるからで、カラムのヘッダと
/// カードで掴む場所を分けているのと同じ理由です。
function ChecklistRow({
  item,
  index,
  count,
  onToggle,
  onChangeText,
  onMove,
  onDelete,
  focused,
  onFocused,
  onSplit,
  onBackspaceEmpty,
}: {
  item: DraftChecklistItem;
  index: number;
  count: number;
  onToggle: () => void;
  onChangeText: (text: string) => void;
  onMove: (direction: "up" | "down") => void;
  onDelete: () => void;
  /** この行に打ち込ませたい（#138）。 */
  focused: boolean;
  onFocused: () => void;
  /** `Enter`。次の行を作るか、末尾の空行なら畳んで抜ける。 */
  onSplit: () => void;
  /** 空の行で `Backspace`。 */
  onBackspaceEmpty: () => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: item.key });
  const input = useRef<HTMLInputElement>(null);

  // 足された行・分けた行に打ち込めるようにする（#138）。当たったことは
  // `onFocus` が親に返すので、ここで状態は書きません。
  useEffect(() => {
    if (!focused) return;
    const element = input.current;
    if (element === null) return;
    element.focus();
    // 上の行へ戻ったときは、続きから打てるように末尾へ。
    element.setSelectionRange(element.value.length, element.value.length);
  }, [focused]);

  return (
    <div
      ref={setNodeRef}
      className="checklist-row"
      data-dragging={isDragging || undefined}
      style={{
        transform: CSS.Translate.toString(transform),
        transition: transition ?? undefined,
      }}
    >
      <button
        type="button"
        className="ghost checklist-handle"
        ref={setActivatorNodeRef}
        aria-label={`チェックリストの ${index + 1} 番目を掴んで並べ替える`}
        title="掴んで並べ替える"
        {...attributes}
        {...listeners}
      >
        ⠿
      </button>
      <button
        type="button"
        className="secondary checklist-toggle"
        aria-pressed={item.checked}
        aria-label={`${item.text} を${item.checked ? "外す" : "チェックする"}`}
        onClick={onToggle}
      >
        {item.checked ? "☑" : "□"}
      </button>
      <input
        ref={input}
        className="field-input checklist-text"
        value={item.text}
        placeholder="項目"
        aria-label={`チェックリストの ${index + 1} 番目`}
        onChange={(event) => {
          onChangeText(event.target.value);
        }}
        onFocus={onFocused}
        // 箇条書きと同じ流れで打てるようにします（#138）。**変換中の
        // `Enter` は取りません**——確定しただけで行が増えます（ADR 0029）。
        onKeyDown={(event) => {
          if (isComposing(event.nativeEvent)) return;
          if (event.key === "Enter") {
            event.preventDefault();
            onSplit();
            return;
          }
          if (event.key === "Backspace" && item.text === "") {
            event.preventDefault();
            onBackspaceEmpty();
          }
        }}
      />
      <button
        type="button"
        className="ghost"
        aria-label="上へ"
        disabled={index === 0}
        onClick={() => {
          onMove("up");
        }}
      >
        ↑
      </button>
      <button
        type="button"
        className="ghost"
        aria-label="下へ"
        disabled={index + 1 >= count}
        onClick={() => {
          onMove("down");
        }}
      >
        ↓
      </button>
      <button
        type="button"
        className="secondary"
        aria-label="項目を削除"
        onClick={onDelete}
      >
        削除
      </button>
    </div>
  );
}

function findCard(board: Board, cardId: number): Card | null {
  for (const column of board.columns) {
    const found = column.cards.find((card) => card.id === cardId);
    if (found !== undefined) return found;
  }
  return null;
}

function FieldError({ message }: { message: string }) {
  return (
    <p className="field-error" role="alert">
      {message}
    </p>
  );
}

/// Rust が入力欄に返した理由を、その欄の脇に出す（`docs/DESIGN.md`「コマンドとイベント」）。
function FieldFailure({
  failure,
  field,
}: {
  failure: AppError | null;
  field: Field;
}) {
  if (failure?.field !== field) return null;
  return <FieldError message={failure.detail} />;
}
