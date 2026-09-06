// 説明欄の中のリンク（[ADR 0002]）。
//
// 説明はプレーンテキストのままです。Markdown は描かず、拾うのは `http(s)://`
// だけ——**その見つけ方は Rust に 1 つだけ**あり（`commands::description_links`）、
// ここが受け取るのはその結果です。同じ規則（末尾の句読点を落とす、括弧の対応を
// 見る）を TypeScript にもう 1 つ書くと、必ずずれます。
//
// 位置は UTF-16 の符号単位で届きます。JavaScript の文字列の数え方そのものなので、
// `slice` にそのまま渡せます。
//
// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md

import type { Platform } from "../ipc/types/Platform";
import type { UrlSpan } from "../ipc/types/UrlSpan";

/// 表示層に並べる一片。`url` が入っているところがリンク。
export interface Segment {
  text: string;
  url: string | null;
}

/// 本文を、リンクとそれ以外に切り分ける。
export function segments(text: string, links: readonly UrlSpan[]): Segment[] {
  const pieces: Segment[] = [];
  let at = 0;
  for (const link of links) {
    // 打っている途中の本文に、1 つ前の本文で見つけた位置が届くことがある。
    // はみ出したものは捨てる——ずれた位置で色を付けるより、色が付かないほうがよい。
    if (link.start < at || link.end > text.length) continue;
    if (link.start > at) pieces.push({ text: text.slice(at, link.start), url: null });
    pieces.push({ text: text.slice(link.start, link.end), url: link.url });
    at = link.end;
  }
  if (at < text.length) pieces.push({ text: text.slice(at), url: null });
  return pieces;
}

/// その位置にリンクがあるなら、それ。
///
/// 端も含めます。`https` の `h` の上や、末尾の 1 文字の後ろで押しても開ける
/// ようにするためで、gpui 版の `link_at` と同じです。
export function linkAt(links: readonly UrlSpan[], offset: number): UrlSpan | null {
  return links.find((link) => offset >= link.start && offset <= link.end) ?? null;
}

/// リンクを開く押し方か（[ADR 0002] の「修飾キーを要求する」）。
///
/// macOS は Cmd、ほかは Ctrl。修飾キー無しのクリックは、文章のどこかを指す
/// ためのものです——押すたびにブラウザが開いたら、説明を直せません。
export function opensLink(
  event: Pick<MouseEvent, "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
  platform: Platform,
): boolean {
  const isMac = platform === "macos";
  const secondary = isMac ? event.metaKey : event.ctrlKey;
  const other = isMac ? event.ctrlKey : event.metaKey;
  return secondary && !other && !event.altKey && !event.shiftKey;
}
