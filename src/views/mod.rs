mod board;
mod capture;

pub use board::BoardView;
pub(crate) use board::QuickCaptureState;
pub(crate) use board::{parse_theme_preference, window_title};
