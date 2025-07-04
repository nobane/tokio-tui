// tokio-tui/src/widgets/form/form_fields/text_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph, Widget},
};

use crate::{tui_theme, InputWidget, TuiWidget};

use super::{FormFieldType, FormFieldWidget};

#[derive(Debug)]
pub struct TextFormField {
    pub value: String,
    pub input_box: InputWidget,
    pub max_length: Option<usize>,
}

impl FormFieldWidget {
    /// Creates a new text input field
    pub fn text(label: impl Into<String>, value: impl Into<String>, required: bool) -> Self {
        Self {
            label: label.into(),
            inner: FormFieldType::Text(TextFormField {
                input_box: InputWidget::new().without_history(),
                value: value.into(),
                max_length: None,
            }),
            required,
            help_text: None,
            is_focused: false,
        }
    }

    /// Creates a new text input field with a maximum length
    pub fn text_with_max_length(
        label: impl Into<String>,
        value: impl Into<String>,
        max_length: usize,
        required: bool,
    ) -> Self {
        Self {
            label: label.into(),
            inner: FormFieldType::Text(TextFormField {
                input_box: InputWidget::new(),
                value: value.into(),
                max_length: Some(max_length),
            }),
            required,
            help_text: None,
            is_focused: false,
        }
    }
}

// Implementations for the field type structs
impl TextFormField {
    pub fn get_value(&self) -> String {
        self.value.clone()
    }

    pub fn is_valid(&self) -> bool {
        !self.value.trim().is_empty()
    }

    pub fn enter(&mut self) {
        self.input_box.focus_and_set_text(&self.value);
    }

    pub fn leave(&mut self) {
        // Save current value before unfocusing
        if self.input_box.is_focused() {
            self.value = self.input_box.text().to_string();
            if let Some(max) = self.max_length {
                self.value = self.value.chars().take(max).collect();
            }
        }
        self.input_box.unfocus();
    }

    pub fn is_active(&self) -> bool {
        self.input_box.is_focused()
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                if self.input_box.is_focused() {
                    // Complete text editing
                    self.value = self.input_box.text().to_string();
                    if let Some(max) = self.max_length {
                        self.value = self.value.chars().take(max).collect();
                    }
                    self.input_box.unfocus();
                    return true;
                }
                false
            }
            _ => {
                // Pass other keys to the input box
                self.input_box.key_event(key)
            }
        }
    }

    pub fn render(&mut self, buf: &mut Buffer, area: Rect, block: Block<'_>) {
        // Render the block
        block.render(area, buf);

        // Calculate content area
        let content_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: 1,
        };

        // Handle value rendering
        if self.input_box.is_focused() {
            // When editing, use the InputBox widget directly
            self.input_box.no_border();
            self.input_box.draw(content_area, buf);
        } else {
            // Normal rendering when not editing
            let value_style = if self.is_active() {
                Style::default().fg(tui_theme::BORDER_FOCUSED)
            } else {
                Style::default().fg(tui_theme::TEXT_FG)
            };

            Paragraph::new(self.value.as_str())
                .style(value_style)
                .render(content_area, buf);
        }
    }

    pub fn calculate_height(&self) -> u16 {
        3
    }
}
