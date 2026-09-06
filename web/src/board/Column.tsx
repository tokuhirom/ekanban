import { useSortable } from "@dnd-kit/sortable";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

import type { Column as ColumnData } from "../ipc/types/Column";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Tag } from "../ipc/types/Tag";
import { Card } from "./Card";
import { handleId } from "./dnd";

interface Props {
  column: ColumnData;
  tags: readonly Tag[];
  dueStatuses: ReadonlyMap<number, DueStatus>;
  matched: ReadonlySet<number> | null;
  selectedCard: number | null;
  onSelectCard: (cardId: number) => void;
}

/// カラムの中身。ヘッダを掴むとカラムごと動き、カードを掴むとカードが動く。
///
/// 掴む場所を分けているのは、カードの上でカラムのドラッグが始まると、
/// 1 枚動かすつもりが列ごと動くからです。
export function Column({
  column,
  tags,
  dueStatuses,
  matched,
  selectedCard,
  onSelectCard,
}: Props) {
  const overLimit = column.wipLimit !== null && column.cards.length > column.wipLimit;
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } =
    useSortable({ id: handleId({ kind: "column", id: column.id }) });
  const cardIds = column.cards.map((card) => handleId({ kind: "card", id: card.id }));

  return (
    <section
      ref={setNodeRef}
      className="column"
      data-placeholder={isDragging || undefined}
      style={{ transform: CSS.Translate.toString(transform), transition: transition ?? undefined }}
    >
      <header
        className="column-header"
        ref={setActivatorNodeRef}
        title="掴んでカラムを並べ替える"
        {...attributes}
        {...listeners}
      >
        <h2 className="column-name">{column.name}</h2>
        <span className="column-count" data-tone={overLimit ? "danger" : undefined}>
          {column.wipLimit === null
            ? `${column.cards.length} 枚`
            : `${column.cards.length} / ${column.wipLimit}`}
          {/* 色だけに意味を持たせない。上限を超えていることは語でも書く。 */}
          {overLimit && <span className="column-over"> 上限超過</span>}
        </span>
      </header>
      {/* カードの下の余白も落とし先。空のカラムに入れられなくなるので、
          高さは残す。 */}
      <div className="column-cards">
        <SortableContext items={cardIds} strategy={verticalListSortingStrategy}>
          {column.cards.map((card) => (
            <Card
              key={card.id}
              card={card}
              tags={tags}
              due={dueStatuses.get(card.id)}
              // 隠さず減光する。隠すと挿入位置が動いてしまう（条件 4）。
              dimmed={matched !== null && !matched.has(card.id)}
              selected={selectedCard === card.id}
              onSelect={onSelectCard}
            />
          ))}
        </SortableContext>
      </div>
    </section>
  );
}
