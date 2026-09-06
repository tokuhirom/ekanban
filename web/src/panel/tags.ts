// タグの上の純粋な操作。カード編集パネルとタグ整理パネルの両方が使います。
//
// ここに盤面の論理は入りません。タグを作る・名前を変える・消すのはどれも Rust の
// コマンドで、ここにあるのは「打った名前が既にあるタグか」「候補に何を出すか」の
// 2 つだけです。

import type { Tag } from "../ipc/types/Tag";

/// 新しいタグの既定の色。
///
/// **これは直書きの色ではありません**——ユーザーが決めるまでの初期値で、
/// 決めたあとは `tags.color` がそのまま使われます（直書きが許されるのは
/// ユーザーが指定したタグの色だけ、`docs/DESIGN.md`）。
export const DEFAULT_TAG_COLOR = "#94a3b8";

/// 打った名前を、既にあるタグに突き合わせる。
///
/// 前後の空白と大文字小文字は無視します。「Rust」と打ったときに「rust」が
/// あるなら、同じ名前のタグをもう 1 つ作らずにそれを選ぶためです。
export function findTagByName(tags: readonly Tag[], name: string): Tag | null {
  const wanted = name.trim().toLocaleLowerCase();
  if (wanted === "") return null;
  return tags.find((tag) => tag.name.trim().toLocaleLowerCase() === wanted) ?? null;
}

/// 候補に出すタグ。
///
/// 既に選んであるものは出しません（付いていることはチップで見えている）。
/// 打ちかけの文字があれば、それを含むものだけに絞ります。
export function suggestTags(
  tags: readonly Tag[],
  selected: readonly number[],
  typed: string,
): Tag[] {
  const needle = typed.trim().toLocaleLowerCase();
  return tags.filter(
    (tag) =>
      !selected.includes(tag.id) &&
      (needle === "" || tag.name.toLocaleLowerCase().includes(needle)),
  );
}
