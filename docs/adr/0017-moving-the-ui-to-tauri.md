# 0017. UI を gpui-kit から Tauri へ移す

- 日付: 2026-09-06
- 状態: 有効
- 関連: なし（issue より先に、方針として決めたもの）

## 状況

ekanban は最初から GPUI Kit で作ってきた。選んだ理由は、GPU で描くネイティブアプリの軽さと、D&D の手触りを自分で決められることだった。それは実際に手に入っている。

問題は、**ここから使いやすくしていく先が見えないこと**にある。症状が 3 つ出ていて、どれも 1 つの issue では解けない。

### 部品に天井がある

`docs/DESIGN.md` に、こう書いてある。

> 説明欄はコードエディタのモード（`EditorState`）で持つ。色を付ける口（`InputHighlighter`）もクリックの当たり判定（`DefinitionProvider`）も、`gpui-base` ではコードエディタのモードにしか無い。ふつうの複数行入力（`TextareaMode`）は `type Extras = ()` で、置き場所が無い。

プレーンテキストの説明欄を出すために、コードエディタを引っぱってきて行番号と折りたたみと検索を切っている。**要求を部品側の都合に合わせて曲げている**のであって、これは今後も同じ形で繰り返す。

タグの入力も同じところに来た。チップ列＋補完付き入力に相当する部品が無い。1 つなら書けばいいが、「入力欄の質を上げる」がこれから当分の作業の中心になるのに、その全部を土台の外側で自作することになる。

### 供給が安定していない

crates.io の状況（2026-09-06 時点）。

| 系統 | 最新 | 最終公開 |
| --- | --- | --- |
| Zed 公式 `gpui` | 0.2.2 | 2025-10-22 |
| `gpui-ce`（community fork） | 0.2.2 | 2026-08-28 |
| `*-gpui-unofficial` | 1.18.1 | 2026-09-04 |
| longbridge `gpui-component` | 0.6.0 | 2026-09-03 |
| **`gpui-kit`（いま使っているもの）** | 0.6.0 | **2026-09-03** |

公式が crates.io 上で 11 ヶ月止まり、その空白を 3 系統が埋めにきている。`Cargo.toml` の `gpui-kit = "0.6.0"` は、`gpui-component` から改名されて公開 3 日目のクレート名を指している。`gpui-pre-linux` という platform 層のスナップショットに乗っていることは、[0012](0012-focus-after-quick-capture-on-linux.md) で既に一度突き当たっている。

### 標準の手段がどれも使えない

gpui はアクセシビリティツリーを持たない。[AccessKit](https://accesskit.dev/) を入れる計画はあるが、Zed 側の見通しは「1.0 よりずっと先まで続く長期プロジェクト」で、2025 年末時点で Windows の JAWS / NVDA からは何も読めない（[zed#6576](https://github.com/zed-industries/zed/discussions/6576)、[zed#41138](https://github.com/zed-industries/zed/issues/41138)）。

AX API も UI Automation も AT-SPI もツリーを返さないので、その上に乗る自動操作は原理的に使えない。テストは gpui 自身の `TestPlatform` が天井で、`view_tests.rs` の 1,047 行は既にそこまで書いてある。**これ以上の手段を外から持ってこられない。**

Rust GUI の中でも、これは gpui の側が外れ値になっている。egui・Slint・Bevy・Freya・Xilem は AccessKit を入れている。

同じ理由が、開発の速度にも効いている。ekanban は AI 支援で書いているが、gpui は学習データが薄く、フォークごとに API が違う。[0001](0001-links-in-the-description.md) は、その代償の記録そのものになっている。

> この判断は 1 日で覆した。「入力欄には読むだけの表示が無い」で止めてしまい、`gpui-base` の入力欄が持つ口をそれ以上調べていなかった。

`docs/` がこの厚さになっているのも、半分は同じことの裏返しで、web なら学習データが持っている知識を、こちらで書き下ろして補っている。

### 閉じた issue の内訳

そして、これまで閉じた issue を見ると、アプリの論理ではなく**フレームワークとの摩擦**だったものが相当ある。

| # | 内容 |
| --- | --- |
| [#19](https://github.com/tokuhirom/ekanban/issues/19) | `cmd-` が非 macOS で Super に落ちる |
| [#49](https://github.com/tokuhirom/ekanban/issues/49) | Wayland でタイトルバーが出ない |
| [#52](https://github.com/tokuhirom/ekanban/issues/52) | `hide` / `activate` が Linux で未実装 |
| [#53](https://github.com/tokuhirom/ekanban/issues/53) | 終了とフルスクリーンが Super に落ちて押せない |
| [#54](https://github.com/tokuhirom/ekanban/issues/54) | `on_reopen` が無く、`Cmd+W` のあと戻せない |
| [#78](https://github.com/tokuhirom/ekanban/issues/78) | 編集画面のメニューが入力欄の下に潜る |

どれも Kanban の話ではない。webview なら、この列はほぼ空になる。

## 決定

**UI 層を Tauri へ移す。** Rust の中核はそのまま残し、画面だけを webview 側で作り直す。

`src` の内訳はこうなっている。

| | 行数 | 移行 |
| --- | --- | --- |
| **gpui 非依存**（`model.rs` 3,386 / `db/mod.rs` 2,233 / `backup.rs` / `paths.rs` / `instance.rs` / `diagnostics.rs`） | **6,391** | **そのまま持っていく** |
| gpui 依存（`views/` 7,559 / `menu.rs` / `lib.rs` / `hotkey.rs` / `actions.rs`） | 8,949 | 作り直す |

`model.rs` と `db/mod.rs` には `gpui` という文字列が 1 つも出てこない。カードの移動も reindex も Undo / Redo もスキーマ移行も、`#[tauri::command]` を被せるだけで載る。作り直すのは 8,949 行で、うち 1,047 行は E2E に置き換わるテストである。

`global-hotkey` は既に tauri-apps のクレートなので、[0012](0012-focus-after-quick-capture-on-linux.md) の制約ごとそのまま残る。`rusqlite` も変わらない。

**移行が終わるまで、gpui 側の決まりごとは有効なまま。** [0005](0005-in-app-menu-without-a-menu-bar.md) [0009](0009-per-platform-key-bindings.md) [0012](0012-focus-after-quick-capture-on-linux.md) [0015](0015-a-menu-bar-on-every-platform.md) [0016](0016-where-the-app-says-things.md) は、いま動いているアプリの規則として生きている。移行が実際に着地した時点で、それぞれを新しい ADR で置き換える。

**ただし、gpui 側の画面には機能を足さない。** 直すのは、使えなくなる不具合だけにする。

## 理由

**これから増える作業のほとんどが、部品の質の話だから。** 機能は README のとおり出揃っていて、残りは「使いやすくする」に寄っている。それは部品の出来がそのまま成果になる領域で、いまいちばん弱いところでもある。タグ操作のような UI は、web には出来のいいものがいくらでもある。

**別の Rust GUI への移動では、何も解決しないから。** egui / Iced / Slint は、部品の品揃えでも AI の習熟度でも gpui-kit と同等か下になる。テスト手法だけは AccessKit で改善するが、それ 1 つのために書き直すなら、3 つとも取れるほうを選ぶ。

**テストが標準の道具に戻るから。** webview なら Playwright がそのまま使える。いまは `TestPlatform` の中に閉じていて、外からの検証手段が 1 つも無い。

**Rust の資産が消えないから。** Tauri のバックエンドは Rust で、ekanban の 6,391 行はそこに載る。webview を使う選択肢の中で、これができるのは Tauri だけ。

**供給が安定しているから。** Tauri は 1.0 を越えていて、単一のフォークに人生を賭ける構図から降りられる。

## 採らなかった案

- **gpui-kit のまま続ける。** 上の 3 つの症状はどれも自力で解けない。部品は上流の設計で、フォークの乱立も AccessKit の不在もこちらの手が届かない。「アプリは完成しているのだから、このまま置く」という選択は成立するが、それは使いやすくするのを諦めるということで、諦めたくないからこの ADR がある。
- **egui / Iced / Slint に移る。** 上記のとおり、部品と AI の習熟という主要な 2 つが改善しない。書き直しの費用は Tauri と大差ないのに、得るものが 1 つだけになる。
- **足りない部品を自分で書き、gpui-kit に留まる。** タグ入力 1 つなら妥当な判断だった。これが 3 つ目・4 つ目と続くと分かった時点で、「部品を自作しながらアプリを作る」プロジェクトになる。作りたいのは Kanban であって UI ツールキットではない。
- **gpui-kit をフォークして自分で持つ。** 上流の更新を取り込み続ける仕事が増えるだけで、AI の習熟度もテスト手法も変わらない。フォークが 4 系統ある状況に 5 つ目を足すことになる。
- **Electron にする。** webview の利点は同じだが、Rust の 6,391 行を捨てて全部を書き直すことになる。メモリも Tauri より重い。
- **中核ごと全部書き直す。** `model.rs` と `db/mod.rs` は gpui と無関係に育っていて、テストも付いている。捨てる理由が無い。

## 結果

得るもの。タグ入力のようなコントロールが既製品で済む。Playwright で本物の E2E が書ける。IME の面倒を webview が見る。AI が知っているスタックになる。プラットフォームごとの差（Wayland の装飾、Super に落ちるキー、`hide` の未実装）が、上の表の列ごと消える。

引き受ける不都合。

- **メモリが増える。** gpui を選んだ理由の 1 つを手放すことになる。macOS の WKWebView で、おそらく今の 1.5〜2 倍。着手前に実測して、想定を大きく超えるならこの判断を見直す。
- **Linux は WebKitGTK になる。** レンダリングも動作も、macOS の WKWebView・Windows の WebView2 と揃わない。プラットフォーム差が無くなるのではなく、**別の種類の差に置き換わる**。
- **依存に Node と npm が入る。** いまは `cargo` だけで完結している。CI もリリースも作り直す。
- **配布物が変わる。** `.app` バンドル、`script/install-linux`、`script/bundle-mac`、[0014](0014-unsigned-apple-silicon-only-macos-builds.md) の署名の話は、全部 Tauri のバンドラ側で組み直す。
- **`unsafe_code = "forbid"` の意味が変わる。** 禁止は Rust 側にしか効かず、UI は TypeScript になる。
- **gpui で払った知見の大半を捨てる。** [0005](0005-in-app-menu-without-a-menu-bar.md) [0009](0009-per-platform-key-bindings.md) [0015](0015-a-menu-bar-on-every-platform.md) [0016](0016-where-the-app-says-things.md) のうち、gpui の都合から来ている部分は無効になる。ただし**判断の中身**（メニューバーはどのプラットフォームでも出す、拒否は言葉ではなくコントロールで表す、常時表示は注意を配る仕組みとして壊れている）は UI ツールキットの話ではないので、移行先でも守る。
- **移行中は 2 つの UI を持つことになる。** どちらも動く状態を保つのは高くつくので、gpui 側は凍結して、Tauri 側が追いついた時点で入れ替える。並行して育てない。

D&D の手触りは、移行が成功したかどうかの判定基準になる。README が「いちばん大事にしています」と書いているのはここで、web の D&D で同じ水準に届かなければ、この移行は失敗である。
