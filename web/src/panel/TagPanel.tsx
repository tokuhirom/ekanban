// タグの整理パネル。
//
// カードの詳細パネルと同じく右に押し出して置きます。扱うのはボード全体のタグ
// なので、カード 1 枚の話である詳細パネルには混ぜません。
//
// **タグの追加・編集・削除はここだけから行います**（`docs/DESIGN.md`「常用しない
// 操作を画面に常時出さない」）。ヘッダにタグを一覧しないのも同じ理由で、
// 絞り込みはカード上のチップから行います。

import { useState } from "react";

import { useIpc } from "../ipc";
import type { AppError } from "../ipc/types/AppError";
import type { Snapshot } from "../ipc/types/Snapshot";
import type { Tag } from "../ipc/types/Tag";

/// 新しいタグの既定の色。
///
/// **これは直書きの色ではありません**——ユーザーが決めるまでの初期値で、
/// 決めたあとは `tags.color` がそのまま使われます（直書きが許されるのは
/// ユーザーが指定したタグの色だけ、`docs/DESIGN.md`）。
const DEFAULT_TAG_COLOR = "#94a3b8";

interface Props {
  tags: readonly Tag[];
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
  onClose: () => void;
}

export function TagPanel({ tags, run, onClose }: Props) {
  const ipc = useIpc();
  const [name, setName] = useState("");
  const [color, setColor] = useState(DEFAULT_TAG_COLOR);
  const [failed, setFailed] = useState<AppError | null>(null);

  async function add() {
    if (name.trim() === "") return;
    const failure = await run(() => ipc.addTag(name, color));
    setFailed(failure);
    if (failure === null) {
      setName("");
      setColor(DEFAULT_TAG_COLOR);
    }
  }

  return (
    <aside
      className="panel tag-panel"
      aria-label="タグの整理"
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.stopPropagation();
        onClose();
      }}
    >
      <header className="panel-header">
        <div className="panel-heading">
          <span className="panel-context">このボード</span>
          <strong className="panel-title">タグの整理</strong>
        </div>
        <div className="panel-actions">
          <button type="button" className="ghost" aria-label="閉じる" onClick={onClose}>
            ✕
          </button>
        </div>
      </header>

      <div className="panel-body">
        {tags.length === 0 && <p className="field-note">タグはまだありません。</p>}
        {tags.map((tag) => (
          <TagRow key={tag.id} tag={tag} run={run} />
        ))}

        <span className="field-label">タグを追加</span>
        <div className="tag-row">
          <input
            className="field-input tag-name-input"
            value={name}
            placeholder="タグの名前"
            aria-label="新しいタグの名前"
            // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。
            onKeyDown={(event) => {
              if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
              event.preventDefault();
              void add();
            }}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
          <input
            type="color"
            className="tag-color-input"
            value={color}
            aria-label="新しいタグの色"
            onChange={(event) => {
              setColor(event.target.value);
            }}
          />
          <button
            type="button"
            className="primary add-tag"
            disabled={name.trim() === ""}
            onClick={() => void add()}
          >
            追加
          </button>
        </div>
        {failed?.field === "tagName" && (
          <p className="field-error" role="alert">
            {failed.detail}
          </p>
        )}
      </div>
    </aside>
  );
}

/// 1 つのタグの行。名前と色をその場で直し、削除もここから。
function TagRow({
  tag,
  run,
}: {
  tag: Tag;
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
}) {
  const ipc = useIpc();
  const [name, setName] = useState(tag.name);
  const [failed, setFailed] = useState<AppError | null>(null);

  async function rename() {
    if (name.trim() === "" || name === tag.name) return;
    setFailed(await run(() => ipc.renameTag(tag.id, name)));
  }

  return (
    <div className="tag-row" data-tag={tag.id}>
      <input
        className="field-input tag-name-input"
        value={name}
        aria-label={`${tag.name} の名前`}
        onChange={(event) => {
          setName(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
          event.preventDefault();
          void rename();
        }}
        // 焦点が外れたときにも確定する。名前を打ってから別の行へ移った操作を、
        // 打たなかったことにしない。
        onBlur={() => void rename()}
      />
      <input
        type="color"
        className="tag-color-input"
        value={tag.color}
        aria-label={`${tag.name} の色`}
        onChange={(event) => {
          void run(() => ipc.setTagColor(tag.id, event.target.value));
        }}
      />
      {/* タグを消してもカードは残る（付いていたタグが外れるだけ）ので、Undo で
          戻せます。確認は出しません（`docs/DESIGN.md`）。 */}
      <button
        type="button"
        className="danger-item remove-tag"
        aria-label={`${tag.name} を削除`}
        onClick={() => {
          void run(() => ipc.removeTag(tag.id));
        }}
      >
        削除
      </button>
      {failed?.field === "tagName" && (
        <p className="field-error" role="alert">
          {failed.detail}
        </p>
      )}
    </div>
  );
}
