// 説明の入力欄と、その裏に重ねたリンクの表示層（§4、[ADR 0002]）。
//
// 説明はプレーンテキストのままなので、`textarea` を捨てません。**同じ字送りの
// 表示層を裏に敷き**、そこで URL に色と下線を付けます。入力欄の文字は透明に
// してあり、見えているのは表示層のほうです。
//
// 押した場所がリンクかどうかは、`selectionStart`（クリックで動いたキャレットの
// 位置）で決めます。**当たり判定を自分で持ちません**——文字の折り返しを数え直す
// ことになり、表示層と 1 文字でもずれたら別のリンクが開きます。
//
// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md

import { useEffect, useState } from "react";

import { useIpc } from "../ipc";
import type { Platform } from "../ipc/types/Platform";
import type { UrlSpan } from "../ipc/types/UrlSpan";
import { linkAt, opensLink, segments } from "./description";

interface Props {
  id: string;
  value: string;
  platform: Platform;
  onChange: (value: string) => void;
}

export function Description({ id, value, platform, onChange }: Props) {
  const ipc = useIpc();
  const [links, setLinks] = useState<readonly UrlSpan[]>([]);

  // 打つたびに Rust に聞きます。**見つけ方を 2 か所に持たない**ためで、往復
  // するのは位置の配列だけです（絞り込みと同じ考え方、§5）。返事が 1 打鍵ぶん
  // 遅れても、遅れて色が付くだけです。
  useEffect(() => {
    let cancelled = false;
    ipc
      .descriptionLinks(value)
      .then((found) => {
        if (!cancelled) setLinks(found);
      })
      .catch(() => {
        // 色が付かないだけなので、打つ手を止めない。
        if (!cancelled) setLinks([]);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc, value]);

  const modifier = platform === "macos" ? "Cmd" : "Ctrl";

  return (
    <div className="description-field">
      <div className="description-layer" aria-hidden="true">
        {segments(value, links).map((piece, index) => (
          <span
            // 本文を切った順番がそのまま鍵になる。同じ文字列が並ぶことがあるので
            // 中身は使えない。
            key={index}
            className={piece.url === null ? undefined : "description-link"}
          >
            {piece.text}
          </span>
        ))}
        {/* 末尾の改行だけだと表示層の高さが 1 行ぶん足りない。 */}
        {"\n"}
      </div>
      <textarea
        id={id}
        className="field-input card-description-input"
        value={value}
        placeholder="任意。詳しいことがあれば"
        rows={4}
        title={`${modifier} を押しながらクリックすると、リンクを開きます`}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onClick={(event) => {
          if (!opensLink(event.nativeEvent, platform)) return;
          const link = linkAt(links, event.currentTarget.selectionStart);
          if (link === null) return;
          event.preventDefault();
          void ipc.openUrl(link.url);
        }}
      />
    </div>
  );
}
