import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

import type { Card as CardData } from "../ipc/types/Card";
import type { DueStatus } from "../model/due";
import type { Tag } from "../ipc/types/Tag";
import { tagChipStyle } from "../panel/tags";
import { handleId } from "./dnd";
import { dueChoices } from "./due";

/// 「9/4」。年をまたぐものだけ年を出す。カードの面は狭いので、いまの年は落とす。
///
/// **今年かどうかは `today` で決めます**（#133）。ブラウザの時計を読むと、
/// 年末年始に Rust の判定と食い違います（`docs/DESIGN.md`「絞り込みと検索」）。
export function shortDate(due: string, today: string): string {
  const [year = "", month = "", day = ""] = due.split("-");
  const short = `${Number(month)}/${Number(day)}`;
  return year === today.slice(0, 4) ? short : `${year}/${month}/${day}`;
}

/// 期限の見出し。色も文言も `DueStatus` から作る。今日を基準にした判定は
/// Rust の `due_statuses` が済ませてあるので、ここでは時計を見ない。
///
/// **4 つの状態を同じ形で書きます**（#133）——「印 + 日付 + 補足」。語順が
/// 状態ごとに変わると、読み手が毎回読み方を切り替えることになります。印は
/// ボード一覧（`Sidebar.tsx`）の `⚠` `◷` と同じもので、盤面と一覧で読み方を
/// 変えません。先のものほど補足を落とし、日付だけにします。
///
/// **`*-foreground` を素の面の文字色に使わない。** あれは対応する背景の上に
/// 載せるための色で、カードの面では背景と同化して読めない（`docs/DESIGN.md`）。
/// ここでは背景用の `danger` / `warning` / `info` のほうを文字色に使う。
export function dueBadge(
  status: DueStatus,
  due: string,
  today: string,
  done = false,
): { tone: string; text: string } | null {
  // 終わったものの置き場では日付だけにします（ADR 0038）。急かす印（`⚠` `◷`）と
  // 「N日超過」は、もう手を動かさないカードでは意味を失い、他の赤の重さまで
  // 下げます。日付を消さないのは、いつまでのものだったかが記録として要るから。
  if (done) {
    return status.kind === "none"
      ? null
      : { tone: "muted", text: shortDate(due, today) };
  }
  switch (status.kind) {
    case "overdue":
      return {
        tone: "danger",
        text: `⚠ ${shortDate(due, today)}（${status.days}日超過）`,
      };
    // 今日の日付はカードを見なくても分かるので、いちばん短い形にする。
    case "today":
      return { tone: "warning", text: "◷ 今日" };
    case "soon":
      return {
        tone: "info",
        text: `${shortDate(due, today)}（あと${status.days}日）`,
      };
    case "upcoming":
      return { tone: "muted", text: shortDate(due, today) };
    case "none":
      return null;
  }
}

/// カード表面に出すチェックリストの進み具合（#140）。
///
/// 項目が無ければ何も出しません。割合は丸めません——1/30 でも「少し進んで
/// いる」ことが見えるほうが役に立ちます。
export function checklistProgress(
  items: readonly { checked: boolean }[],
): { checked: number; total: number; ratio: number; done: boolean } | null {
  if (items.length === 0) return null;
  const checked = items.filter((item) => item.checked).length;
  return {
    checked,
    total: items.length,
    ratio: checked / items.length,
    done: checked === items.length,
  };
}

interface FaceProps {
  card: CardData;
  tags: readonly Tag[];
  due: DueStatus | undefined;
  /** 終わったものの置き場にあるか（ADR 0038）。トーンダウンして描く。 */
  done?: boolean;
  /** 繰り返しが出したカードで、**その定義がまだ残っている**（#198）。
   *
   * 出すのは、勝手に片付くことの説明になるからです。定義が消えたあとの
   * カードには出しません——説明する相手がもういません。 */
  recurring?: boolean;
  /** `due_statuses` を出した日。年を出すかどうかをここから決める（時計ではなく）。 */
  today: string;
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
export function CardFace({
  card,
  tags,
  due,
  done = false,
  recurring = false,
  today,
  activeTag,
  onToggleTagFilter,
}: FaceProps) {
  const cardTags = card.tagIds
    .map((id) => tags.find((tag) => tag.id === id))
    .filter((tag): tag is Tag => tag !== undefined);
  const badge =
    due !== undefined && card.dueDate !== null
      ? dueBadge(due, card.dueDate, today, done)
      : null;
  const progress = checklistProgress(card.checklistItems);

  return (
    <>
      {/* しるしはタイトルの行に置きます。行を増やすとカードの高さが中身で
          変わり、落とす位置の判定が動きます（`docs/DESIGN.md`「画面の作り」）。
          色だけに意味を持たせないので、読み上げにも名前を付けます。 */}
      <div className="card-title">
        {recurring && (
          <span className="card-recurring" role="img" aria-label="繰り返し">
            🔁
          </span>
        )}
        {card.title}
      </div>
      {badge !== null && (
        <div className="card-due" data-tone={badge.tone}>
          {badge.text}
        </div>
      )}
      {/* 進捗はバーと数で出します（#140）。記号を項目数だけ並べていたころは、
          項目が増えるとカードの高さが変わっていました（`docs/DESIGN.md`
          「ドラッグ＆ドロップ」）。バーの幅は項目数によらず一定です。
          色だけに意味を持たせないので、終わったものには `✓` を付けます。 */}
      {progress !== null && (
        <div
          className="card-checklist"
          data-done={progress.done || undefined}
          role="img"
          aria-label={`チェックリスト ${String(progress.checked)}/${String(progress.total)}`}
        >
          <span className="card-progress">
            <span
              className="card-progress-fill"
              style={{ width: `${String(progress.ratio * 100)}%` }}
            />
          </span>
          {progress.done && "✓ "}
          {progress.checked}/{progress.total}
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
                <span
                  key={tag.id}
                  className="tag-chip"
                  style={tagChipStyle(tag)}
                >
                  {chip}
                </span>
              );
            }
            return (
              <button
                key={tag.id}
                type="button"
                className="tag-chip"
                style={tagChipStyle(tag)}
                aria-pressed={active}
                title={
                  active
                    ? `${tag.name} の絞り込みを解除`
                    : `${tag.name} で絞り込む`
                }
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
      {card.description.trim() !== "" && (
        <div className="card-description">{card.description}</div>
      )}
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
  done = false,
  recurring = false,
  today,
  activeTag,
  onToggleTagFilter,
  dimmed,
  selected,
  onSelect,
  onOpen,
  onContextMenu,
}: Props) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: handleId({ kind: "card", id: card.id }),
  });

  return (
    <article
      ref={setNodeRef}
      className="card"
      data-card={card.id}
      // 終わったものは**色**で、絞り込みは**濃さ**で沈めます（ADR 0038）。
      // 同じ表現手段を 2 つの意味に使うと、暗い理由が読めなくなります。
      data-done={done || undefined}
      data-dimmed={dimmed || undefined}
      data-selected={selected || undefined}
      // 掴んでいる間、元の場所は空きとして残す。周りが詰まってしまうと、
      // どこに戻るのかが読めなくなる（条件 2）。
      data-placeholder={isDragging || undefined}
      style={{
        transform: CSS.Translate.toString(transform),
        transition: transition ?? undefined,
      }}
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
        done={done}
        recurring={recurring}
        today={today}
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
  /** `due_statuses` を出した日。期限の候補はここから数える（ブラウザの時計ではなく）。 */
  today: string;
  onClose: () => void;
  onCopy: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onToggleTag: (tagId: number) => void;
  /** 期限をその場で当て外しする（#132）。`null` で期限なし。 */
  onSetDueDate: (dueDate: string | null) => void;
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
export function CardMenu({
  card,
  tags,
  at,
  today,
  onClose,
  onCopy,
  onArchive,
  onDelete,
  onToggleTag,
  onSetDueDate,
}: MenuProps) {
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
      {/* 期限は、パネルを開かずに当て外しできるようにします（#132）。
          当てるのは `setCardDueDate` の 1 操作なので、Undo も 1 手。 */}
      <span className="menu-label">期限</span>
      {dueChoices(today).map((choice) => (
        <button
          key={choice.label}
          type="button"
          className="ghost"
          onClick={() => {
            onClose();
            onSetDueDate(choice.date);
          }}
        >
          {/* 色だけに意味を持たせない。当たっている日付は印で書く。 */}
          {card.dueDate === choice.date ? "✓ " : "□ "}
          {choice.label}
        </button>
      ))}
      <button
        type="button"
        className="ghost"
        onClick={() => {
          onClose();
          onSetDueDate(null);
        }}
      >
        {card.dueDate === null ? "✓ " : "□ "}
        なし
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
