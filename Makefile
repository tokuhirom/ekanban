APP_NAME := Ekanban
# Tauri のバンドラが出す先（`docs/TAURI-MIGRATION.md` §11）。
BUNDLE := target/release/bundle
RELEASE_APP := $(BUNDLE)/macos/$(APP_NAME).app
# 画面側の依存として入っている Tauri の CLI。別に入れる必要はない。
TAURI := ../../web/node_modules/.bin/tauri

.PHONY: help build release dev web-install web-check e2e test types types-check fmt fmt-check lint deps-check check screenshots icon bundle bundle-debug open install install-linux uninstall-linux clean

help: ## このヘルプを表示する
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## デバッグビルド
	cargo build --workspace

release: ## リリースビルド
	cargo build --workspace --release

dev: web-install ## アプリを開発モードで起動する (Vite の開発サーバごと)
	cd crates/app && $(TAURI) dev

web-install: ## 画面側の依存を入れる (ロックファイルのとおりに)
	npm --prefix web ci

web-check: web-install ## 画面側の型検査・lint・単体テスト
	npm --prefix web run typecheck
	npm --prefix web run lint
	npm --prefix web run test

e2e: web-install ## ハーネスを上げて Playwright を走らせる (Chromium と WebKit)
	cargo build -p ekanban-harness --example manual_screenshot_seed
	cargo build -p ekanban-harness
	npm --prefix web run e2e

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

icon: web-install ## 3 つの OS 分のアイコンを assets/icon.png から生成する
	cd crates/app && $(TAURI) icon ../../assets/icon.png

bundle: web-install ## この OS の配布物を作る (.app/.dmg、.deb/.AppImage、インストーラ)
	cd crates/app && $(TAURI) build

bundle-debug: web-install ## デバッグビルドから配布物を作る
	cd crates/app && $(TAURI) build --debug

open: bundle ## .app を作って起動する (macOS)
	open $(RELEASE_APP)

install: bundle ## .app を /Applications にインストールする (macOS)
	rm -rf /Applications/$(APP_NAME).app
	cp -R $(RELEASE_APP) /Applications/
	@echo "installed /Applications/$(APP_NAME).app"

install-linux: ## Linux のアプリ一覧に登録する (~/.local 以下、root は要らない)
	cargo build -p ekanban-app --release
	script/install-linux

uninstall-linux: ## install-linux で入れたものを消す
	script/install-linux --uninstall

clean: ## ビルド成果物を消す
	cargo clean
