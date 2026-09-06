import type { Column as ColumnData } from "../ipc/types/Column";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Tag } from "../ipc/types/Tag";
import { Card } from "./Card";

interface Props {
  column: ColumnData;
  tags: readonly Tag[];
  dueStatuses: ReadonlyMap<number, DueStatus>;
  matched: ReadonlySet<number> | null;
}

export function Column({ column, tags, dueStatuses, matched }: Props) {
  const overLimit = column.wipLimit !== null && column.cards.length > column.wipLimit;

  return (
    <section className="column">
      <header className="column-header">
        <h2 className="column-name">{column.name}</h2>
        <span className="column-count" data-tone={overLimit ? "danger" : undefined}>
          {column.wipLimit === null
            ? `${column.cards.length} 枚`
            : `${column.cards.length} / ${column.wipLimit}`}
          {/* 色だけに意味を持たせない。上限を超えていることは語でも書く。 */}
          {overLimit && <span className="column-over"> 上限超過</span>}
        </span>
      </header>
      <div className="column-cards">
        {column.cards.map((card) => (
          <Card
            key={card.id}
            card={card}
            tags={tags}
            due={dueStatuses.get(card.id)}
            dimmed={matched !== null && !matched.has(card.id)}
          />
        ))}
      </div>
    </section>
  );
}
