import { useSortable } from "@dnd-kit/sortable";
import {
  SortableContext,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useState } from "react";

import type { AppError } from "../ipc/types/AppError";
import type { Column as ColumnData } from "../ipc/types/Column";
import type { DueStatus } from "../model/due";
import type { BoardDocument, Outcome } from "../model/board";
import { renameColumn, setColumnDone } from "../model/board";
import type { Tag } from "../ipc/types/Tag";
import { isComposing } from "../shell/ime";
import { Card } from "./Card";
import { handleId } from "./dnd";

interface Props {
  column: ColumnData;
  tags: readonly Tag[];
  dueStatuses: ReadonlyMap<number, DueStatus>;
  /** `due_statuses` を出した日。カード表面の期限表示がここから年を決める。 */
  today: string;
  matched: ReadonlySet<number> | null;
  /** 絞り込んでいるタグ。カード上のチップに印を付ける。 */
  activeTag: number | null;
  onToggleTagFilter: (tagId: number) => void;
  selectedCard: number | null;
  /** 最後の 1 本は消せない。理由を言わずにコントロールを無効にする（`docs/DESIGN.md`）。 */
  lastColumn: boolean;
  /** クイックキャプチャの入れ先。決めるのは画面（`state/board.ts`）。 */
  captureTarget: boolean;
  /** このカラムをクイックキャプチャの入れ先にする。 */
  onSetCaptureColumn: (columnId: number) => void;
  run: (act: (document: BoardDocument) => Outcome<unknown>) => Promise<AppError | null>;
  onSelectCard: (cardId: number) => void;
  onOpenCard: (cardId: number) => void;
  onCardContextMenu: (cardId: number, at: { x: number; y: number }) => void;
  onNewCard: (columnId: number) => void;
  onArchiveColumn: (column: ColumnData) => void;
  onRemoveColumn: (column: ColumnData) => void;
}

/// カラムの中身。ヘッダを掴むとカラムごと動き、カードを掴むとカードが動く。
///
/// 掴む場所を分けているのは、カードの上でカラムのドラッグが始まると、
/// 1 枚動かすつもりが列ごと動くからです。
export function Column({
  column,
  tags,
  dueStatuses,
  today,
  matched,
  activeTag,
  onToggleTagFilter,
  selectedCard,
  lastColumn,
  captureTarget,
  onSetCaptureColumn,
  run,
  onSelectCard,
  onOpenCard,
  onCardContextMenu,
  onNewCard,
  onArchiveColumn,
  onRemoveColumn,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: handleId({ kind: "column", id: column.id }) });
  const cardIds = column.cards.map((card) =>
    handleId({ kind: "card", id: card.id }),
  );

  return (
    <section
      ref={setNodeRef}
      className="column"
      data-column={column.id}
      data-placeholder={isDragging || undefined}
      style={{
        transform: CSS.Translate.toString(transform),
        transition: transition ?? undefined,
      }}
    >
      {editing ? (
        <ColumnEditor
          column={column}
          run={run}
          onDone={() => {
            setEditing(false);
          }}
        />
      ) : (
        <header
          className="column-header"
          ref={setActivatorNodeRef}
          title="掴んでカラムを並べ替える。ダブルクリックで名前を変える"
          {...attributes}
          {...listeners}
          // 名前を変える入口を、ボード一覧（#119）とカードに揃えます（#145）。
          // 掴んだ判定は 4px 動かしてからなので、ダブルクリックとぶつかりません。
          // `…` の「編集」は残します——キーボードから辿れる入口が要ります。
          onDoubleClick={() => {
            setEditing(true);
          }}
        >
          <h2 className="column-name">{column.name}</h2>
          <span className="column-count">{column.cards.length} 枚</span>
          {/* キャプチャ先は印だけにします（#130）。入れ先はアプリ全体で 1 つ
              （`docs/DESIGN.md`「クイックキャプチャ」）なので、盤面のどこかに
              出ていないと `…` を 1 本ずつ開くまで分かりません。文言を常時
              置く代わりに、意味は読み上げ名と説明に持たせます——運んでいるのは
              色ではなく形なので、「色だけに意味を持たせない」にも触れません。 */}
          {/* 終わったものの置き場（ADR 0038）。色だけに意味を持たせないので、
              印と読み上げ名で出します（`docs/DESIGN.md`「画面の作り」）。 */}
          {column.done && (
            <span
              className="column-done"
              role="img"
              aria-label="完了扱い"
              title="終わったものの置き場。ここのカードは期限を数えず、薄く出る"
            >
              ✓
            </span>
          )}
          {captureTarget && (
            <span
              className="column-capture"
              role="img"
              aria-label="クイックキャプチャ先"
              title="クイックキャプチャ先"
            >
              ⚡
            </span>
          )}
          {/* 常用しない操作は `…` に畳む（`docs/DESIGN.md`）。掴むのはヘッダ
              なので、ボタンの上でドラッグが始まらないよう押下を止める。 */}
          <button
            type="button"
            className="ghost column-menu-button"
            aria-label={`${column.name} の操作`}
            aria-expanded={menuOpen}
            onPointerDown={(event) => {
              event.stopPropagation();
            }}
            onClick={() => {
              setMenuOpen((open) => !open);
            }}
          >
            …
          </button>
        </header>
      )}
      {menuOpen && (
        <div className="menu column-menu">
          <button
            type="button"
            className="ghost"
            // 空のカラムにはアーカイブするものが無い。押せてしまうと
            // 「アーカイブするカードがありません」と言う必要が出る。
            disabled={column.cards.length === 0}
            onClick={() => {
              setMenuOpen(false);
              onArchiveColumn(column);
            }}
          >
            アーカイブ
          </button>
          {/* 何本でも立てられます（ADR 0038）。「完了」と「キャンセル済み」を
              並べて両方立てるのが、ボードではなくカラムの属性にした理由です。 */}
          <button
            type="button"
            className="ghost set-column-done"
            onClick={() => {
              setMenuOpen(false);
              void run((document) => setColumnDone(document, column.id, !column.done));
            }}
          >
            {column.done ? "完了扱いをやめる" : "完了扱いにする"}
          </button>
          <button
            type="button"
            className="ghost set-capture-column"
            // すでに入れ先なら押す意味がない。理由は「⚡」が出ていることで分かる。
            disabled={captureTarget}
            onClick={() => {
              setMenuOpen(false);
              onSetCaptureColumn(column.id);
            }}
          >
            クイックキャプチャ先にする
          </button>
          <button
            type="button"
            className="ghost"
            onClick={() => {
              setMenuOpen(false);
              setEditing(true);
            }}
          >
            編集
          </button>
          <button
            type="button"
            className="danger-item"
            disabled={lastColumn}
            onClick={() => {
              setMenuOpen(false);
              onRemoveColumn(column);
            }}
          >
            削除
          </button>
        </div>
      )}
      {/* カードの下の余白も落とし先。空のカラムに入れられなくなるので、
          高さは残す。 */}
      <div className="column-cards">
        {/* 空のカラムでも、そこが落とし先だと分かるようにする。カードが
            1 枚も無いと、掴んだものをどこへ持っていけばよいかが読めない。 */}
        {column.cards.length === 0 && (
          <p className="column-empty">ここにドロップ</p>
        )}
        <SortableContext items={cardIds} strategy={verticalListSortingStrategy}>
          {column.cards.map((card) => (
            <Card
              key={card.id}
              card={card}
              tags={tags}
              due={dueStatuses.get(card.id)}
              today={today}
              activeTag={activeTag}
              onToggleTagFilter={onToggleTagFilter}
              // 終わったものはトーンダウンして描く（ADR 0038）。減光とは
              // 別の表現にする——**濃さ**は絞り込みが使っている。
              done={column.done}
              // 隠さず減光する。隠すと挿入位置が動いてしまう（条件 4）。
              dimmed={matched !== null && !matched.has(card.id)}
              selected={selectedCard === card.id}
              onSelect={onSelectCard}
              onOpen={onOpenCard}
              onContextMenu={onCardContextMenu}
            />
          ))}
        </SortableContext>
      </div>
      <footer className="column-footer">
        <button
          type="button"
          className="ghost add-card"
          onClick={() => {
            onNewCard(column.id);
          }}
        >
          ＋ カードを追加
        </button>
      </footer>
    </section>
  );
}

/// カラム名を直す。ヘッダと入れ替えて出す。
function ColumnEditor({
  column,
  run,
  onDone,
}: {
  column: ColumnData;
  run: (act: (document: BoardDocument) => Outcome<unknown>) => Promise<AppError | null>;
  onDone: () => void;
}) {
  const [name, setName] = useState(column.name);
  const [failed, setFailed] = useState<AppError | null>(null);

  async function save() {
    if (name.trim() === "") return;
    // 変わっていなければ呼びません——同じ値で呼ぶと Undo に空の 1 手が積まれます。
    if (name !== column.name) {
      const failure = await run((document) => renameColumn(document, column.id, name));
      if (failure !== null) {
        setFailed(failure);
        return;
      }
    }
    onDone();
  }

  return (
    <div
      className="column-editor"
      onKeyDown={(event) => {
        if (isComposing(event.nativeEvent)) return;
        // 1 行の欄なので Enter で確定し、Escape で取り消す（`docs/DESIGN.md`）。
        if (event.key === "Enter") {
          event.preventDefault();
          void save();
        } else if (event.key === "Escape") {
          event.stopPropagation();
          onDone();
        }
      }}
    >
      <input
        className="field-input column-name-input"
        value={name}
        placeholder="カラムの名前"
        aria-label="カラムの名前"
        autoFocus
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      {name.trim() === "" && (
        <p className="field-error" role="alert">
          カラム名を入力してください
        </p>
      )}
      {failed?.field === "columnName" && (
        <p className="field-error" role="alert">
          {failed.detail}
        </p>
      )}
      <div className="button-row">
        <button
          type="button"
          className="primary save-column"
          disabled={name.trim() === ""}
          onClick={() => void save()}
        >
          保存
        </button>
        <button type="button" className="secondary" onClick={onDone}>
          取消
        </button>
      </div>
    </div>
  );
}
