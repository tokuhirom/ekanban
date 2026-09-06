import type { Card as CardData } from "../ipc/types/Card";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Tag } from "../ipc/types/Tag";

/// 「9/4」。年をまたぐものだけ年を出す（gpui 版の `short_date` / `display_date`）。
export function shortDate(due: string): string {
  const [year = "", month = "", day = ""] = due.split("-");
  const thisYear = String(new Date().getFullYear());
  const short = `${Number(month)}/${Number(day)}`;
  return year === thisYear ? short : `${year}/${month}/${day}`;
}

/// 期限の見出し。gpui 版の `render_due_badge` と同じ文言・同じ色の選び方。
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

interface Props {
  card: CardData;
  tags: readonly Tag[];
  due: DueStatus | undefined;
  /** 絞り込みに外れている。隠さず減光する（D&D の挿入位置を動かさないため）。 */
  dimmed: boolean;
}

export function Card({ card, tags, due, dimmed }: Props) {
  const cardTags = card.tagIds
    .map((id) => tags.find((tag) => tag.id === id))
    .filter((tag): tag is Tag => tag !== undefined);
  const badge = due !== undefined && card.dueDate !== null ? dueBadge(due, card.dueDate) : null;
  const checked = card.checklistItems.filter((item) => item.checked).length;
  const progress = card.checklistItems.map((item) => (item.checked ? "■" : "□")).join("");

  return (
    <article className="card" data-dimmed={dimmed || undefined}>
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
          {cardTags.map((tag) => (
            // タグの色はユーザーが決めたもの。直書きの色が許されるのは
            // ここだけ（`docs/DESIGN.md`）。
            <span key={tag.id} className="tag-chip" style={{ background: tag.color }}>
              {tag.name}
            </span>
          ))}
        </div>
      )}
      {card.description.trim() !== "" && <div className="card-description">{card.description}</div>}
    </article>
  );
}
