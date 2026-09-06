//! 説明の入力欄の中で、URL をリンクとして見せて、開けるようにする。
//!
//! 説明はプレーンテキストのままにする方針（`docs/DESIGN.md` の「やらないこと」に
//! Markdown の描画がある）なので、ここが解釈するのは `http(s)://` だけ。見出しも
//! 強調も記法も見ない。
//!
//! 色を付ける口（`InputHighlighter`）はコードエディタのモードにしか無い。ふつうの
//! 複数行入力（`TextareaMode`）は `type Extras = ()` で、色付けもクリック判定も
//! 置き場所が無い。そのため説明欄は `EditorState` にしてある。行番号・折りたたみは
//! 切ってあり、言語は下の [`LANGUAGE`] だけで、コードとして扱っているわけではない。

use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{
    component::input::{
        DefinitionProvider, EditorState, FoldRange, HighlightStyleResolver, InputEdit,
        InputHighlighter, InputHighlighterFactory, Rope, RopeExt as _,
    },
    component::ActiveTheme as _,
    App, Context, HighlightStyle, Hsla, SharedString, Task, UnderlineStyle, Window,
};

use crate::model::find_urls;

/// その byte 位置に URL があるなら、その範囲。
///
/// 端も含める。`https` の `h` の上や、末尾の 1 文字の後ろで押しても開けるように
/// するため。
fn link_at(text: &str, offset: usize) -> Option<Range<usize>> {
    link_ranges(text)
        .into_iter()
        .find(|range| (range.start..=range.end).contains(&offset))
}

/// 本文の中の URL の位置。byte 位置で持つ。
fn link_ranges(text: &str) -> Vec<Range<usize>> {
    find_urls(text)
        .into_iter()
        .map(|url| {
            // `find_urls` は本文の部分文字列を返すので、位置は差で出る。
            let start = url.as_ptr() as usize - text.as_ptr() as usize;
            start..start + url.len()
        })
        .collect()
}

/// 説明欄に付ける「言語」の名前。実在の言語ではなく、この強調のための札。
pub(crate) const LANGUAGE: &str = "ekanban-description";

/// 説明欄用の強調を組み立てる工場。`EditorState::set_highlighter_factory` に渡す。
pub(crate) fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        (language == LANGUAGE)
            .then(|| Box::new(LinkHighlighter::default()) as Box<dyn InputHighlighter>)
    })
}

/// 本文の中の URL の位置と、それを描く色を覚えておくもの。
#[derive(Default)]
struct LinkHighlighter {
    /// 本文の先頭からの byte 位置で持つ。`styles` が受け取る範囲と同じ単位。
    links: Vec<Range<usize>>,
    /// リンクの文字色。テーマから引く（`docs/DESIGN.md`「色は `ActiveTheme::theme()`
    /// から引く」）。`update` が呼ばれるまでは未設定で、そのときは文字色を変えない。
    color: Option<Hsla>,
}

impl LinkHighlighter {
    fn style(&self) -> HighlightStyle {
        HighlightStyle {
            color: self.color,
            underline: Some(UnderlineStyle {
                thickness: gpui_kit::px(1.),
                color: self.color,
                wavy: false,
            }),
            ..Default::default()
        }
    }
}

impl InputHighlighter for LinkHighlighter {
    fn language(&self) -> SharedString {
        LANGUAGE.into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        cx: &mut Context<EditorState>,
    ) {
        // `Rope` から一度 `String` にする。説明は人が書く長さなので、編集ごとに
        // 読み直しても差が出ない。増分で追うほうが、URL の切れ目を跨ぐ編集で
        // 間違えやすい。
        let text = text.to_string();
        self.links = link_ranges(&text);
        self.color = Some(cx.theme().colors.info);
    }

    /// `range` を隙間なく覆う、重ならない順序どおりの並びを返す。
    ///
    /// リンクでないところは `HighlightStyle::default()`（＝色を変えない）で埋める。
    /// 隙間があると、その部分がどう描かれるか決まらない。
    fn styles(
        &self,
        range: &Range<usize>,
        _resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let style = self.style();
        let mut runs = Vec::new();
        let mut at = range.start;
        for link in &self.links {
            let start = link.start.max(range.start);
            let end = link.end.min(range.end);
            if start >= end {
                continue;
            }
            if at < start {
                runs.push((at..start, HighlightStyle::default()));
            }
            runs.push((start..end, style));
            at = end;
        }
        if at < range.end {
            runs.push((at..range.end, HighlightStyle::default()));
        }
        runs
    }

    /// 折りたたみは使わない。説明はコードではないので、畳む単位が無い。
    fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// `Cmd` / `Ctrl` + クリックで URL を開くための受け口。
///
/// エディタは「定義へ移動」の行き先の scheme が `http(s)` なら、そのまま
/// `cx.open_url` に渡す。ここが答えるのは「その位置に URL があるか」だけで、
/// 開くのはエディタ側。
///
/// 素のクリックを取らないのは、それが「カーソルをそこに置く」という編集の意味を
/// 既に持っているため。Obsidian の編集モードも修飾キーを要求する。
pub(crate) struct LinkDefinitions;

impl DefinitionProvider for LinkDefinitions {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<gpui_kit::Result<Vec<lsp_types::LocationLink>>> {
        let body = text.to_string();
        let Some(range) = link_at(&body, offset) else {
            return Task::ready(Ok(Vec::new()));
        };
        let Ok(target) = body[range.clone()].parse::<lsp_types::Uri>() else {
            return Task::ready(Ok(Vec::new()));
        };

        // `origin_selection_range` を返すと、`Cmd` ホバーの下線と当たり判定が
        // URL 全体になる。返さないと単語の切れ目で切られ、`https` だけが
        // リンクに見える。
        let origin = lsp_types::Range {
            start: text.offset_to_position(range.start),
            end: text.offset_to_position(range.end),
        };
        // 行き先は外部の URL なので、文書内の位置は意味を持たない。エディタは
        // scheme を見て `open_url` に回すため、ここは使われない。
        let nowhere = lsp_types::Range {
            start: lsp_types::Position::new(0, 0),
            end: lsp_types::Position::new(0, 0),
        };
        Task::ready(Ok(vec![lsp_types::LocationLink {
            origin_selection_range: Some(origin),
            target_uri: target,
            target_range: nowhere,
            target_selection_range: nowhere,
        }]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoStyles;

    impl HighlightStyleResolver for NoStyles {
        fn style(&self, _: &str) -> Option<HighlightStyle> {
            None
        }
    }

    /// `update` は `Context<EditorState>` を要るので、テストからは位置の計算だけを
    /// 直接組み立てて確かめる。
    fn highlighter(text: &str) -> LinkHighlighter {
        LinkHighlighter {
            links: link_ranges(text),
            color: Some(gpui_kit::hsla(0.6, 1., 0.5, 1.)),
        }
    }

    fn run_kinds(text: &str) -> Vec<(String, bool)> {
        let highlighter = highlighter(text);
        highlighter
            .styles(&(0..text.len()), &NoStyles)
            .into_iter()
            .map(|(range, style)| (text[range].to_string(), style.color.is_some()))
            .collect()
    }

    #[test]
    fn points_at_the_address_under_the_cursor() {
        let text = "見て https://example.com/a ね";
        let start = text.find("https").expect("the address is in the text");
        let end = start + "https://example.com/a".len();

        assert_eq!(
            link_at(text, start),
            Some(start..end),
            "the first character counts"
        );
        assert_eq!(
            link_at(text, start + 4),
            Some(start..end),
            "and so does the middle"
        );
        assert_eq!(link_at(text, end), Some(start..end), "and the far end");
        assert_eq!(link_at(text, 0), None, "the words before it are not a link");
        assert_eq!(link_at(text, text.len()), None, "nor the ones after");
        assert_eq!(link_at("リンクのない説明", 3), None);
    }

    #[test]
    fn marks_only_the_address_inside_the_text() {
        assert_eq!(
            run_kinds("見て https://example.com/a ね"),
            [
                ("見て ".to_string(), false),
                ("https://example.com/a".to_string(), true),
                (" ね".to_string(), false),
            ],
            "the address is the only styled run, and the words around it keep their colour"
        );
    }

    #[test]
    fn covers_the_whole_range_without_gaps() {
        for text in [
            "",
            "リンクのない説明",
            "https://example.com/a",
            "https://example.com/a https://example.com/b",
            "先頭 https://example.com/a 中 https://example.com/b 末尾",
        ] {
            let runs = highlighter(text).styles(&(0..text.len()), &NoStyles);
            let mut at = 0;
            for (range, _) in &runs {
                assert_eq!(
                    range.start, at,
                    "the runs of {text:?} leave no gap: {runs:?}"
                );
                assert!(range.start < range.end, "and none of them is empty");
                at = range.end;
            }
            assert_eq!(
                at,
                text.len(),
                "and together they reach the end of {text:?}"
            );
        }
    }

    #[test]
    fn clips_the_runs_to_the_range_it_was_asked_about() {
        let text = "先頭 https://example.com/a 末尾";
        let asked = 3..text.len() - 3;
        let runs = highlighter(text).styles(&asked, &NoStyles);

        assert_eq!(
            runs.first().map(|(range, _)| range.start),
            Some(asked.start)
        );
        assert_eq!(runs.last().map(|(range, _)| range.end), Some(asked.end));
    }
}
