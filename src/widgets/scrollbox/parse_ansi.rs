// tokio-tui/src/widgets/scrollbox/parse_ansi.rs
use ratatui::style::Modifier;

pub use ratatui::style::{Color, Style};

#[derive(Debug, Clone)]
pub struct StyledChar {
    pub ch: char,
    pub style: Style,
}

impl StyledChar {
    pub fn new(ch: char, style: Style) -> Self {
        Self { ch, style }
    }
}

impl<K: AsRef<char>> From<K> for StyledChar {
    fn from(value: K) -> Self {
        StyledChar {
            ch: *value.as_ref(),
            style: Style::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StyledText {
    pub chars: Vec<StyledChar>,
}

impl From<String> for StyledText {
    fn from(value: String) -> Self {
        Self::unstyled(value)
    }
}

impl From<&String> for StyledText {
    fn from(value: &String) -> Self {
        Self::unstyled(value)
    }
}

impl From<&str> for StyledText {
    fn from(value: &str) -> Self {
        Self::unstyled(value)
    }
}

// impl<K: AsRef<str>> From<K> for StyledText {
//     fn from(value: K) -> Self {
//         StyledText::default()
//             .append(value, Style::default())
//             .to_owned()
//     }
// }

impl From<StyledChar> for StyledText {
    fn from(val: StyledChar) -> Self {
        StyledText { chars: vec![val] }
    }
}

impl StyledText {
    pub fn unstyled(value: impl AsRef<str>) -> Self {
        StyledText::default()
            .append(value, Style::default())
            .to_owned()
    }
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn from_styled<K: AsRef<str>>(value: K, style: Style) -> Self {
        StyledText::default().append(value, style).to_owned()
    }
    pub fn append(&mut self, text: impl AsRef<str>, style: Style) -> &mut Self {
        for ch in text.as_ref().chars() {
            self.chars.push(StyledChar { ch, style });
        }
        self
    }
    pub fn append_default(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.append(text, Style::default())
    }
    pub fn append_option(&mut self, text: Option<impl AsRef<str>>, style: Style) -> &mut Self {
        if let Some(text) = text {
            for ch in text.as_ref().chars() {
                self.chars.push(StyledChar { ch, style });
            }
        }
        self
    }

    pub fn append_string(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.append(text, Style::default())
    }

    pub fn append_colored(&mut self, text: impl AsRef<str>, color: Color) -> &mut Self {
        self.append(text, Style::default().fg(color))
    }

    pub fn append_char(&mut self, ch: char, style: Style) -> &mut Self {
        self.chars.push(StyledChar { ch, style });
        self
    }

    pub fn append_space(&mut self) -> &mut Self {
        self.append_char(' ', Style::default())
    }
    pub fn append_spaces(&mut self, n: usize) -> &mut Self {
        for _ in 0..n {
            self.append_space();
        }
        self
    }

    pub fn append_formatted(
        &mut self,
        text: impl AsRef<str>,
        style_fn: impl Fn(char) -> Style,
    ) -> &mut Self {
        for ch in text.as_ref().chars() {
            self.chars.push(StyledChar {
                ch,
                style: style_fn(ch),
            });
        }
        self
    }

    pub fn append_text(&mut self, other: &StyledText) -> &mut Self {
        self.chars.extend_from_slice(&other.chars);
        self
    }
}

pub fn parse_ansi_string(s: impl AsRef<str>) -> StyledText {
    let mut chars = Vec::new();
    let mut current_style = Style::default();
    let mut i = 0;

    // Hyperlink state tracking
    let mut in_hyperlink = false;
    let hyperlink_style = Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED);
    let s = s.as_ref();
    while i < s.len() {
        // Check for OSC 8 hyperlinks
        if s[i..].starts_with("\x1b]8;") {
            if let Some(end_idx) = find_hyperlink_end(&s[i..]) {
                if s[i + 4..i + 6] == *";;" {
                    in_hyperlink = true;
                    current_style = hyperlink_style;
                } else if s[i + 4..i + 6] == *"\\\\" {
                    in_hyperlink = false;
                    current_style = Style::default();
                }
                i += end_idx;
                continue;
            }
        }

        // Check for ANSI escape sequences
        if s[i..].starts_with("\x1b[") {
            if let Some((end_idx, new_style)) = parse_sgr_sequence(&s[i..], current_style) {
                current_style = new_style;
                i += end_idx;
                continue;
            }
        }

        // Handle normal character
        let ch = if let Some(c) = s[i..].chars().next() {
            c
        } else {
            break;
        };

        chars.push(StyledChar {
            ch,
            style: if in_hyperlink {
                hyperlink_style
            } else {
                current_style
            },
        });
        i += ch.len_utf8();
    }

    StyledText { chars }
}

// Helper function to find the end of a hyperlink sequence
fn find_hyperlink_end(s: &str) -> Option<usize> {
    // Look for the end of a hyperlink sequence (either \x07 or \x1b\\)
    if let Some(bell_end) = s.find('\x07') {
        return Some(bell_end + 1);
    }

    if let Some(esc_end) = s.find("\x1b\\") {
        return Some(esc_end + 2);
    }

    None // No proper end found
}

// Parse an SGR (Select Graphic Rendition) sequence
fn parse_sgr_sequence(s: &str, current_style: Style) -> Option<(usize, Style)> {
    // s should start with ESC[
    if !s.starts_with("\x1b[") {
        return None;
    }

    // Find the end of the escape sequence (marked by 'm')
    let end = s.find('m')?;
    let params_str = &s[2..end]; // Skip ESC[ and exclude 'm'

    // Parse parameters
    let params: Vec<u16> = params_str
        .split(';')
        .filter_map(|p| p.parse::<u16>().ok())
        .collect();

    let mut new_style = current_style;

    // Process the parameters
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => new_style = Style::default(), // Reset all attributes
            1 => new_style = new_style.add_modifier(Modifier::BOLD),
            2 => new_style = new_style.add_modifier(Modifier::DIM),
            3 => new_style = new_style.add_modifier(Modifier::ITALIC),
            4 => new_style = new_style.add_modifier(Modifier::UNDERLINED),
            5 => new_style = new_style.add_modifier(Modifier::SLOW_BLINK),
            6 => new_style = new_style.add_modifier(Modifier::RAPID_BLINK),
            7 => new_style = new_style.add_modifier(Modifier::REVERSED),
            9 => new_style = new_style.add_modifier(Modifier::CROSSED_OUT),

            // Basic foreground colors (30-37)
            30..=37 => {
                new_style = new_style.fg(ansi_color_to_ratatui(params[i] - 30));
            }

            // Basic background colors (40-47)
            40..=47 => {
                new_style = new_style.bg(ansi_color_to_ratatui(params[i] - 40));
            }

            // 256-color foreground
            38 => {
                if i + 2 < params.len() {
                    match params[i + 1] {
                        // 8-bit color (256 colors)
                        5 => {
                            let color_idx = params[i + 2];
                            new_style = new_style.fg(ansi_256_to_ratatui(color_idx));
                            i += 2;
                        }
                        // 24-bit RGB color
                        2 => {
                            if i + 4 < params.len() {
                                let r = params[i + 2] as u8;
                                let g = params[i + 3] as u8;
                                let b = params[i + 4] as u8;
                                new_style = new_style.fg(Color::Rgb(r, g, b));
                                i += 4;
                            }
                        }
                        _ => {}
                    }
                }
            }

            // 256-color background
            48 => {
                if i + 2 < params.len() {
                    match params[i + 1] {
                        // 8-bit color (256 colors)
                        5 => {
                            let color_idx = params[i + 2];
                            new_style = new_style.bg(ansi_256_to_ratatui(color_idx));
                            i += 2;
                        }
                        // 24-bit RGB color
                        2 => {
                            if i + 4 < params.len() {
                                let r = params[i + 2] as u8;
                                let g = params[i + 3] as u8;
                                let b = params[i + 4] as u8;
                                new_style = new_style.bg(Color::Rgb(r, g, b));
                                i += 4;
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Bright foreground colors (90-97)
            90..=97 => {
                new_style = new_style.fg(ansi_color_to_ratatui((params[i] - 90) + 8));
            }

            // Bright background colors (100-107)
            100..=107 => {
                new_style = new_style.bg(ansi_color_to_ratatui((params[i] - 100) + 8));
            }

            _ => {}
        }
        i += 1;
    }

    Some((end + 1, new_style))
}

// Basic ANSI colors to Ratatui colors
fn ansi_color_to_ratatui(code: u16) -> Color {
    match code {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        8 => Color::Gray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::Gray,
        _ => Color::Reset,
    }
}

// 256-color to Ratatui color (simplified - more comprehensive would map all 256 colors)
fn ansi_256_to_ratatui(code: u16) -> Color {
    let code = code as u8;

    // Standard ANSI colors (0-15)
    if code < 16 {
        return ansi_color_to_ratatui(code as u16);
    }

    // 6×6×6 RGB color cube (16-231)
    if (16..=231).contains(&code) {
        let code = code - 16;
        let r = ((code / 36) % 6) * 51;
        let g = ((code / 6) % 6) * 51;
        let b = (code % 6) * 51;
        return Color::Rgb(r, g, b);
    }

    // Grayscale (232-255)
    if code >= 232 {
        let gray = (code - 232) * 10 + 8;
        return Color::Rgb(gray, gray, gray);
    }

    Color::Reset
}

pub enum EitherIter<T, I> {
    Single(std::iter::Once<T>),
    Multiple(I),
}

impl<T, I: Iterator<Item = T>> Iterator for EitherIter<T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EitherIter::Single(iter) => iter.next(),
            EitherIter::Multiple(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            EitherIter::Single(iter) => iter.size_hint(),
            EitherIter::Multiple(iter) => iter.size_hint(),
        }
    }
}

pub trait IntoEitherIter<T> {
    type Iter: Iterator<Item = T>;
    fn into_either_iter(self) -> Self::Iter;
}

impl<T> IntoEitherIter<T> for Vec<T> {
    type Iter = EitherIter<T, std::vec::IntoIter<T>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Multiple(self.into_iter())
    }
}

impl<T, const N: usize> IntoEitherIter<T> for [T; N] {
    type Iter = EitherIter<T, std::array::IntoIter<T, N>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Multiple(self.into_iter())
    }
}

impl<'a, T> IntoEitherIter<&'a T> for &'a [T] {
    type Iter = EitherIter<&'a T, std::slice::Iter<'a, T>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Multiple(self.iter())
    }
}

impl<T> IntoEitherIter<T> for std::collections::VecDeque<T> {
    type Iter = EitherIter<T, std::collections::vec_deque::IntoIter<T>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Multiple(self.into_iter())
    }
}

impl IntoEitherIter<String> for String {
    type Iter = EitherIter<String, std::iter::Empty<String>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Single(std::iter::once(self))
    }
}

impl<'a, T> IntoEitherIter<&'a T> for &'a T {
    type Iter = EitherIter<&'a T, std::iter::Empty<&'a T>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Single(std::iter::once(self))
    }
}

impl<'a> IntoEitherIter<&'a str> for &'a str {
    type Iter = EitherIter<&'a str, std::iter::Empty<&'a str>>;

    fn into_either_iter(self) -> Self::Iter {
        EitherIter::Single(std::iter::once(self))
    }
}

pub fn process_items<T: AsRef<str>>(items: impl IntoEitherIter<T>) {
    for item in items.into_either_iter() {
        println!("Processing: {}", item.as_ref());
    }
}
