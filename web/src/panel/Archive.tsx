// アーカイブ表示（[ADR 0010]）。
//
// 盤面の代わりに、アーカイブしたカードを日ごとに並べます。**絞り込みから外れた
// カードは隠します**——盤面では減光しますが、それは落とす位置を動かさないため
// で、ここには落とす位置がありません。
//
// [ADR 0010]: ../../../docs/adr/0010-hiding-instead-of-dimming-in-the-archive.md

import { CardFace } from "../board/Card";
import type { Board } from "../ipc/types/Board";
import type { DueStatus } from "../ipc/types/DueStatus";
import { archivedGroups } from "./archived";

interface Props {
  board: Board;
  dueStatuses: ReadonlyMap<number, DueStatus>;
  /** 絞り込みに一致したカード。`null` は「絞り込んでいない」。 */
  matched: ReadonlySet<number> | null;
  onRestore: (cardId: number) => void;
}

export function Archive({ board, dueStatuses, matched, onRestore }: Props) {
  const groups = archivedGroups(board.archivedCards, matched);
  const shown = groups.reduce((count, group) => count + group.cards.length, 0);

  return (
    <div className="archive" aria-label="アーカイブ">
      {board.archivedCards.length === 0 && (
        <p className="field-note">アーカイブ済みのカードはありません</p>
      )}
      {board.archivedCards.length > 0 && shown === 0 && (
        <p className="field-note">絞り込みに一致するカードはありません</p>
      )}
      {groups.map((group) => (
        <section className="archive-group" key={group.label}>
          <header className="archive-day">
            <h2 className="archive-day-label">{group.label}</h2>
            <span className="archive-day-count">{group.cards.length} 件</span>
          </header>
          {group.cards.map((card) => (
            <article className="card archived-card" key={card.id} data-card={card.id}>
              <div className="archived-card-body">
                <CardFace card={card} tags={board.tags} due={dueStatuses.get(card.id)} />
              </div>
              <button
                type="button"
                className="primary restore-card"
                onClick={() => {
                  onRestore(card.id);
                }}
              >
                復元
              </button>
            </article>
          ))}
        </section>
      ))}
    </div>
  );
}
