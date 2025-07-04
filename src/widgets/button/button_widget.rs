// tokio-tui/src/widgets/button/button_widget.rs

use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget},
};

use crate::TuiWidget;

/// A widget for rendering and interacting with a row of buttons
pub struct ButtonsWidget {
    /// Buttons to display (text and style for each)
    buttons: Vec<(String, Style, Style)>,
    /// Currently selected button
    selected: usize,
    /// Whether the widget is focused
    is_focused: bool,
    /// Whether to use highlighting (background color) for selected button
    use_highlight: bool,
    /// Padding between buttons
    padding: u16,
    /// Callback for when a button is activated
    on_select: Option<Box<dyn Fn(usize) + Send + Sync>>,
}

impl std::fmt::Debug for ButtonsWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ButtonsWidget")
            .field("buttons", &self.buttons)
            .field("selected", &self.selected)
            .field("is_focused", &self.is_focused)
            .field("use_highlight", &self.use_highlight)
            .field("padding", &self.padding)
            .field("on_select", &self.on_select.is_some())
            .finish()
    }
}

impl ButtonsWidget {
    /// Create a new buttons widget
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            selected: 0,
            is_focused: false,
            use_highlight: true,
            padding: 4,
            on_select: None,
        }
    }

    /// Add a button with text and styles
    pub fn add_button(
        mut self,
        text: impl Into<String>,
        normal_style: Style,
        selected_style: Style,
    ) -> Self {
        self.buttons
            .push((text.into(), normal_style, selected_style));
        self
    }

    /// Set a callback for when a button is activated
    pub fn on_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        self.on_select = Some(Box::new(callback));
        self
    }

    /// Set padding between buttons
    pub fn with_padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Set whether to use background highlighting for selected button
    pub fn with_highlight(mut self, use_highlight: bool) -> Self {
        self.use_highlight = use_highlight;
        self
    }

    /// Set the selected button
    pub fn select(mut self, index: usize) -> Self {
        self.selected = index.min(self.buttons.len().saturating_sub(1));
        self
    }

    /// Get the number of buttons
    pub fn button_count(&self) -> usize {
        self.buttons.len()
    }

    /// Get the index of the selected button
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Set the selected button (mutable version)
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.buttons.len().saturating_sub(1));
    }

    /// Select the next button
    pub fn next_button(&mut self) {
        if !self.buttons.is_empty() {
            self.selected = (self.selected + 1) % self.buttons.len();
        }
    }

    /// Select the previous button
    pub fn prev_button(&mut self) {
        if !self.buttons.is_empty() {
            self.selected = if self.selected == 0 {
                self.buttons.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Trigger the callback with the selected button index
    pub fn trigger_selected(&self) {
        if let Some(callback) = &self.on_select {
            callback(self.selected);
        }
    }
}

impl Default for ButtonsWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiWidget for ButtonsWidget {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        if self.buttons.is_empty() {
            return;
        }

        // Calculate total width needed
        let button_widths: Vec<u16> = self
            .buttons
            .iter()
            .map(|(text, _, _)| text.len() as u16 + 2) // +2 for padding inside button
            .collect();

        let total_width: u16 =
            button_widths.iter().sum::<u16>() + (self.padding * (self.buttons.len() as u16 - 1));

        // Calculate starting x position to center the buttons
        let mut x = area.x + (area.width.saturating_sub(total_width) / 2);

        // Render each button
        for (i, (text, normal_style, selected_style)) in self.buttons.iter().enumerate() {
            let button_width = button_widths[i];
            let is_selected = i == self.selected;

            let style = if is_selected && self.is_focused {
                if self.use_highlight {
                    *selected_style
                } else {
                    *normal_style
                }
            } else {
                *normal_style
            };

            Paragraph::new(text.as_str())
                .style(style)
                .alignment(Alignment::Center)
                .render(
                    Rect {
                        x,
                        y: area.y,
                        width: button_width,
                        height: 1,
                    },
                    buf,
                );

            x += button_width + self.padding;
        }
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        match key.code {
            KeyCode::Left => {
                self.prev_button();
            }
            KeyCode::Right => {
                self.next_button();
            }
            KeyCode::Enter => {
                self.trigger_selected();
            }
            _ => return false,
        };
        true
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
