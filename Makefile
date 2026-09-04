APP_NAME := Ekanban
RELEASE_APP := target/release/bundle/$(APP_NAME).app
DEBUG_APP := target/debug/bundle/$(APP_NAME).app

.PHONY: help build release run test fmt fmt-check lint check icon bundle bundle-debug open install clean

help: ## このヘルプを表示する
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## デバッグビルド
	cargo build

release: ## リリースビルド
	cargo build --release

run: ## ターミナルから直接起動する (デバッグビルド)
	cargo run

test: ## テストを実行する
	cargo test --all-features

fmt: ## フォーマットを適用する
	cargo fmt --all

fmt-check: ## フォーマット崩れがないか確認する
	cargo fmt --all -- --check

lint: ## clippy を実行する
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt-check lint test ## CI と同じチェックを一通り走らせる

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

clean: ## ビルド成果物を消す
	cargo clean
