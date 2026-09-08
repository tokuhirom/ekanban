// タグの整理パネル。
//
// カードの詳細パネルと同じく右に押し出して置きます。扱うのはボード全体のタグ
// なので、カード 1 枚の話である詳細パネルには混ぜません。
//
// **タグの編集・削除はここだけから行います**（`docs/DESIGN.md`「常用しない
// 操作を画面に常時出さない」）。ヘッダにタグを一覧しないのも同じ理由で、
// 絞り込みはカード上のチップから行います。
//
// 追加はここと、カード編集パネルのタグ欄の 2 か所です（ADR 0027）。あちらは
// 「いま打っているカードに付ける」ためのもので、名前と色を整えるのはここです。
//
// **どちらで作っても、色は自動で付きます**（ADR 0044）。作るときに色を選ばせず、
// 気に入らなければこの一覧の色見本から変える、という順にしてあります。

import { useState } from "react";

import { useIpc } from "../ipc";
import type { AppError } from "../ipc/types/AppError";
import type { Snapshot } from "../ipc/types/Snapshot";
import type { Tag } from "../ipc/types/Tag";
import { useAppActions } from "../shell/actions";
import { isComposing } from "../shell/ime";
import { AUTO_TAG_COLOR, tagColor } from "./tags";

interface Props {
  tags: readonly Tag[];
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
  onClose: () => void;
}

export function TagPanel({ tags, run, onClose }: Props) {
  const ipc = useIpc();
  const [name, setName] = useState("");
  const [failed, setFailed] = useState<AppError | null>(null);

  useAppActions({ cancelEdit: onClose });

  async function add() {
    if (name.trim() === "") return;
    // **送る前に空にします**（#185）。保存の往復を待ってから空にすると、待って
    // いるあいだに打った文字が、返ってきた空への差し替えに巻き込まれて消えます。
    // カード編集パネルのタグ欄も送る前に空にしており（`CardPanel.tsx` の
    // `commit()`）、タグを作れる 2 か所（ADR 0027）はこれで揃います。
    //
    // `Enter` の二重押しで同じタグを 2 回送らないのも、ここで済みます——
    // 2 回目は欄が空なので、上の行で戻ります。
    setName("");
    // 色は渡しません。決めていないタグは自動で色が付きます（ADR 0044）。
    const failure = await run(() => ipc.addTag(name, AUTO_TAG_COLOR));
    setFailed(failure);
    // 断られた名前は打ち直せるように戻します（`docs/DESIGN.md`）。**ただし、
    // 往復のあいだに次の名前が打たれていたら戻しません**——断られた理由は欄の
    // 脇に出ているので、打ちかけを消してまで戻す値打ちがありません。
    if (failure !== null) setName((current) => (current === "" ? name : current));
  }

  return (
    <aside
      className="panel tag-panel"
      aria-label="タグの整理"
      onKeyDown={(event) => {
        // 変換を取り消す Escape でパネルを閉じない（`shell/ime.ts`）。
        if (event.key !== "Escape" || isComposing(event.nativeEvent)) return;
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
            // メニューの「タグを追加」で開いたときに、そのまま打ちはじめられる
            // ようにする。開く道が 1 つしかないので、奪う相手もいない。
            autoFocus
            // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。
            onKeyDown={(event) => {
              if (event.key !== "Enter" || isComposing(event.nativeEvent)) return;
              event.preventDefault();
              void add();
            }}
            onChange={(event) => {
              setName(event.target.value);
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
          if (event.key !== "Enter" || isComposing(event.nativeEvent)) return;
          event.preventDefault();
          void rename();
        }}
        // 焦点が外れたときにも確定する。名前を打ってから別の行へ移った操作を、
        // 打たなかったことにしない。
        onBlur={() => void rename()}
      />
      {/* 色を決めていないタグには、自動の色がそのまま見本として出ます
          （ADR 0044）。ここで動かした瞬間に、その色が「決めた色」になります。 */}
      <input
        type="color"
        className="tag-color-input"
        value={tagColor(tag)}
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
