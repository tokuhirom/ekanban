// タグの上の純粋な操作。カード編集パネルとタグ整理パネルの両方が使います。
//
// ここに盤面の論理は入りません。タグを作る・名前を変える・消すのはどれも Rust の
// コマンドで、ここにあるのは「打った名前が既にあるタグか」「候補に何を出すか」、
// そして「そのタグを何色で描くか」の 3 つだけです。

import type { Tag } from "../ipc/types/Tag";

/// 自動で振り分けるタグの色（`docs/DESIGN.md`「画面の作り」、ADR 0044）。
///
/// **色を決めていないタグは、この並びから順に色を取ります。** 以前は新しいタグが
/// 全部同じ既定色になり、色で見分けたければタグ整理パネルを開いて 1 つずつ塗る
/// ことになっていました（ADR 0027 が対価として引き受けていた不都合です）。
///
/// 隣り合う ID が似た色にならないよう、**色相を飛ばして並べて**あります。タグは
/// 作った順に ID が増えるので、続けて作った 2 つは必ず離れた色になります。
///
/// ここは `#rrggbb` の直書きが許される場所です（`docs/DESIGN.md`）。画面のほかの
/// 色と違い、タグの色は**そのタグを見分けるためのもの**で、テーマの一部では
/// ないためです。
///
/// **どれも明るいほうに寄せてあります。** チップはライトでもダークでも同じ色で
/// 描くので（テーマで塗り替えると、同じタグが 2 つの顔を持つことになります）、
/// どちらの地の上でも浮くほうに揃えるほかありません。明るい地の上に濃い文字が
/// 4.5:1 以上で載ることは `tags.test.ts` が数えています。
export const TAG_PALETTE = [
  "#f87171", // 赤
  "#38bdf8", // 空
  "#facc15", // 黄
  "#c084fc", // 紫
  "#4ade80", // 緑
  "#f472b6", // 桃
  "#2dd4bf", // 青緑
  "#fb923c", // 橙
  "#818cf8", // 藍
  "#94a3b8", // 灰
] as const;

/// タグの色を決めていないことを表す値。
///
/// 空文字です。**「色を決めていない」を別のフィールドにしないのは、色が文字列
/// 1 つで足りるからです**——保存の形（`crates/core`）も TypeScript の型も
/// 変えずに済みます。ユーザーがタグ整理パネルで色を決めると、そのときはじめて
/// `#rrggbb` が入ります。
export const AUTO_TAG_COLOR = "";

/// このタグを描く色。
///
/// ユーザーが決めた色があればそれを、無ければ ID から自動で割り当てます。
/// **ID から引くので、ほかのタグを消しても色は動きません。** 溜まった順に
/// 並べ直す作りだと、タグを 1 つ消しただけで残り全部の色が変わります。
///
/// パレットより多くのタグを作れば色は一巡します。見分けるための色であって、
/// タグの識別子ではないので、そこは許します（名前は常にチップに出ています。
/// `docs/DESIGN.md`「色だけに意味を持たせない」）。
export function tagColor(tag: Tag): string {
  if (tag.color.trim() !== "") return tag.color;
  const length = TAG_PALETTE.length;
  const index = (((tag.id - 1) % length) + length) % length;
  return TAG_PALETTE[index] ?? TAG_PALETTE[0];
}

/// チップの背景に載せる文字色。
///
/// **背景から決めます。** 素の `--color-foreground` のままだと、ライトテーマの
/// 濃い文字が濃い色のチップに沈み、ダークテーマの淡い文字が淡い色のチップに
/// 溶けます。色はユーザーも自由に決められるので、どちらも実際に起きます。
///
/// 濃いほうと淡いほうを当てて、**WCAG のコントラスト比が大きいほうを選びます**。
/// 明るさの境目を 1 つ決め打ちにすると、境目のすぐ両側でどちらを選んでも読めない
/// 色が残ります。ここも `#rrggbb` の直書きが許されます——タグの色の上に載せる
/// ための文字色で、テーマではなくその色から決まるためです。
export function tagTextColor(color: string): string {
  const background = relativeLuminance(color);
  if (background === null) return CHIP_TEXT_DARK;
  return contrast(background, CHIP_TEXT_DARK_LUMINANCE) >=
    contrast(background, CHIP_TEXT_LIGHT_LUMINANCE)
    ? CHIP_TEXT_DARK
    : CHIP_TEXT_LIGHT;
}

const CHIP_TEXT_DARK = "#0f172a";
const CHIP_TEXT_LIGHT = "#f8fafc";
const CHIP_TEXT_DARK_LUMINANCE = relativeLuminance(CHIP_TEXT_DARK) ?? 0;
const CHIP_TEXT_LIGHT_LUMINANCE = relativeLuminance(CHIP_TEXT_LIGHT) ?? 1;

/// チップに当てる style。背景と、その上で読める文字色。
export function tagChipStyle(tag: Tag): { background: string; color: string } {
  const background = tagColor(tag);
  return { background, color: tagTextColor(background) };
}

/// WCAG のコントラスト比。明るいほうを上に置いた比で、1〜21 になります。
function contrast(one: number, other: number): number {
  const bright = Math.max(one, other);
  const dark = Math.min(one, other);
  return (bright + 0.05) / (dark + 0.05);
}

/// WCAG の相対輝度（0〜1）。読めない色なら `null`。
function relativeLuminance(color: string): number | null {
  const rgb = parseHexColor(color);
  if (rgb === null) return null;
  const [red, green, blue] = rgb;
  return (
    0.2126 * channelLuminance(red) +
    0.7152 * channelLuminance(green) +
    0.0722 * channelLuminance(blue)
  );
}

/// `#rgb` / `#rrggbb` を 0〜1 の 3 つに開く。読めなければ `null`。
function parseHexColor(color: string): [number, number, number] | null {
  const text = color.trim().replace(/^#/, "");
  const expanded =
    text.length === 3
      ? text
          .split("")
          .map((digit) => digit + digit)
          .join("")
      : text;
  if (!/^[0-9a-f]{6}$/i.test(expanded)) return null;
  const value = Number.parseInt(expanded, 16);
  return [
    ((value >> 16) & 0xff) / 255,
    ((value >> 8) & 0xff) / 255,
    (value & 0xff) / 255,
  ];
}

/// WCAG の相対輝度が使う、1 チャンネルぶんのガンマ補正。
function channelLuminance(channel: number): number {
  return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

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
/// **何も打っていないうちは 1 つも出しません**（#169）。以前は選んでいないタグを
/// 全部並べていましたが、ボードのタグが増えるほど、カード 1 枚を書いている場所を
/// ボード全体のタグの一覧が埋めます（`docs/DESIGN.md`「常用しない操作を画面に
/// 常時出さない」）。どんなタグがあるかを見るのはタグ整理パネルの仕事で、ここは
/// 打った文字に当たるものを出す場所です。
///
/// 既に選んであるものは出しません（付いていることはチップで見えている）。
export function suggestTags(
  tags: readonly Tag[],
  selected: readonly number[],
  typed: string,
): Tag[] {
  const needle = typed.trim().toLocaleLowerCase();
  if (needle === "") return [];
  return tags.filter(
    (tag) =>
      !selected.includes(tag.id) && tag.name.toLocaleLowerCase().includes(needle),
  );
}
