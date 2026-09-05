# 0006. CI を 3 つのプラットフォームで回す

- 日付: 2026-09-05
- 状態: 有効
- 関連: [#56](https://github.com/tokuhirom/ekanban/issues/56) [#66](https://github.com/tokuhirom/ekanban/pull/66)

## 状況

`Check and test` は `ubuntu-latest` でしか走らず、macOS と Windows がビルドされるのはタグを打ったときだけだった。そのため次のコードは **pull request の時点で一度もコンパイルされない**。

- `src/menu.rs` のネイティブメニューバー（`cx.set_menus` が実際に効くのは macOS だけ）
- `src/paths.rs` と `src/diagnostics.rs` の `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` の分岐

[0005](0005-in-app-menu-without-a-menu-bar.md) がその面積を広げた。プラットフォームごとに素直な形にする方針を採った以上、片方でしかコンパイルされないコードは増える。壊れていても気づく手段が「人が手元で起動する」しかない状態は、そこで持たなくなった。

## 決定

`Check and test`（ubuntu、fmt / clippy / test / build）は**名前も中身もそのまま**残し、`macos-latest` と `windows-latest` を回す別ジョブ `Build and test` を matrix で足す。新しいジョブは `cargo test --all-features` と `cargo build --all-features` だけ。

## 理由

- **fmt と clippy を ubuntu だけにする理由。** プラットフォームに依らないので、3 回回しても結果が同じで時間だけ増える
- **既存ジョブを matrix にしない理由（重要）。** matrix にすると check run の名前が `Check and test (ubuntu-latest)` になり、**`Check and test` という名前のチェックがどこにも現れなくなる**。ルールセットはその名前で必須チェックを指定しているので、条件が永久に満たされず、auto-merge を含めて**すべての pull request がマージ不能になる**

2 つ目の罠は、踏むと直すまで何も進まない種類のものなので、ワークフローのコメントと `docs/DEVELOPMENT.md` の両方に理由を書いた。

## 採らなかった案

| 案 | 採らなかった理由 |
| --- | --- |
| `check` ジョブを matrix にする | 必須チェックの名前が消え、全 PR がマージ不能になる |
| 3 プラットフォームで fmt と clippy も回す | 結果が同じで、実行時間だけ増える |
| リリース時だけ 3 プラットフォームでビルドする（従来） | 壊れたと分かるのがタグを打った後になる。直す場所が PR から遠い |

## 結果

**良くなったこと。** PR の時点で 3 プラットフォームのビルドとテストが走る。導入した時点では macOS も Windows も緑で、隠れた退行は無かった。以降の PR はすべて 3 つで検証されている。

**引き受けた不都合。**

- **CI の所要時間が伸びた。** Windows は cold cache で 9 分ほどかかる（ubuntu は 1〜2 分）
- **必須チェックの追加はリポジトリ設定**なので、ワークフローの変更だけでは完結しない。ルールセットに `Build and test (macos-latest)` と `Build and test (windows-latest)` を足すまでの間は、それらが赤くても ubuntu だけでマージできてしまう状態が残った（設定は同日に更新済み）
- ヘッドレステストが通ることと、その環境で**実際に動く**ことは別。macOS のメニューバーも Wayland の枠も、コンパイルが通るだけでは動作の保証にならない
