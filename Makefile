APP_NAME := Ekanban
RELEASE_APP := target/release/bundle/$(APP_NAME).app
DEBUG_APP := target/debug/bundle/$(APP_NAME).app

.PHONY: help build release run dev web-install web-check test types types-check fmt fmt-check lint deps-check check screenshots icon bundle bundle-debug open install install-linux uninstall-linux clean

help: ## このヘルプを表示する
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## デバッグビルド
	cargo build --workspace

release: ## リリースビルド
	cargo build --workspace --release

run: ## いまのアプリ (gpui) をターミナルから直接起動する
	cargo run -p ekanban

dev: web-install ## Tauri のアプリを開発モードで起動する (Vite の開発サーバごと)
	cd crates/app && ../../web/node_modules/.bin/tauri dev

web-install: ## 画面側の依存を入れる (ロックファイルのとおりに)
	npm --prefix web ci

web-check: web-install ## 画面側の型検査・lint・単体テスト
	npm --prefix web run typecheck
	npm --prefix web run lint
	npm --prefix web run test

test: ## テストを実行する
	cargo test --workspace --all-features

fmt: ## フォーマットを適用する
	cargo fmt --all

fmt-check: ## フォーマット崩れがないか確認する
	cargo fmt --all -- --check

lint: ## clippy を実行する
	cargo clippy --workspace --all-targets --all-features -- -D warnings

deps-check: ## ekanban-core が UI ツールキットに依存していないことを確かめる
	script/check-core-independence

types: ## Rust の型から TypeScript の型を書き出す (web/src/ipc/types/)
	cargo test --workspace

types-check: types ## 書き出した型がコミットしてあるものと同じか確かめる
	git diff --exit-code -- web/src/ipc/types

check: fmt-check lint test types-check deps-check web-check ## CI と同じチェックを一通り走らせる

screenshots: ## マニュアルのスクリーンショットを撮り直す (Linux/X11 のみ)
	script/manual-screenshots

icon: assets/icon.icns ## macOS 用の .icns アイコンを生成する

assets/icon.icns: assets/icon.png
	@set -eu; \
	if [ "$$(uname -s)" != "Darwin" ]; then \
		echo "assets/icon.icns は macOS 上でのみ生成できます (sips/iconutil が必要です)" >&2; \
		exit 1; \
	fi; \
	iconset="$$(mktemp -d "$${TMPDIR:-/tmp}/ekanban-iconset.XXXXXX").iconset"; \
	mkdir -p "$$iconset"; \
	trap 'rm -rf "$$iconset"' EXIT; \
	for size in 16 32 128 256 512; do \
		sips -z $$size $$size assets/icon.png --out "$$iconset/icon_$${size}x$${size}.png" >/dev/null; \
		retina_size=$$((size * 2)); \
		sips -z $$retina_size $$retina_size assets/icon.png --out "$$iconset/icon_$${size}x$${size}@2x.png" >/dev/null; \
	done; \
	iconutil -c icns "$$iconset" -o "$@"

bundle: icon ## リリースビルドから .app を作る
	script/bundle-mac release

bundle-debug: icon ## デバッグビルドから .app を作る
	script/bundle-mac debug

open: bundle ## .app を作って起動する
	open $(RELEASE_APP)

install: bundle ## .app を /Applications にインストールする
	rm -rf /Applications/$(APP_NAME).app
	cp -R $(RELEASE_APP) /Applications/
	@echo "installed /Applications/$(APP_NAME).app"

install-linux: release ## Linux のアプリ一覧に登録する (~/.local 以下)
	script/install-linux

uninstall-linux: ## install-linux で入れたものを消す
	script/install-linux --uninstall

clean: ## ビルド成果物を消す
	cargo clean
