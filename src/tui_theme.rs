// tokio-tui/src/tui_theme.rs
use ratatui::style::Color;

#[cfg(windows)]
pub const THUMB_SYMBOL: &str = "▃";
#[cfg(not(windows))]
pub const THUMB_SYMBOL: &str = "🬋";

pub const BORDER_DEFAULT: Color = Color::Rgb(100, 100, 100);
pub const SCROLLBAR_DEFAULT: Color = Color::Rgb(200, 200, 200);
pub const BORDER_RESET: Color = Color::Rgb(101, 101, 101);
pub const BORDER_FOCUSED: Color = Color::Yellow;
pub const BORDER_ACTIVE: Color = Color::White;
pub const BORDER_UNFOCUSED: Color = Color::Rgb(70, 70, 70);
pub const SEARCH_HIGHLIGHT_COLOR: Color = Color::Rgb(240, 180, 0);
pub const CURRENT_MATCH_COLOR: Color = Color::Rgb(255, 100, 0);

pub const COLOR_ORANGE: Color = Color::Rgb(255, 165, 0);
pub const COLOR_PURPLE: Color = Color::Rgb(128, 0, 128);
pub const COLOR_PINK: Color = Color::Rgb(255, 192, 203);
pub const COLOR_BROWN: Color = Color::Rgb(165, 42, 42);
pub const COLOR_TEAL: Color = Color::Rgb(0, 128, 128);
pub const COLOR_LIME: Color = Color::Rgb(50, 205, 50);
pub const COLOR_INDIGO: Color = Color::Rgb(75, 0, 130);
pub const COLOR_GOLD: Color = Color::Rgb(255, 215, 0);
pub const COLOR_SILVER: Color = Color::Rgb(192, 192, 192);
pub const COLOR_NAVY: Color = Color::Rgb(0, 0, 128);
pub const COLOR_MAROON: Color = Color::Rgb(128, 0, 0);

pub const TEXT_FG: Color = Color::White;
pub const TEXT_BG: Color = Color::Black;
pub const ACTIVE_FG: Color = Color::Cyan;
pub const SELECTED_FG: Color = Color::Black;
pub const SELECTED_BG: Color = Color::Yellow;
pub const UNFOCUSED_FG: Color = Color::Rgb(170, 170, 170);
pub const HINT_FG: Color = Color::Rgb(70, 70, 70);

const HOUR: u8 = 120;
const MINUTE: u8 = 150;
const SEC: u8 = 180;

pub const HOUR_FG: Color = Color::Rgb(HOUR, HOUR, HOUR);
pub const MINUTE_FG: Color = Color::Rgb(MINUTE, MINUTE, MINUTE);
pub const SEC_FG: Color = Color::Rgb(SEC, SEC, SEC);

const GRAY_BASE: u8 = 30;
const GRAY_STEP: u8 = 30;
pub const GRAY0_FG: Color = Color::Rgb(GRAY_BASE, GRAY_BASE, GRAY_BASE);
pub const GRAY1_FG: Color = Color::Rgb(
    GRAY_BASE + GRAY_STEP,
    GRAY_BASE + GRAY_STEP,
    GRAY_BASE + GRAY_STEP,
);
pub const GRAY2_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 2),
    GRAY_BASE + (GRAY_STEP * 2),
    GRAY_BASE + (GRAY_STEP * 2),
);
pub const GRAY3_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 3),
    GRAY_BASE + (GRAY_STEP * 3),
    GRAY_BASE + (GRAY_STEP * 3),
);
pub const GRAY4_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 4),
    GRAY_BASE + (GRAY_STEP * 4),
    GRAY_BASE + (GRAY_STEP * 4),
);
pub const GRAY5_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 5),
    GRAY_BASE + (GRAY_STEP * 5),
    GRAY_BASE + (GRAY_STEP * 5),
);
pub const GRAY6_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 6),
    GRAY_BASE + (GRAY_STEP * 6),
    GRAY_BASE + (GRAY_STEP * 6),
);
pub const GRAY7_FG: Color = Color::Rgb(
    GRAY_BASE + (GRAY_STEP * 7),
    GRAY_BASE + (GRAY_STEP * 7),
    GRAY_BASE + (GRAY_STEP * 7),
);
