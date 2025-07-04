// tokio-tui/src/widgets/form/form_fields/form_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders},
};

use crate::{tui_theme, TabsWidget};

use super::{ListField, SelectFormField, SubFormField, SubFormListField, TextFormField};

/// Represents a field in the form with its label and type
#[derive(Debug)]
pub struct FormFieldWidget {
    pub label: String,
    pub inner: FormFieldType,
    pub required: bool,
    pub help_text: Option<String>,
    pub is_focused: bool,
}

#[derive(Debug)]
pub enum FormFieldType {
    Text(TextFormField),
    Select(SelectFormField),
    List(ListField),
    SubForm(SubFormField),         // For 1:1 nested form
    SubFormList(SubFormListField), // For Vec<SubForm>
}

impl FormFieldWidget {
    /// Adds help text to this field
    pub fn with_help_text(mut self, text: impl Into<String>) -> Self {
        self.help_text = Some(text.into());
        self
    }

    // In the get_value_as_string method
    pub fn get_value_as_string(&self) -> String {
        self.inner.get_value_as_string()
    }

    // In the is_valid method
    pub fn is_valid(&self) -> bool {
        if !self.required {
            return true;
        }

        self.inner.is_valid()
    }

    pub fn inner(&self) -> &FormFieldType {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut FormFieldType {
        &mut self.inner
    }

    // In the enter method
    pub fn enter(&mut self) {
        self.inner.enter();
    }

    // In the leave method
    pub fn leave(&mut self) {
        self.inner.leave();
    }

    // In the is_active method
    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    /// Focuses this field and prepares it for editing
    pub fn focus(&mut self) {
        self.is_focused = true;
        self.enter();
    }

    /// Unfocuses this field
    pub fn unfocus(&mut self) {
        self.is_focused = false;
        self.leave();
    }

    /// Returns whether this field is focused
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    // In the handle_key_event method
    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        // If Escape is pressed and we're in an active inner widget
        if key.code == KeyCode::Esc && self.is_active() {
            self.leave();
            return true;
        }

        // If Enter is pressed and we're focused but not active
        if key.code == KeyCode::Enter && self.is_focused() && !self.is_active() {
            self.enter();
            return true;
        }

        // Pass the event to the inner field if active
        if self.is_active() {
            self.inner.handle_key_event(key)
        } else {
            false
        }
    }

    pub fn render(&mut self, buf: &mut Buffer, area: Rect, _tabs_widget: Option<&mut TabsWidget>) {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.is_focused {
                Style::default().fg(tui_theme::BORDER_FOCUSED)
            } else {
                Style::default().fg(tui_theme::BORDER_DEFAULT)
            });

        // Add label to top-left of block
        let mut label = self.label.clone();
        if !self.required {
            label.push_str(" [optional]");
        }
        block = block.title_top(Line::from(Span::raw(label)).left_aligned());

        match &mut self.inner {
            FormFieldType::Text(field) => field.render(buf, area, block),
            FormFieldType::Select(field) => field.render(buf, area, block),
            FormFieldType::List(field) => field.render(buf, area, block),
            FormFieldType::SubForm(field) => field.render(buf, area, block),
            FormFieldType::SubFormList(field) => field.render(buf, area, block),
        }
    }
}

impl FormFieldType {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match self {
            FormFieldType::Text(field) => field.handle_key_event(key),
            FormFieldType::Select(field) => field.handle_key_event(key),
            FormFieldType::List(field) => field.handle_key_event(key),
            FormFieldType::SubForm(field) => field.handle_key_event(key),
            FormFieldType::SubFormList(field) => field.handle_key_event(key),
        }
    }
    // In the get_value_as_string method
    pub fn get_value_as_string(&self) -> String {
        match self {
            FormFieldType::Text(field) => field.get_value(),
            FormFieldType::Select(field) => field.get_value(),
            FormFieldType::List(field) => field.get_value(),
            FormFieldType::SubForm(field) => field.get_value(),
            FormFieldType::SubFormList(field) => field.get_value(),
        }
    }

    // In the is_valid method
    pub fn is_valid(&self) -> bool {
        match self {
            FormFieldType::Text(field) => field.is_valid(),
            FormFieldType::Select(field) => field.is_valid(),
            FormFieldType::List(field) => field.is_valid(),
            FormFieldType::SubForm(field) => field.is_valid(),
            FormFieldType::SubFormList(field) => field.is_valid(),
        }
    }

    // In the enter method
    pub fn enter_end(&mut self) {
        match self {
            FormFieldType::Text(field) => field.enter(),
            FormFieldType::Select(field) => field.enter(),
            FormFieldType::List(field) => field.enter_end(),
            FormFieldType::SubForm(field) => field.enter_end(),
            FormFieldType::SubFormList(field) => field.enter_end(),
        }
    }

    // In the enter method
    pub fn enter_start(&mut self) {
        match self {
            FormFieldType::Text(field) => field.enter(),
            FormFieldType::Select(field) => field.enter(),
            FormFieldType::List(field) => field.enter_start(),
            FormFieldType::SubForm(field) => field.enter_start(),
            FormFieldType::SubFormList(field) => field.enter_start(),
        }
    }

    // In the enter method
    pub fn enter(&mut self) {
        match self {
            FormFieldType::Text(field) => field.enter(),
            FormFieldType::Select(field) => field.enter(),
            FormFieldType::List(field) => field.enter(),
            FormFieldType::SubForm(field) => field.enter(),
            FormFieldType::SubFormList(field) => field.enter(),
        }
    }

    // In the leave method
    pub fn leave(&mut self) {
        match self {
            FormFieldType::Text(field) => field.leave(),
            FormFieldType::Select(field) => field.leave(),
            FormFieldType::List(field) => field.leave(),
            FormFieldType::SubForm(field) => field.leave(),
            FormFieldType::SubFormList(field) => field.leave(),
        }
    }

    // In the is_active method
    pub fn is_active(&self) -> bool {
        match self {
            FormFieldType::Text(field) => field.is_active(),
            FormFieldType::Select(field) => field.is_open(),
            FormFieldType::List(field) => field.is_active(),
            FormFieldType::SubForm(field) => field.is_active(),
            FormFieldType::SubFormList(field) => field.is_active(),
        }
    }
}
