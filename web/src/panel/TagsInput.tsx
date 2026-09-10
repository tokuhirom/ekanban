// タグを付け外しする欄。**カードの編集パネルと繰り返しの設定パネルで同じもの
// を使います**（#115、[ADR 0027]）。
//
// 以前は繰り返しのほうだけがボードのタグを全部並べた押しボタンで、同じ「タグを
// 付ける」がカードと別の見た目・別の操作になっていました。付けるものが同じなら
// 打ち方も同じであるべきで、部品を 1 つに寄せてあります——そのタグが無ければ
// その場で作れることも、両方で同じように使えます。
//
// **打った名前が既にあるタグならそれを選び、無ければ作って選びます。** 大文字
// 小文字と前後の空白は無視して突き合わせるので、同じ名前のタグが 2 つできる
// ことはありません。作るところまでをここに置くのは、タグ整理パネルを開いて
// 戻ってくる往復が、書いている最中には重すぎるからです。名前の変更・色・削除は
// 今までどおりタグ整理パネルにしか置きません。
//
// チップの `✕` は「これから外す」で、タグそのものは残ります。
//
// [ADR 0027]: ../../../docs/adr/0027-creating-tags-while-editing-a-card.md

import { useState } from "react";

import type { AppError } from "../ipc/types/AppError";
import type { Tag } from "../ipc/types/Tag";
import { isComposing } from "../shell/ime";
import { findTagByName, suggestTags, tagChipStyle } from "./tags";

export function TagsInput({
  tags,
  selected,
  failure,
  context,
  onToggle,
  onCreate,
}: {
  tags: readonly Tag[];
  selected: readonly number[];
  failure: AppError | null;
  /**
   * 読み上げの名前に付ける前置き。**同じ画面にこの欄が 2 つ以上あるとき**に
   * 渡します（繰り返しの一覧は定義の数だけ並びます）。カードの編集パネルの
   * ように 1 つしか無いところでは要りません。
   */
  context?: string;
  onToggle: (tagId: number) => void;
  onCreate: (name: string) => void | Promise<void>;
}) {
  const [typed, setTyped] = useState("");
  const chips = selected
    .map((tagId) => tags.find((tag) => tag.id === tagId))
    .filter((tag): tag is Tag => tag !== undefined);
  const suggestions = suggestTags(tags, selected, typed);
  const prefix = context === undefined ? "" : `${context} の`;

  /// 打った名前を確定する。既にあれば選ぶだけ、無ければ作る。
  function commit() {
    const name = typed.trim();
    if (name === "") return;
    const existing = findTagByName(tags, name);
    setTyped("");
    if (existing !== null) {
      if (!selected.includes(existing.id)) onToggle(existing.id);
      return;
    }
    void onCreate(name);
  }

  return (
    <>
      <div className="tags-input">
        {chips.map((tag) => (
          <span
            key={tag.id}
            className="tag-chip tags-input-chip"
            style={tagChipStyle(tag)}
          >
            {tag.name}
            <button
              type="button"
              className="tags-input-remove"
              aria-label={`${prefix}${tag.name} を外す`}
              onClick={() => {
                onToggle(tag.id);
              }}
            >
              ✕
            </button>
          </span>
        ))}
        <input
          className="tags-input-field"
          value={typed}
          placeholder={
            chips.length === 0 ? "タグを打って Enter（無ければ作ります）" : ""
          }
          // 見出しを出さなくなったので、名前はここで持ちます（#144）。
          aria-label={`${prefix}タグ`}
          onChange={(event) => {
            setTyped(event.target.value);
          }}
          onKeyDown={(event) => {
            // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。IME の変換を
            // 確定する Enter でタグを作らないよう `shell/ime.ts` を通す。
            if (event.key === "Enter" && !isComposing(event.nativeEvent)) {
              event.preventDefault();
              commit();
              return;
            }
            // 空の欄での Backspace は末尾のチップを外す。打ち間違えたタグを、
            // チップまでポインタを運ばずに取り消せるようにする。
            const last = chips[chips.length - 1];
            if (
              event.key === "Backspace" &&
              typed === "" &&
              last !== undefined
            ) {
              event.preventDefault();
              onToggle(last.id);
            }
          }}
        />
      </div>
      {failure?.field === "tagName" && (
        <p className="field-error" role="alert">
          {failure.detail}
        </p>
      )}
      {/* 候補は打っているあいだだけ出す（#169）。開いた時点でまだ付けていない
          タグを全部並べると、ボードのタグが増えるほど 1 枚の編集画面が埋まる。
          どんなタグがあるかを一覧するのはタグ整理パネルの仕事。 */}
      {suggestions.length > 0 && (
        <div className="button-row tag-suggestions">
          {suggestions.map((tag) => (
            <button
              key={tag.id}
              type="button"
              className="secondary"
              onClick={() => {
                setTyped("");
                onToggle(tag.id);
              }}
            >
              {tag.name}
            </button>
          ))}
        </div>
      )}
    </>
  );
}
