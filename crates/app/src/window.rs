//! ウィンドウの矩形を覚えて、次の起動で戻す（`docs/DESIGN.md`「ウィンドウ」）。
//!
//! 置き場所は `app_state` の 1 行のままです。`tauri-plugin-window-state` は
//! 使いません——データの置き場所を 2 つに割らないためで、[ADR 0004] が決めた
//! 「1 データベース 1 プロセス」も、矩形だけ別のファイルに逃がすと崩れます。
//!
//! **判断の部分は Tauri を知りません。** 表示できる画面に載っているか、位置まで
//! 戻してよいかは、ただの計算として書いてあり、テストが窓を開けずに読めます。
//!
//! [ADR 0004]: ../../../docs/adr/0004-one-process-per-database.md

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use ekanban_core::db::{Database, WindowBoundsState};
use ekanban_core::diagnostics;
use tauri::{LogicalPosition, LogicalSize, Runtime, WebviewWindow};

/// 動かしている間は毎フレーム届く。静まるまで待って、最後の 1 つだけ書く。
const QUIET: Duration = Duration::from_millis(250);

/// 画面 1 枚ぶんの、使える範囲（論理ピクセル）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Display {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// 覚えていた矩形が、いまつながっている画面に丸ごと載るか。
///
/// 載らないなら使いません。外付けの画面を外したあとに、そこにあった位置で
/// 開くと、**ウィンドウが見えないまま起動します**。
pub(crate) fn fits_on_a_display(bounds: &WindowBoundsState, displays: &[Display]) -> bool {
    let left = f64::from(bounds.x);
    let top = f64::from(bounds.y);
    let right = left + f64::from(bounds.width);
    let bottom = top + f64::from(bounds.height);
    displays.iter().any(|display| {
        left >= display.left
            && right <= display.right
            && top >= display.top
            && bottom <= display.bottom
    })
}

/// 位置まで戻してよいか。
///
/// Wayland では戻しません。クライアントが自分の位置を知ることも決めることも
/// できないので、覚えている値は前のセッションのごみです。大きさは戻せます。
pub(crate) fn restores_position(wayland_display: Option<&str>, session_type: Option<&str>) -> bool {
    !(wayland_display.is_some()
        || session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland")))
}

fn restores_position_here() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    restores_position(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
    )
}

/// 覚えていた矩形でウィンドウを開き直す。
///
/// 失敗しても起動は止めません。既定の大きさで出るだけです。
pub(crate) fn restore<R: Runtime>(window: &WebviewWindow<R>, saved: Option<WindowBoundsState>) {
    let Some(saved) = saved else {
        return;
    };

    let displays = window
        .available_monitors()
        .map(|monitors| {
            monitors
                .iter()
                .map(|monitor| {
                    let scale = monitor.scale_factor();
                    let position = monitor.position().to_logical::<f64>(scale);
                    let size = monitor.size().to_logical::<f64>(scale);
                    Display {
                        left: position.x,
                        top: position.y,
                        right: position.x + size.width,
                        bottom: position.y + size.height,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // 画面の一覧が読めない環境（無いことがある）では、大きさだけ戻す。
    let on_a_display = !displays.is_empty() && fits_on_a_display(&saved, &displays);

    if let Err(error) = window.set_size(LogicalSize::new(
        f64::from(saved.width),
        f64::from(saved.height),
    )) {
        diagnostics::log(&format!("failed to restore the window size: {error}"));
    }
    if on_a_display && restores_position_here() {
        if let Err(error) =
            window.set_position(LogicalPosition::new(f64::from(saved.x), f64::from(saved.y)))
        {
            diagnostics::log(&format!("failed to restore the window position: {error}"));
        }
    }
}

/// 動いた・大きさが変わったを受けて、静まってから 1 回だけ書く。
///
/// ウィンドウを掴んで動かしている間、位置は毎フレーム変わります。届くたびに
/// SQLite に書くと、1 回の移動で数十のトランザクションになります。**最後の値
/// だけが要る**ので、静まるまで待ちます。
pub(crate) struct BoundsSaver {
    database_path: PathBuf,
    sender: Option<Sender<WindowBoundsState>>,
    thread: Option<JoinHandle<()>>,
    /// 最後に受けた矩形。終わるときに書ききるために持ちます。
    last: Mutex<Option<WindowBoundsState>>,
}

impl BoundsSaver {
    pub(crate) fn spawn(database_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel::<WindowBoundsState>();
        let path = database_path.clone();
        let thread = std::thread::spawn(move || {
            while let Ok(mut bounds) = receiver.recv() {
                // 静まるまで待つ。送り手が居なくなったときも、最後の値は書いてから
                // 終わる（`RecvTimeoutError` のどちらでもここを抜ける）。
                while let Ok(next) = receiver.recv_timeout(QUIET) {
                    bounds = next;
                }
                write(&path, bounds);
            }
        });
        Self {
            database_path,
            sender: Some(sender),
            thread: Some(thread),
            last: Mutex::new(None),
        }
    }

    pub(crate) fn record(&self, bounds: WindowBoundsState) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some(bounds);
        if let Some(sender) = &self.sender {
            let _ = sender.send(bounds);
        }
    }

    /// 終わる前に、最後の矩形をその場で書く。
    ///
    /// 待っている間にプロセスが終わると、そのぶんが落ちます。動かしてすぐ
    /// 終了したときに、位置が 1 回ぶん古いまま残るのを避けるためです。
    pub(crate) fn flush(&self) {
        let last = *self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(bounds) = last {
            write(&self.database_path, bounds);
        }
    }
}

fn write(database_path: &std::path::Path, bounds: WindowBoundsState) {
    if let Err(error) =
        Database::open(database_path).and_then(|database| database.set_window_bounds(bounds))
    {
        // 覚え損ねても操作は続けられる。黙って消さずに記録だけする。
        diagnostics::log(&format!("failed to remember the window bounds: {error}"));
    }
}

impl Drop for BoundsSaver {
    /// 終わるときに、まだ書いていない最後の値を書ききってから戻る。
    fn drop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// いまのウィンドウの矩形。フルスクリーンや最大化のときは覚えない。
///
/// 覚えると、次の起動が画面いっぱいのまま開き、**元の大きさに戻す手が無くなり
/// ます**。
pub(crate) fn current_bounds<R: Runtime>(window: &WebviewWindow<R>) -> Option<WindowBoundsState> {
    if window.is_fullscreen().unwrap_or(false)
        || window.is_maximized().unwrap_or(false)
        || window.is_minimized().unwrap_or(false)
    {
        return None;
    }
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(WindowBoundsState {
        x: position.x as f32,
        y: position.y as f32,
        width: size.width as f32,
        height: size.height as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    fn display() -> Display {
        Display {
            left: 0.,
            top: 0.,
            right: 1920.,
            bottom: 1080.,
        }
    }

    fn bounds(x: f32, y: f32) -> WindowBoundsState {
        WindowBoundsState {
            x,
            y,
            width: 800.,
            height: 600.,
        }
    }

    #[test]
    fn keeps_a_rectangle_that_lies_on_a_display() {
        assert!(fits_on_a_display(&bounds(100., 100.), &[display()]));
    }

    /// 外付けの画面を外したあとに、そこにあった位置で開かない。
    #[test]
    fn drops_a_rectangle_that_hangs_off_every_display() {
        assert!(!fits_on_a_display(&bounds(3000., 100.), &[display()]));
        assert!(!fits_on_a_display(&bounds(100., -400.), &[display()]));
    }

    /// 画面が 1 枚でも収まればよい。またぐ位置は落とす。
    #[test]
    fn accepts_a_rectangle_on_the_second_display() {
        let second = Display {
            left: 1920.,
            top: 0.,
            right: 3840.,
            bottom: 1080.,
        };
        assert!(fits_on_a_display(
            &bounds(2000., 100.),
            &[display(), second]
        ));
        assert!(
            !fits_on_a_display(&bounds(1800., 100.), &[display(), second]),
            "a rectangle straddling two displays is not restored"
        );
    }

    #[test]
    fn does_not_restore_the_position_on_wayland() {
        assert!(!restores_position(Some("wayland-0"), None));
        assert!(!restores_position(None, Some("wayland")));
        assert!(restores_position(None, Some("x11")));
        assert!(restores_position(None, None));
    }

    /// 動かしている間の値をまとめて、最後の 1 つだけが残ること。
    #[test]
    fn writes_the_rectangle_the_window_came_to_rest_at() {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("board.sqlite3");
        Database::open(&path).expect("a new database is created");

        {
            let saver = BoundsSaver::spawn(path.clone());
            for x in 0..20 {
                saver.record(bounds(x as f32, 0.));
            }
            // drop で書ききってから戻る。
        }

        let database = Database::open(&path).expect("the database reopens");
        assert_eq!(
            database.load_window_bounds().expect("the bounds are read"),
            Some(bounds(19., 0.)),
            "the last rectangle is the one that is remembered"
        );
    }
}
