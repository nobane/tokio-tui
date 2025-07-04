// tokio-tui/src/widgets/input/input_widget.rs
use std::path::PathBuf;

use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};

use crate::{TuiWidget, tui_theme};

pub struct InputWidget {
    input: String,
    cursor_position: usize,
    is_focused: bool,
    history: Vec<String>,
    history_index: usize,
    history_file: Option<PathBuf>,
    history_tx: Option<mpsc::UnboundedSender<String>>,
    hint: String,
    borders: Option<Borders>,
    border_tl_text: Option<String>,
    border_tr_text: Option<String>,
    text_style: Style,
    hint_style: Style,
    prefix_style: Style,
    prefix: String,
    suffix: String,
    submission: Option<String>,
    history_enabled: bool,
    needs_redraw: bool,
    last_area: Rect,
}

impl std::fmt::Debug for InputWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputBox")
            .field("input", &self.input)
            .field("cursor_position", &self.cursor_position)
            .field("is_focused", &self.is_focused)
            .field("history", &self.history)
            .field("history_index", &self.history_index)
            .field("history_file", &self.history_file)
            .field("history_tx", &self.history_tx)
            .field("hint", &self.hint)
            .field("borders", &self.borders)
            .field("border_tl_text", &self.border_tl_text)
            .field("border_tr_text", &self.border_tr_text)
            .field("text_style", &self.text_style)
            .field("hint_style", &self.hint_style)
            .field("prefix", &self.prefix)
            .field("suffix", &self.suffix)
            .finish()
    }
}

impl InputWidget {
    pub fn new() -> Self {
        Self {
            hint: String::new(),
            input: String::new(),
            cursor_position: 0,
            is_focused: false,
            history: Vec::new(),
            history_index: 0,
            history_file: None,
            history_tx: None,
            history_enabled: true,
            border_tl_text: None,
            border_tr_text: None,
            borders: Some(Borders::ALL),
            text_style: Style::default().fg(Color::White),
            hint_style: Style::default().fg(tui_theme::HINT_FG),
            prefix_style: Style::default().fg(Color::White),
            prefix: String::new(),
            suffix: String::new(),
            submission: None,
            needs_redraw: true,
            last_area: Rect::default(),
        }
    }

    pub fn take_submission(&mut self) -> Option<String> {
        let result = self.submission.take();
        if result.is_some() {
            self.redraw();
        }
        result
    }

    pub fn without_history(mut self) -> Self {
        self.history_enabled = false;
        self
    }

    pub async fn with_history_file(mut self, path: PathBuf) -> Self {
        self.history_enabled = true;
        self.history_file = Some(path.clone());
        self.load_history().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.history_tx = Some(tx);

        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                {
                    let _ = file.write_all(command.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            }
        });

        self
    }

    pub fn set_text(&mut self, text: impl AsRef<str>) {
        let new_text = text.as_ref().to_string();
        if self.input != new_text {
            self.input = new_text;
            self.cursor_position = self.input.len();
            self.redraw();
        }
    }

    pub fn focus_and_set_text(&mut self, text: impl AsRef<str>) {
        self.set_text(text);
        self.focus();
    }

    pub fn focus_and_clear(&mut self) {
        self.clear();
        self.focus();
    }

    pub fn set_tl_text(&mut self, text: impl AsRef<str>) {
        let new_text = text.as_ref().to_string();
        if self.border_tl_text.as_ref().is_none_or(|t| *t != new_text) {
            self.border_tl_text = Some(new_text);
            self.redraw();
        }
    }

    pub fn clear_tl_text(&mut self) {
        if self.border_tl_text.is_some() {
            self.border_tl_text = None;
            self.redraw();
        }
    }

    pub fn set_tr_text(&mut self, text: impl AsRef<str>) {
        let new_text = text.as_ref().to_string();
        if self.border_tr_text.as_ref().is_none_or(|t| *t != new_text) {
            self.border_tr_text = Some(new_text);
            self.redraw();
        }
    }

    pub fn clear_tr_text(&mut self) {
        if self.border_tr_text.is_some() {
            self.border_tr_text = None;
            self.redraw();
        }
    }

    pub fn set_border(&mut self, borders: Borders) -> &mut Self {
        if self.borders != Some(borders) {
            self.borders = Some(borders);
            self.redraw();
        }
        self
    }

    pub fn no_border(&mut self) -> &mut Self {
        if self.borders.is_some() {
            self.borders = None;
            self.redraw();
        }
        self
    }

    pub fn with_border(mut self, borders: Borders) -> Self {
        self.borders = Some(borders);
        self
    }

    pub fn without_border(mut self) -> Self {
        self.borders = None;
        self
    }

    pub fn with_text_style(mut self, style: Style) -> Self {
        self.text_style = style;
        self
    }

    pub fn set_text_style(&mut self, style: Style) {
        if self.text_style != style {
            self.text_style = style;
            self.redraw();
        }
    }

    pub fn with_prefix_style(mut self, style: Style) -> Self {
        self.prefix_style = style;
        self
    }

    pub fn set_prefix_style(&mut self, style: Style) {
        if self.prefix_style != style {
            self.prefix_style = style;
            self.redraw();
        }
    }

    pub fn with_hint_style(mut self, style: Style) -> Self {
        self.hint_style = style;
        self
    }

    pub fn set_hint_style(&mut self, style: Style) {
        if self.hint_style != style {
            self.hint_style = style;
            self.redraw();
        }
    }

    /// Returns the current text content of the input box
    pub fn text(&self) -> &str {
        &self.input
    }

    pub fn set_hint(&mut self, hint: impl AsRef<str>) {
        let new_hint = hint.as_ref().to_string();
        if self.hint != new_hint {
            self.hint = new_hint;
            self.redraw();
        }
    }

    pub fn with_hint(mut self, hint: impl AsRef<str>) -> Self {
        self.hint = hint.as_ref().to_string();
        self
    }

    pub fn hint(&self) -> &str {
        &self.hint
    }

    pub fn set_prefix(&mut self, prefix: impl AsRef<str>) {
        let new_prefix = prefix.as_ref().to_string();
        if self.prefix != new_prefix {
            self.prefix = new_prefix;
            self.redraw();
        }
    }

    pub fn with_prefix(mut self, prefix: impl AsRef<str>) -> Self {
        self.prefix = prefix.as_ref().to_string();
        self
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn set_suffix(&mut self, suffix: impl AsRef<str>) {
        let new_suffix = suffix.as_ref().to_string();
        if self.suffix != new_suffix {
            self.suffix = new_suffix;
            self.redraw();
        }
    }

    pub fn with_suffix(mut self, suffix: impl AsRef<str>) -> Self {
        self.suffix = suffix.as_ref().to_string();
        self
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    async fn load_history(&mut self) {
        if let Some(path) = &self.history_file {
            if let Ok(file) = File::open(path).await {
                let reader = BufReader::new(file);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    self.history.push(line);
                }
                self.history_index = self.history.len();
            }
        }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn clear(&mut self) {
        if !self.input.is_empty() {
            self.input.clear();
            self.cursor_position = 0;
            self.redraw();
        }
    }

    fn handle_enter(&mut self) {
        if !self.input.is_empty() && self.submission.is_none() {
            let input = self.input.clone();

            // Add to history
            self.history.push(input.clone());
            self.history_index = self.history.len();

            // Save to history file if enabled
            if let Some(tx) = self.history_tx.clone() {
                let _ = tx.send(input.clone());
            }

            // Invoke callback if set
            self.submission = Some(input);

            // Clear input
            self.clear();
            self.redraw();
        }
    }

    pub fn clear_and_unfocus(&mut self) {
        self.clear();
        self.unfocus();
    }

    pub fn redraw(&mut self) {
        self.needs_redraw = true;
    }
}

impl Default for InputWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiWidget for InputWidget {
    fn need_draw(&self) -> bool {
        self.needs_redraw
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        // Check if area changed
        if self.last_area != area {
            self.redraw();
        }
        self.last_area = area;

        // Create the content with prefix and suffix
        let base_style = if self.is_focused {
            self.text_style
        } else {
            self.text_style.fg(tui_theme::UNFOCUSED_FG)
        };
        let prefix_style = if self.is_focused {
            self.prefix_style
        } else {
            self.prefix_style.fg(tui_theme::UNFOCUSED_FG)
        };
        let cursor_style = base_style
            .bg(if self.is_focused {
                tui_theme::TEXT_FG
            } else {
                tui_theme::UNFOCUSED_FG
            })
            .fg(tui_theme::TEXT_BG);
        let mut spans = vec![Span::styled(&self.prefix, prefix_style)];

        let content = if self.input.is_empty() && !self.hint.is_empty() {
            // Show hint text with prefix/suffix
            if self.is_focused {
                spans.push(Span::styled(" ", cursor_style));
            }
            spans.push(Span::styled(&self.suffix, base_style));

            Line::from(spans)
        } else {
            // Show normal input text with prefix/suffix and cursor

            if self.is_focused {
                // Split the input at cursor position
                if self.cursor_position <= self.input.len() {
                    // Text before cursor
                    if self.cursor_position > 0 {
                        let before_cursor = &self.input[..self.cursor_position];
                        spans.push(Span::styled(before_cursor, base_style));
                    }

                    // Character at cursor (or space if at end)
                    if self.cursor_position < self.input.len() {
                        // Get single character at cursor position
                        let cursor_char = &self.input[self.cursor_position..=self.cursor_position];
                        spans.push(Span::styled(cursor_char, cursor_style));

                        // Text after cursor
                        if self.cursor_position + 1 < self.input.len() {
                            let after_cursor = &self.input[self.cursor_position + 1..];
                            spans.push(Span::styled(after_cursor, base_style));
                        }
                    } else {
                        // Cursor is at the end, show a highlighted space
                        spans.push(Span::styled(" ", cursor_style));
                    }
                }
            } else {
                // When not focused, just show the full text
                spans.push(Span::styled(&self.input, base_style));
            }

            spans.push(Span::styled(&self.suffix, base_style));
            Line::from(spans)
        };

        let mut block = Block::default();

        if let Some(border) = &self.borders {
            block = block
                .borders(*border)
                .border_style(Style::default().fg(if self.is_focused {
                    tui_theme::BORDER_FOCUSED
                } else {
                    tui_theme::BORDER_DEFAULT
                }));

            if let Some(tl_text) = &self.border_tl_text {
                block = block.title_top(Line::from(Span::raw(tl_text)).left_aligned());
            }

            if let Some(tr_text) = &self.border_tr_text {
                block = block.title_top(Line::from(Span::raw(tr_text)).right_aligned());
            }
        }

        // Render the paragraph with the block
        Paragraph::new(content).block(block).render(area, buf);

        // Reset the flag after rendering
        self.needs_redraw = false;
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if !self.is_focused {
            return false;
        }

        let mut handled = true;

        match key.code {
            KeyCode::Enter => {
                self.handle_enter();
            }
            KeyCode::Char(to_insert) => {
                self.input.insert(self.cursor_position, to_insert);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Left if self.cursor_position > 0 => {
                self.cursor_position -= 1;
            }
            KeyCode::Right if self.cursor_position < self.input.len() => {
                self.cursor_position += 1;
            }
            KeyCode::Up if self.history_enabled && self.history_index > 0 => {
                self.history_index -= 1;
                self.input = self.history[self.history_index].clone();
                self.cursor_position = self.input.len();
            }
            KeyCode::Down if self.history_enabled => {
                if self.history_index + 1 < self.history.len() {
                    self.history_index += 1;
                    self.input = self.history[self.history_index].clone();
                    self.cursor_position = self.input.len();
                } else if self.history_index > 0 {
                    self.history_index = 0;
                    self.clear();
                }
            }
            _ => {
                handled = false;
            }
        }

        if handled {
            self.redraw();
        }

        handled
    }

    fn focus(&mut self) {
        if !self.is_focused {
            self.is_focused = true;
            self.redraw();
        }
    }

    fn unfocus(&mut self) {
        if self.is_focused {
            self.is_focused = false;
            self.redraw();
        }
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
