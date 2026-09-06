# Changelog

## [v0.2.0](https://github.com/tokuhirom/ekanban/compare/v0.1.3...v0.2.0) - 2026-09-06

### 変更
- Record the decision to move the UI from gpui-kit to Tauri by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/95
- Tauri でいまの機能をどう実現するかの設計 by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/98
- Split the core out of the UI as a Cargo workspace by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/99
- Draw the board in a Tauri window by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/100
- Drag and drop cards and columns with dnd-kit by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/101
- Check the drag conditions on both webview engines by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/102
- Let the webview edit cards, columns, tags and boards by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/103
- Put the menu bar, the theme and the window state on Tauri by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/104
- Give the webview the archive, the files and the description links by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/105
- Bring quick capture over to Tauri by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/106
- Ship the Tauri build instead of the gpui one by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/107
- Delete the gpui app by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/108
- Move the migration's rules into the design record by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/109
- Start the app from `cargo run`, or say why it cannot by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/110
- Correct the manual where it still describes the gpui app by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/111
- Filter by tag from the chips on a card by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/112
- Let the description's text show through its input field by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/121
- Explain the code without pointing at gpui by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/122
- Work through the open issues on the card and board panels by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/123

## [v0.1.3](https://github.com/tokuhirom/ekanban/compare/v0.1.2...v0.1.3) - 2026-09-06

### 変更
- Say things where they will be read, or not at all by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/87
- Draw the card panel menu above the fields it covers by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/94

## [v0.1.2](https://github.com/tokuhirom/ekanban/compare/v0.1.1...v0.1.2) - 2026-09-05

### 変更
- Linux と Windows のメニューを VS Code / Zed と同じメニューバーにする by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/80
- アプリを動かして確かめるときの手順を書く by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/83
- 期限の書き分けの表を、画像の日付に合わせる by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/85

## [v0.1.1](https://github.com/tokuhirom/ekanban/compare/v0.1.0...v0.1.1) - 2026-09-05

### 変更
- Save a card when Enter is pressed in its title field by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/44
- Let a column scroll when it holds more cards than fit by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/46
- Reach the menu bar's items without a menu bar by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/47
- Correct the min-height rule to name the axis that matters by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/48
- Keep a daily generation of the database by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/64
- Let a closed window come back, and give it a Window menu by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/65
- Build and test on macOS and Windows too by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/66
- Let only one process open a database by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/67
- Open a new database on an empty board by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/68
- Draw a frame when the compositor will not by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/69
- Offer the addresses written in a description by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/70
- Make the addresses in a description read as links by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/72
- Record the reasoning behind the design rules by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/74
- Require an ADR for a significant decision by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/76
- Clear the remaining open issues by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/77

## [v0.1.0](https://github.com/tokuhirom/ekanban/commits/v0.1.0) - 2026-09-05

### 変更
- Macos app bundle by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/1
- Add an implement-issue skill and require pull requests by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/15
- Open card details in a panel on the right instead of inline by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/16
- Let the board list collapse to a rail by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/18
- Give the window a title and an application id by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/20
- Bind application shortcuts to the secondary key by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/21
- Let a global hotkey bring ekanban to the front by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/22
- Capture a card from a one-line window by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/23
- Let a column be chosen as the capture destination by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/25
- Disable quick capture where it cannot work by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/26
- Write a manual, and fix what writing it exposed by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/27
- Point the README at people who want to use ekanban by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/29
- Add the MIT license text the README claims by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/30
- Filter by the tags already on the cards by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/33
- Drop filtering by due date by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/34
- Retake the manual screenshots, and keep the way they are taken by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/35
- Stop filling a new card's description with instructions by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/37
- ボードを本物のウィンドウで確かめるテストを足す by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/38
- タグを打ったらバイナリがリリースされるようにする by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/39
- tagpr にリリースを任せる by @tokuhirom in https://github.com/tokuhirom/ekanban/pull/40
