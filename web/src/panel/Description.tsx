// 説明の欄。**Markdown のエディタです**（#129、[ADR 0033]）。
//
// 打ちながら整います——`**太字**`、`*斜体*`、`` `コード` ``、`# 見出し`、
// `- 箇条書き`、`- [ ] チェック項目`、`> 引用`。URL は打った瞬間にリンクに
// なります。
//
// **箇条書きは、打ち止めたところで終わります**（[ADR 0037]）。`Enter` は次の
// 項目を出しますが、その項目に何も打たないままもう一度押すと、段落に戻ります。
// 行頭の `Backspace` も同じです。ここは `ListPlugin` が持っている振る舞いで、
// **自前で `Enter` を捕まえてはいません**——入れ子や番号の振り直しまで同じ判断で
// 動く必要があり、2 か所に書くと必ず食い違います。
//
// **持っているのは Markdown の文字列だけ**です。`description` の形も、
// データベースも、書き出しも変わりません。編集器の中の木は表示のためのもので、
// 出入りするときに `@lexical/markdown` が文字列へ直します。
//
// 開くのは修飾キー＋クリックのままです（[ADR 0002] から引き継ぎ）。エディタの
// 中では、素のクリックは「そこにカーソルを置く」操作だからです。**開いてよい形か
// どうかを決めるのは Rust**（`commands::openable_url`）で、そこは動かしていません。
//
// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md
// [ADR 0033]: ../../../docs/adr/0033-a-markdown-editor-for-the-description.md
// [ADR 0037]: ../../../docs/adr/0037-a-list-that-ends-where-you-stop-typing.md

import { CodeNode } from "@lexical/code";
import { AutoLinkNode, LinkNode } from "@lexical/link";
import { $insertList, $isListItemNode, $isListNode, ListItemNode, ListNode } from "@lexical/list";
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
  CHECK_LIST,
  TRANSFORMERS as MARKDOWN_TRANSFORMERS,
} from "@lexical/markdown";
import { AutoLinkPlugin } from "@lexical/react/LexicalAutoLinkPlugin";
import { CheckListPlugin } from "@lexical/react/LexicalCheckListPlugin";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { type EditorState, TextNode } from "lexical";
import { useEffect, useRef } from "react";

import { useIpc } from "../ipc";
import type { Platform } from "../ipc/types/Platform";
import { opensLink, urlAround } from "./links";

interface Props {
  id: string;
  value: string;
  platform: Platform;
  onChange: (value: string) => void;
  /**
   * 打った内容を確定する（保存済みのカードだけ、#141、ADR 0032）。
   *
   * 欄を離れたときと、打ち止まって 1 秒で呼びます。**打鍵のたびには呼びません**
   * ——1 打鍵ごとに `update_card` を呼ぶと、Undo が 1 文字ずつ積まれます。
   */
  onCommit?: ((value: string) => void) | undefined;
}

/// 読み書きに使う変換。**`@lexical/markdown` の既定には `- [ ]` が入っていません**
/// ので、こちらで足します。並びの先頭に置くのは、`- [ ] 買い物` を「箇条書き」と
/// 読まれる前に「チェック項目」と読ませるためです——後ろに置くと、`- ` のほうが
/// 先に合って、`[ ]` がただの文字として残ります。
const TRANSFORMERS = [CHECK_LIST, ...MARKDOWN_TRANSFORMERS];

/// 編集器が使うノード。**Markdown の変換が作るものと揃えます**——ここに無い
/// ノードを変換が作ると、その場で例外になります。
const NODES = [HeadingNode, QuoteNode, ListNode, ListItemNode, CodeNode, LinkNode, AutoLinkNode];

/// クラス名は自分たちのものを当てます。リンクだけは、これまでと同じ
/// `description-link`——見た目も、テストが見ている名前も変えません。
const THEME = {
  link: "description-link",
  paragraph: "description-paragraph",
  // チェック項目は `☐` を自分で描きます。`ul` と `li` は要素のまま
  // CSS で整えていますが、**チェックの付き外しは class でしか分かりません**
  // ——`listitemChecked` / `listitemUnchecked` を渡さないと、印の描き分けが
  // できません。
  list: {
    checklist: "description-checklist",
    listitemChecked: "description-checked",
    listitemUnchecked: "description-unchecked",
  },
  text: {
    bold: "description-bold",
    italic: "description-italic",
    code: "description-code",
  },
};

export function Description({ id, value, platform, onChange, onCommit }: Props) {
  const ipc = useIpc();
  const modifier = platform === "macos" ? "Cmd" : "Ctrl";
  // いま画面に出ている Markdown。**外から来た値と突き合わせる**のに使います
  // ——自分が出した値がそのまま戻ってきたときに、編集器を作り直さないため。
  const shownRef = useRef(value);

  return (
    <LexicalComposer
      initialConfig={{
        namespace: "description",
        theme: THEME,
        nodes: NODES,
        // 読めない Markdown で編集器が固まるより、素のまま出したい。
        onError: (error: Error) => {
          void ipc.logFrontendError(`description editor: ${error.message}`);
        },
        editorState: () => {
          $convertFromMarkdownString(value, TRANSFORMERS);
        },
      }}
    >
      <div
        className="description-field"
        // 欄を離れたら確定（#141）。打ち止まって 1 秒のほうは下の見張りが呼びます。
        onBlur={() => {
          onCommit?.(shownRef.current);
        }}
      >
        <RichTextPlugin
          contentEditable={
            <ContentEditable
              id={id}
              className="field-input card-description-input"
              aria-label="説明"
              title={`${modifier} を押しながらクリックすると、リンクを開きます`}
            />
          }
          placeholder={<p className="description-placeholder">説明（任意）</p>}
          ErrorBoundary={LexicalErrorBoundary}
        />
        {/* 打ちながら整える。`**` や `# ` を打った時点で変わります。 */}
        <MarkdownShortcutPlugin transformers={TRANSFORMERS} />
        {/* 箇条書きの振る舞い。空の項目で `Enter` を押すと箇条書きが
            終わり、行頭の `Backspace` は項目をほどきます。**節を作るだけなら
            変換器だけで足ります**——これが無いと、`Enter` がいつまでも次の
            項目を出し続けます。 */}
        <ListPlugin />
        {/* `- [ ]` のチェック項目を押して付け外しする。 */}
        <CheckListPlugin />
        <CheckItem />
        <HistoryPlugin />
        <LinkPlugin />
        <AutoLinkPlugin matchers={MATCHERS} />
        <MarkdownValue value={value} shownRef={shownRef} onChange={onChange} onCommit={onCommit} />
        <OpenLink platform={platform} />
      </div>
    </LexicalComposer>
  );
}

/// 打った Markdown を外へ出し、外から来た Markdown を編集器へ入れる。
///
/// **自分が出した値が戻ってきたときは何もしません。** 入れ直すと、打っている
/// 途中でカーソルが先頭へ飛びます。
function MarkdownValue({
  value,
  shownRef,
  onChange,
  onCommit,
}: {
  value: string;
  shownRef: { current: string };
  onChange: (value: string) => void;
  onCommit: ((value: string) => void) | undefined;
}) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (value === shownRef.current) return;
    shownRef.current = value;
    editor.update(() => {
      $convertFromMarkdownString(value, TRANSFORMERS);
    });
  }, [editor, shownRef, value]);

  useEffect(
    () =>
      editor.registerUpdateListener(({ editorState }: { editorState: EditorState }) => {
        const markdown = editorState.read(() => $convertToMarkdownString(TRANSFORMERS));
        if (markdown === shownRef.current) return;
        shownRef.current = markdown;
        onChange(markdown);
      }),
    [editor, onChange, shownRef],
  );

  // 打ち止まって 1 秒で確定します（#141）。**打鍵のたびには確定しません**
  // ——1 文字ごとに `update_card` を呼ぶと、Undo が 1 文字ずつ積まれます。
  const committed = useRef(value);
  useEffect(() => {
    if (onCommit === undefined || value === committed.current) return;
    const timer = setTimeout(() => {
      committed.current = value;
      onCommit(value);
    }, 1000);
    return () => {
      clearTimeout(timer);
    };
  }, [onCommit, value]);

  return null;
}

/// 箇条書きの項目の先頭に `[ ] ` と打ったら、チェック項目にする。
///
/// **Markdown の変換だけでは届きません。** `@lexical/markdown` が記法を見るのは
/// 段落——根の直下にある塊——の先頭だけで、`- ` を打った時点でそこは箇条書きの
/// 項目になっています。`- [ ] 買い物` と打つと、`- ` が先に効いて、残りの `[ ]`
/// はただの文字として残ります。そこで、**項目の中の文字が変わったときに見ます**。
///
/// 変わるのはその箇条書き全体です（`$insertList` がそうします）。Lexical では
/// 「チェックが付くかどうか」は箇条書きの側の性質で、1 つの箇条書きに印のある項目と
/// 無い項目を混ぜられません。**混ぜられる形に見せかけない**ほうが、書いたものと
/// 保存される Markdown が食い違わずに済みます。
function CheckItem() {
  const [editor] = useLexicalComposerContext();

  useEffect(
    () =>
      editor.registerNodeTransform(TextNode, (text) => {
        const item = text.getParent();
        if (!$isListItemNode(item) || item.getFirstChild() !== text) return;
        const list = item.getParent();
        if (!$isListNode(list) || list.getListType() === "check") return;
        const match = /^\[[ x]\]\s/i.exec(text.getTextContent());
        if (match === null) return;

        // 記法は残しません。打った `[ ] ` は印そのものになります。
        const rest = text.getTextContent().slice(match[0].length);
        if (rest === "") {
          text.remove();
          item.select();
        } else {
          text.setTextContent(rest);
          // カーソルは記法を消したところ——打ち続けた文字が、印の右に出ます。
          text.select(0, 0);
        }
        $insertList("check");
        // `[x]` は付いた状態、`[ ]` は付いていない状態で置きます。
        item.setChecked(match[0].toLowerCase().includes("x"));
      }),
    [editor],
  );

  return null;
}

/// 打った URL をリンクにする。
///
/// 拾う形（`http(s)://` だけ、末尾の句読点は落とす）は `links.ts` の 1 か所に
/// 置き、`AutoLinkPlugin` にはその結果を渡します。**打っている途中の伸び縮みは
/// プラグインに任せます**——自分で節を差し替えると、1 文字打つたびに途中まで
/// のリンクが固まります。
const MATCHERS = [
  (text: string) => {
    const span = urlAround(text);
    if (span === null) return null;
    const url = text.slice(span.start, span.end);
    return { index: span.start, length: span.end - span.start, text: url, url };
  },
];

/// 修飾キー＋クリックでリンクを開く（ADR 0002 から引き継ぎ）。
function OpenLink({ platform }: { platform: Platform }) {
  const ipc = useIpc();
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const root = editor.getRootElement();
    if (root === null) return;
    const onClick = (event: MouseEvent) => {
      if (!opensLink(event, platform)) return;
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      const anchor = target.closest("a");
      const url = anchor?.getAttribute("href");
      if (url === null || url === undefined) return;
      event.preventDefault();
      void ipc.openUrl(url);
    };
    root.addEventListener("click", onClick);
    return () => {
      root.removeEventListener("click", onClick);
    };
  }, [editor, ipc, platform]);

  return null;
}
