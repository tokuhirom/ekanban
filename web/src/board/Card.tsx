import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

import type { Card as CardData } from "../ipc/types/Card";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Tag } from "../ipc/types/Tag";
import { handleId } from "./dnd";

/// 「9/4」。年をまたぐものだけ年を出す。カードの面は狭いので、いまの年は落とす。
export function shortDate(due: string): string {
  const [year = "", month = "", day = ""] = due.split("-");
  const thisYear = String(new Date().getFullYear());
  const short = `${Number(month)}/${Number(day)}`;
  return year === thisYear ? short : `${year}/${month}/${day}`;
}

/// 期限の見出し。色も文言も `DueStatus` から作る。今日を基準にした判定は
/// Rust の `due_statuses` が済ませてあるので、ここでは時計を見ない。
///
/// **`*-foreground` を素の面の文字色に使わない。** あれは対応する背景の上に
/// 載せるための色で、カードの面では背景と同化して読めない（`docs/DESIGN.md`）。
/// ここでは背景用の `danger` / `warning` / `info` のほうを文字色に使う。
export function dueBadge(status: DueStatus, due: string): { tone: string; text: string } | null {
  switch (status.kind) {
    case "overdue":
      return { tone: "danger", text: `期限切れ ${status.days}日 (${shortDate(due)})` };
    case "today":
      return { tone: "warning", text: `期限 今日 (${shortDate(due)})` };
    case "soon":
      return { tone: "info", text: `期限 あと ${status.days}日 (${shortDate(due)})` };
    case "upcoming":
      return { tone: "muted", text: `期限 ${shortDate(due)}` };
    case "none":
      return null;
  }
}

interface FaceProps {
  card: CardData;
  tags: readonly Tag[];
  due: DueStatus | undefined;
  /** 絞り込んでいるタグ。押されているチップに印を付けるのに使う。 */
  activeTag?: number | null | undefined;
  /** タグのチップが押された。ゴースト（`DragOverlay`）では渡さない。 */
  onToggleTagFilter?: ((tagId: number) => void) | undefined;
}

/// カードの表面。ゴースト（`DragOverlay`）も同じものを描くので、掴んだ瞬間に
/// 見た目が変わりません。
///
/// **高さを中身で変えすぎない**規則は残します。落とす位置が見て分かること
/// （`docs/DESIGN.md`「ドラッグ＆ドロップ」の受け入れ条件）は、掴んでいる間に周りの高さが動かないことで決まります。
export function CardFace({ card, tags, due, activeTag, onToggleTagFilter }: FaceProps) {
  const cardTags = card.tagIds
    .map((id) => tags.find((tag) => tag.id === id))
    .filter((tag): tag is Tag => tag !== undefined);
  const badge = due !== undefined && card.dueDate !== null ? dueBadge(due, card.dueDate) : null;
  const checked = card.checklistItems.filter((item) => item.checked).length;
  const progress = card.checklistItems.map((item) => (item.checked ? "■" : "□")).join("");

  return (
    <>
      <div className="card-title">{card.title}</div>
      {badge !== null && (
        <div className="card-due" data-tone={badge.tone}>
          {badge.text}
        </div>
      )}
      {card.checklistItems.length > 0 && (
        <div className="card-checklist">
          {progress} {checked}/{card.checklistItems.length}
        </div>
      )}
      {cardTags.length > 0 && (
        <div className="card-tags">
          {cardTags.map((tag) => {
            const active = activeTag === tag.id;
            // タグの色はユーザーが決めたもの。直書きの色が許されるのは
            // ここだけ（`docs/DESIGN.md`）。
            const chip = (
              <>
                {/* 絞り込み中であることを、色だけでなく印でも出す
                    （`docs/DESIGN.md`「画面の作り」）。 */}
                {active ? "✓ " : ""}
                {tag.name}
              </>
            );
            if (onToggleTagFilter === undefined) {
              return (
                <span key={tag.id} className="tag-chip" style={{ background: tag.color }}>
                  {chip}
                </span>
              );
            }
            return (
              <button
                key={tag.id}
                type="button"
                className="tag-chip"
                style={{ background: tag.color }}
                aria-pressed={active}
                title={active ? `${tag.name} の絞り込みを解除` : `${tag.name} で絞り込む`}
                // カードの選択とドラッグに取られないようにする。押した先は
                // 絞り込みで、カードを掴む操作ではない。
                onPointerDown={(event) => {
                  event.stopPropagation();
                }}
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleTagFilter(tag.id);
                }}
                onDoubleClick={(event) => {
                  event.stopPropagation();
                }}
              >
                {chip}
              </button>
            );
          })}
        </div>
      )}
      {card.description.trim() !== "" && <div className="card-description">{card.description}</div>}
    </>
  );
}

interface Props extends FaceProps {
  /** 絞り込みに外れている。隠さず減光する（D&D の挿入位置を動かさないため）。 */
  dimmed: boolean;
  selected: boolean;
  onSelect: (cardId: number) => void;
  /** 編集パネルを開く。 */
  onOpen: (cardId: number) => void;
  /** 右クリックメニューを、画面の座標で開く。描くのは `Board`。 */
  onContextMenu: (cardId: number, at: { x: number; y: number }) => void;
}

export function Card({
  card,
  tags,
  due,
  activeTag,
  onToggleTagFilter,
  dimmed,
  selected,
  onSelect,
  onOpen,
  onContextMenu,
}: Props) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: handleId({ kind: "card", id: card.id }),
  });

  return (
    <article
      ref={setNodeRef}
      className="card"
      data-card={card.id}
      data-dimmed={dimmed || undefined}
      data-selected={selected || undefined}
      // 掴んでいる間、元の場所は空きとして残す。周りが詰まってしまうと、
      // どこに戻るのかが読めなくなる（条件 2）。
      data-placeholder={isDragging || undefined}
      style={{ transform: CSS.Translate.toString(transform), transition: transition ?? undefined }}
      {...attributes}
      {...listeners}
      // dnd-kit の `listeners` にも `onPointerDown` がある。React は**あとに
      // 置いたほうを採る**ので、選択を先に書くと掴む処理に上書きされ、逆に
      // 書くと掴めなくなる。両方いるので、ここで順に呼ぶ。
      onPointerDown={(event) => {
        onSelect(card.id);
        listeners?.onPointerDown?.(event);
      }}
      // **1 回のクリックでは開きません。** クリックは選ぶ操作で、そこから
      // ドラッグも始まります（`activationConstraint` の 4px）。開くたびに
      // パネルが出ると、掴もうとしただけで画面が動きます。開くのは
      // ダブルクリックか、選んだうえでの Enter。
      onDoubleClick={() => {
        onOpen(card.id);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        onSelect(card.id);
        onContextMenu(card.id, { x: event.clientX, y: event.clientY });
      }}
    >
      <CardFace
        card={card}
        tags={tags}
        due={due}
        activeTag={activeTag}
        onToggleTagFilter={onToggleTagFilter}
      />
    </article>
  );
}

export interface MenuProps {
  card: CardData;
  tags: readonly Tag[];
  at: { x: number; y: number };
  onClose: () => void;
  onCopy: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onToggleTag: (tagId: number) => void;
}

/// カードの右クリックメニュー。
///
/// **カードの操作はここに集約します**（`docs/DESIGN.md`「常用しない操作を画面に
/// 常時出さない」）。webview では既定の右クリックメニューが先に出るので、
/// `shell/harden.ts` がそれを止めています（`docs/DESIGN.md`「画面の作り」）。
///
/// **カードの中には描きません。** カードは dnd-kit の `transform` を持つことが
/// あり、`transform` を持つ要素は `position: fixed` の基準になります。画面の
/// 座標で置いたメニューが、掴んだ量だけずれることになるので、`Board` が盤面の
/// 外側で描きます。
export function CardMenu({ card, tags, at, onClose, onCopy, onArchive, onDelete, onToggleTag }: MenuProps) {
  return (
    <div
      className="menu card-menu"
      style={{ left: at.x, top: at.y }}
      // 中を押しても閉じないようにする。閉じるのは項目を選んだときと、
      // 外を押したとき（`.menu-scrim`）。
      onPointerDown={(event) => {
        event.stopPropagation();
      }}
    >
      <button
        type="button"
        className="ghost"
        onClick={() => {
          onClose();
          onCopy();
        }}
      >
        コピー
      </button>
      <span className="menu-label">タグ</span>
      {tags.map((tag) => (
        <button
          key={tag.id}
          type="button"
          className="ghost"
          onClick={() => {
            onClose();
            onToggleTag(tag.id);
          }}
        >
          {/* 色だけに意味を持たせない。付いているかどうかは印で書く。 */}
          {card.tagIds.includes(tag.id) ? "✓ " : "□ "}
          {tag.name}
        </button>
      ))}
      <button
        type="button"
        className="ghost"
        onClick={() => {
          onClose();
          onArchive();
        }}
      >
        アーカイブ
      </button>
      <button
        type="button"
        className="danger-item"
        onClick={() => {
          onClose();
          onDelete();
        }}
      >
        削除
      </button>
    </div>
  );
}
