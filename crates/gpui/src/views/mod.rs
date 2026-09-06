mod board;
mod capture;
mod description_links;
mod menu_bar;
mod window_chrome;

pub use board::BoardView;
pub(crate) use board::{parse_theme_preference, window_title};
pub(crate) use board::{CaptureTarget, QuickCaptureState, ThemePreference};
