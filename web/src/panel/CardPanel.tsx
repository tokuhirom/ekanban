// カードの編集パネル。
//
// ボードに重ねず、右端に押し出して置きます。重ねると右端のカラムが隠れ、
// ドロップ先が見えなくなるためです（gpui 版と同じ）。
//
// **下書きはここが持ちます**（`docs/TAURI-MIGRATION.md` §2）。打っている間は
// Rust に渡さず、保存を押した 1 回だけ `add_card` か `update_card` を呼びます。
// gpui 版が「先にカードを足して、タイトルが入るまで保存を保留する」形だったのは、
// 下書きの置き場所がモデルの中にしか無かったからで、その経路はここで消えます。

import { useState } from "react";

import { useIpc } from "../ipc";
import type { AppError } from "../ipc/types/AppError";
import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { Field } from "../ipc/types/Field";
import type { Platform } from "../ipc/types/Platform";
import type { Snapshot } from "../ipc/types/Snapshot";
import { useAppActions } from "../shell/actions";
import { Description } from "./Description";
import type { Editing } from "../state/board";
import {
  deleteChecklistItem,
  draftIsSavable,
  draftOf,
  emptyDraft,
  moveChecklistItem,
  quickDueDates,
  setChecklistText,
  toggleChecklistItem,
  toggleTag,
  type CardDraft,
} from "./draft";

interface Props {
  board: Board;
  editing: Editing;
  /** `due_statuses` を出した日。期限の近道はここから数える（ブラウザの時計ではなく）。 */
  today: string;
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
  today,
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
  const [menuOpen, setMenuOpen] = useState(false);

  const savable = draftIsSavable(draft);

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
        ? ipc.addCard(editing.columnId, draft.title, draft.description)
        : ipc.updateCard(
            editing.cardId,
            draft.title,
            draft.description,
            draft.dueDate,
            draft.tagIds,
            draft.checklist,
          ),
    );
    setFailed(failure);
    // 断られた値は打ち直せるように残す。通ったときだけ閉じる。
    if (failure === null) onClose();
  }

  const columnName =
    editing.kind === "new"
      ? (board.columns.find((column) => column.id === editing.columnId)?.name ?? "カラム不明")
      : (board.columns.find((column) => column.cards.some((each) => each.id === editing.cardId))
          ?.name ?? "カラム不明");

  return (
    <aside
      className="panel card-panel"
      aria-label="カードの編集"
      // カラム名やタグ名の編集と同じく Escape で閉じます。パネル全体では Enter を
      // 取りません——説明が複数行なので、改行のほうを優先します。保存する Enter は
      // タイトル欄の中だけ（`docs/DESIGN.md`）。
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
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
          <button type="button" className="ghost" aria-label="閉じる" onClick={onClose}>
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
            if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
            event.preventDefault();
            void save();
          }}
        />
        {draft.title.trim() === "" && <FieldError message="タイトルを入力してください" />}
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

        {/* 期限・チェックリスト・タグは、保存済みのカードにしか付けられません。
            `add_card` が受けるのはタイトルと説明だけで、まだカードが無いうちは
            付ける先がないからです（§3）。足したあとに開いて付けます。 */}
        {editing.kind === "card" && (
          <>
            <label className="field-label" htmlFor="card-due-date">
              期限
            </label>
            <input
              id="card-due-date"
              className="field-input card-due-input"
              value={draft.dueDate}
              placeholder="YYYY-MM-DD（空欄で期限なし）"
              onChange={(event) => {
                setDraft({ ...draft, dueDate: event.target.value });
              }}
            />
            <FieldFailure failure={failed} field="dueDate" />
            <div className="button-row">
              {quickDueDates(today).map((quick) => (
                <button
                  key={quick.label}
                  type="button"
                  className="secondary"
                  onClick={() => {
                    setDraft({ ...draft, dueDate: quick.date });
                  }}
                >
                  {quick.label}
                </button>
              ))}
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  setDraft({ ...draft, dueDate: "" });
                }}
              >
                クリア
              </button>
            </div>

            <span className="field-label">チェックリスト</span>
            <FieldFailure failure={failed} field="checklistItem" />
            {draft.checklist.map((item, index) => (
              <div className="checklist-row" key={item.id ?? `new-${index}`}>
                <button
                  type="button"
                  className="secondary checklist-toggle"
                  aria-pressed={item.checked}
                  aria-label={`${item.text} を${item.checked ? "外す" : "チェックする"}`}
                  onClick={() => {
                    setDraft({ ...draft, checklist: toggleChecklistItem(draft.checklist, index) });
                  }}
                >
                  {item.checked ? "☑" : "□"}
                </button>
                <input
                  className="field-input checklist-text"
                  value={item.text}
                  placeholder="項目"
                  aria-label={`チェックリストの ${index + 1} 番目`}
                  onChange={(event) => {
                    setDraft({
                      ...draft,
                      checklist: setChecklistText(draft.checklist, index, event.target.value),
                    });
                  }}
                />
                <button
                  type="button"
                  className="ghost"
                  aria-label="上へ"
                  disabled={index === 0}
                  onClick={() => {
                    setDraft({
                      ...draft,
                      checklist: moveChecklistItem(draft.checklist, index, "up"),
                    });
                  }}
                >
                  ↑
                </button>
                <button
                  type="button"
                  className="ghost"
                  aria-label="下へ"
                  disabled={index + 1 >= draft.checklist.length}
                  onClick={() => {
                    setDraft({
                      ...draft,
                      checklist: moveChecklistItem(draft.checklist, index, "down"),
                    });
                  }}
                >
                  ↓
                </button>
                <button
                  type="button"
                  className="secondary"
                  aria-label="項目を削除"
                  onClick={() => {
                    setDraft({ ...draft, checklist: deleteChecklistItem(draft.checklist, index) });
                  }}
                >
                  削除
                </button>
                {item.text.trim() === "" && <FieldError message="項目名を入力してください" />}
              </div>
            ))}
            <div className="button-row">
              <button
                type="button"
                className="secondary add-checklist-item"
                onClick={() => {
                  // まだ保存していない項目は `id` を持ちません。`update_card` が
                  // `null` を「新しい項目」として読みます。
                  setDraft({
                    ...draft,
                    checklist: [...draft.checklist, { id: null, text: "", checked: false }],
                  });
                }}
              >
                ＋ 項目を追加
              </button>
            </div>

            <span className="field-label">タグ</span>
            <div className="button-row card-tag-picker">
              {board.tags.length === 0 && (
                <span className="field-note">タグはまだありません（タグ整理から追加）</span>
              )}
              {board.tags.map((tag) => {
                const selected = draft.tagIds.includes(tag.id);
                return (
                  <button
                    key={tag.id}
                    type="button"
                    className="secondary"
                    aria-pressed={selected}
                    onClick={() => {
                      setDraft({ ...draft, tagIds: toggleTag(draft.tagIds, tag.id) });
                    }}
                  >
                    {/* 色だけに意味を持たせない。選んであることは印でも書く。 */}
                    {selected ? "✓ " : ""}
                    {tag.name}
                  </button>
                );
              })}
            </div>
          </>
        )}
      </div>

      <footer className="panel-footer">
        <button type="button" className="secondary" onClick={onClose}>
          キャンセル
        </button>
        <button type="button" className="primary save-card" disabled={!savable} onClick={() => void save()}>
          保存
        </button>
      </footer>
    </aside>
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

/// Rust が入力欄に返した理由を、その欄の脇に出す（§3）。
function FieldFailure({ failure, field }: { failure: AppError | null; field: Field }) {
  if (failure?.field !== field) return null;
  return <FieldError message={failure.detail} />;
}
