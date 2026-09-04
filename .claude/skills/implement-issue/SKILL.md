---
name: implement-issue
description: tokuhirom/ekanban の GitHub issue を実装する。先に設計して issue にコメントし、それから実装し、PR を作って自動マージを有効化する。「#12 を実装して」「issue 12 やっといて」のように issue 番号や URL を渡して使う。
---

# ekanban の issue を実装する

設計 → issue にコメント → 実装 → PR → 自動マージ有効化、までを 1 本で通す。

## 対象の制限

**扱ってよいのは `tokuhirom/ekanban` の issue だけ。**

- 引数が番号（`12` / `#12`）なら `tokuhirom/ekanban` のものとみなす
- 引数が URL なら `https://github.com/tokuhirom/ekanban/issues/<番号>` であることを確認する。ホストや owner/repo が違えば **そこで止める。実装も PR も作らない**
- `gh` は毎回 `--repo tokuhirom/ekanban` を付ける。カレントディレクトリからの推定に任せない
- 引数が無ければ、どの issue かをユーザーに聞く。open issue から勝手に選ばない

## 1. 状況を確認する

```bash
gh issue view <N> --repo tokuhirom/ekanban --comments
git status --short
```

- issue が CLOSED なら止めて報告する
- 既に設計コメントが付いていれば、それを踏まえる。同じ内容を二重に投稿しない
- 作業ツリーが汚れていたら止めて確認する。**このリポジトリは並行して別のセッションが触っていることがある。** 他人の未コミットの変更を巻き込まない
- きれいなら `git switch main && git pull --ff-only`

## 2. 設計する

読むもの:

- `docs/DESIGN.md` — 引き継ぐ設計の決まりごと / スコープ外 / 検討して入れないと決めたもの / 変更の完了条件
- `AGENTS.md` — コーディング規約とテストの方針
- issue の本文と既存のコメント
- 関係するソース。`src/model.rs`（モデル）、`src/db/mod.rs`（スキーマと保存）、`src/views/`（描画と入力）、`src/menu.rs`（メニュー）

決めること:

- 変更するファイルと、それぞれで何をするか
- データモデルやスキーマを変えるか。変えるならマイグレーションと、旧バージョンの DB を開くテスト
- `docs/DESIGN.md` の「引き継ぐ設計の決まりごと」に触れるか。破るなら、破る理由
- テストで何を担保するか
- issue の受け入れ条件のうち、コードでは確認できないもの（日本語 IME、macOS のライト / ダーク表示、Finder・Dock の表示など）

**issue の受け入れ条件を勝手に足したり減らしたりしない。** スコープを変えるべきだと思ったら、変えずにコメントでそう書き、判断を仰ぐ。

設計が固まらない、または issue の記述だけでは決められないことがあるなら、**実装に進まず** その点を issue にコメントしてユーザーに聞く。

## 3. 設計を issue にコメントする（実装の前に）

```bash
gh issue comment <N> --repo tokuhirom/ekanban --body-file <path>
```

日本語で書く。issue も `docs/DESIGN.md` も日本語なので合わせる。

```markdown
## 実装方針

<何をどう作るか。3〜10 行>

## 変更するファイル

| ファイル | 変更内容 |
| --- | --- |
| `src/...` | ... |

## 設計判断

- <選んだ案／採らなかった案／その理由>

## テスト

- <追加するテストと、それが担保すること>

## 実機確認が必要なもの

- <AI では確認できないもの。無ければ「なし」>
```

## 4. ブランチを切って実装する

```bash
git switch -c issue-<N>-<英語の短いスラッグ>
```

- 設計コメントに書いたとおりに実装する
- 途中で方針が変わったら、**変わった時点で** issue に追加でコメントする。PR を作ってから辻褄を合わせない
- `AGENTS.md` の規約に従う。SQL は `src/db/` に閉じる。UI から直接 DB を触らない。エラーは既存の `thiserror` 型を使う
- テスト名は観測できる振る舞いで付ける（`moves_card_to_another_column` のように）。DB のテストは `tempfile` を使い、実物の `.ekanban.sqlite3` を触らない

## 5. 完了条件を通す

`docs/DESIGN.md` の「変更の完了条件」を全部満たす。

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-features
```

落ちたら直す。**落ちたまま次に進まない。** 直せないなら、そこで止めて出力ごと報告する。

実機確認（IME、ライト / ダーク、Finder・Dock）は AI にはできない。**「確認した」と書かない。** PR 本文の「確認をお願いしたいこと」に回す。

## 6. commit / push / PR

コミットメッセージは英語。1 行目に命令形の要約、空行、なぜそうしたか。末尾に:

```
Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
```

```bash
git push -u origin <branch>
gh pr create --repo tokuhirom/ekanban --base main --title "<英語の要約>" --body-file <path>
```

PR 本文:

```markdown
Closes #<N>

## やったこと

- ...

## 受け入れ条件

- [x] <issue の受け入れ条件をそのまま並べ、確認できたものにチェック>
- [ ] <実機確認が要るものは未チェックのまま残す>

## 確認をお願いしたいこと

- <実機でしか確認できないもの。無ければ「なし」>

## 検査

`cargo fmt` / `clippy -D warnings` / `test --all-features` / `build --all-features` すべて通過。

設計: <設計コメントの URL>

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

`Closes #<N>` は必ず入れる。マージで issue が閉じるようにする。

## 7. 自動マージを有効化する

```bash
gh pr merge <PR番号> --repo tokuhirom/ekanban --squash --auto --delete-branch
```

リポジトリ側は設定済み。`allow_auto_merge` が有効で、`delete_branch_on_merge` も有効。ruleset `main` が `main` に必須ステータスチェック `Check and test`（`.github/workflows/ci.yml`）を課しているので、PR は CI が緑になるまでブロックされ、`--auto` はその通過を待ってからマージする。

失敗したら、理由をそのまま報告する。握りつぶさない。確認するもの:

```bash
gh api repos/tokuhirom/ekanban --jq .allow_auto_merge          # true のはず
gh api repos/tokuhirom/ekanban/rulesets --jq '.[].enforcement'  # active のはず
gh pr checks <PR番号> --repo tokuhirom/ekanban
```

- 「Pull request is in clean status」で断られたら、必須チェックが外れている。**マージせずに**ユーザーへ報告して指示を仰ぐ。ブロックが無い状態の `--auto` は即マージと同じで、CI を待たない
- CI が落ちたら、auto-merge を有効にしたまま放置しない。落ちた内容を直して push し直す

有効化できたら PR の URL を報告して終わる。**マージの完了を待たない。**

## 最後に報告すること

- issue 番号と、設計コメントの URL
- PR の URL と、自動マージが有効になったかどうか
- 実機で確認してほしいこと
- 通した検査とその結果

## やらないこと

- `main` に直接コミットしない。ruleset で必須チェックが掛かっているので直接 push は通らないし、通す手段を探すのでもない
- `tokuhirom/ekanban` 以外の issue を扱わない
- issue の受け入れ条件を勝手に変えない
- 実機確認していないものを「確認済み」と書かない
- `--admin` や `--force` でマージの保護を迂回しない
- 設計コメントを飛ばして実装に入らない
