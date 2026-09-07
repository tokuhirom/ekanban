// 説明の中の URL（#129、[ADR 0033]）。
//
// **見つけ方はここにあります。** ADR 0002 のころは Rust の `description_links`
// が位置を返し、こちらは色を塗るだけでしたが、Markdown のエディタでは「どこが
// リンクか」は編集器の中のノードそのものです。外から位置で塗る作りには戻せない
// ので、拾う規則をこちらに持ちます（[ADR 0033]）。
//
// **開いてよい形かどうかは Rust が決めたままです**（`commands::openable_url`）。
// ここが決めるのは「打った文字のどこからどこまでが URL か」だけです。
//
// [ADR 0033]: ../../../docs/adr/0033-a-markdown-editor-for-the-description.md

import type { Platform } from "../ipc/types/Platform";

/** 見つけた URL の位置。`text.slice(start, end)` がその URL。 */
export interface UrlSpan {
  start: number;
  end: number;
}

const SCHEMES = ["https://", "http://"];

/// 末尾に付いてきた句読点や閉じ括弧。URL の一部ではない。
const TAIL = [
  ".", ",", ";", ":", "!", "?", '"', "'", "]", "}", ">",
  "。", "、", "！", "？", "」", "』", "】", "）", ")",
];

/// 閉じ括弧に対応する開き括弧。対応が取れているなら URL の一部として残す。
const OPENING: Record<string, string> = { ")": "(", "）": "（", "]": "[", "}": "{" };

/// 打った文字の中の、最初の `http(s)://` の URL。無ければ `null`。
///
/// 拾うのは `http://` と `https://` だけです。裸の `example.com` や `ftp://` まで
/// 拾うと、URL でない文字列がリンクになります。
export function urlAround(text: string): UrlSpan | null {
  const start = SCHEMES.map((scheme) => text.indexOf(scheme))
    .filter((at) => at !== -1)
    .sort((a, b) => a - b)[0];
  if (start === undefined) return null;

  const rest = text.slice(start);
  const space = rest.search(/\s/u);
  const end = start + (space === -1 ? rest.length : space);
  const url = trimTail(text.slice(start, end));
  // スキームだけのものは URL として扱わない。
  if (SCHEMES.some((scheme) => url === scheme || url.length <= scheme.length)) return null;
  return { start, end: start + url.length };
}

/// URL の末尾に付いてきた句読点や閉じ括弧を落とす。
///
/// 「詳しくは https://example.com/a 。」の `。` は URL ではない。ただし `)` は、
/// 対応する `(` が URL の中にあるなら残す——`https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)`
/// のようなアドレスを壊さないため。
function trimTail(url: string): string {
  let trimmed = url;
  for (;;) {
    const last = Array.from(trimmed).at(-1);
    if (last === undefined || !TAIL.includes(last)) return trimmed;
    const opening = OPENING[last];
    if (opening !== undefined) {
      const body = trimmed.slice(0, -last.length);
      if (count(body, opening) > count(body, last)) return trimmed;
    }
    trimmed = trimmed.slice(0, -last.length);
  }
}

function count(text: string, character: string): number {
  return Array.from(text).filter((each) => each === character).length;
}

/// リンクを開く押し方か（ADR 0002 の「修飾キーを要求する」を引き継ぐ）。
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
