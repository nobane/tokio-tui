// tokio-tui/src/widgets/form/form_widget.rs

use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};
use std::collections::HashMap;
use tracing::debug;

use crate::{tui_theme, ButtonsWidget, TuiWidget};

use super::{FormData, FormFieldType, FormFieldWidget};

pub type FormWidgetCallback = Box<dyn Fn(&mut FormWidget) + Send + Sync>;

pub struct FormWidget {
    pub title: String,
    fields: HashMap<String, FormFieldWidget>,
    // Store the keys in a Vec to maintain order for navigation
    field_keys: Vec<String>,
    border_style: Style,

    // Option<usize> where None means buttons are selected
    active_field_index: Option<usize>,

    is_focused: bool,
    on_cancel: Option<FormWidgetCallback>,
    on_submit: Option<FormWidgetCallback>,

    submit_buttons: ButtonsWidget,
    nested: bool,

    status: FormWidgetStatus,
}
#[derive(PartialEq, Eq)]
pub enum FormWidgetStatus {
    None,
    Submit,
    Cancel,
}

fn make_buttons(with_cancel: bool) -> ButtonsWidget {
    let mut buttons = ButtonsWidget::new();
    buttons = buttons.add_button(
        "Submit",
        Style::default().fg(Color::Green),
        Style::default().fg(Color::Black).bg(Color::Green),
    );
    if with_cancel {
        buttons = buttons.add_button(
            "Cancel",
            Style::default().fg(Color::Red),
            Style::default().fg(Color::Black).bg(Color::Red),
        );
    }
    buttons
}

impl FormWidget {
    pub fn new(title: impl Into<String>) -> Self {
        // Set up the Submit/Cancel buttons

        Self {
            title: title.into(),
            fields: HashMap::new(),
            field_keys: Vec::new(),
            border_style: Style::default().fg(tui_theme::BORDER_DEFAULT),
            active_field_index: None, // Buttons selected by default
            is_focused: false,
            on_cancel: None,
            on_submit: None,
            submit_buttons: make_buttons(false),
            nested: false,
            status: FormWidgetStatus::None,
        }
    }

    pub fn new_nested() -> Self {
        let mut nested_form = Self::new("");
        nested_form.nested = true;
        nested_form
    }

    // Cancels the form
    fn cancel_form(&mut self) {
        if let Some(callback) = self.on_cancel.take() {
            callback(self);

            self.on_cancel = Some(callback)
        }
        if !self.nested {
            self.status = FormWidgetStatus::Cancel
        }
    }

    // Submit the form
    fn submit_form(&mut self) {
        if let Some(callback) = self.on_submit.take() {
            callback(self);

            self.on_submit = Some(callback);
        }

        if !self.nested {
            self.status = FormWidgetStatus::Submit
        }
    }

    pub fn reset_submit(&mut self) -> bool {
        if self.status == FormWidgetStatus::Submit {
            self.status = FormWidgetStatus::None;
            true
        } else {
            false
        }
    }

    pub fn reset_closed(&mut self) -> bool {
        if self.status != FormWidgetStatus::None {
            self.status = FormWidgetStatus::None;
            true
        } else {
            false
        }
    }

    pub fn keys(&self) -> &Vec<String> {
        &self.field_keys
    }

    pub fn field_mut(&mut self, idx: usize) -> Option<&mut FormFieldWidget> {
        self.field_keys
            .get(idx)
            .and_then(|key| self.fields.get_mut(key))
    }

    pub fn field_ref(&mut self, idx: usize) -> Option<&FormFieldWidget> {
        self.field_keys
            .get(idx)
            .and_then(|key| self.fields.get(key))
    }

    pub fn active_mut(&mut self) -> Option<&mut FormFieldWidget> {
        self.active_field_index.and_then(|idx| self.field_mut(idx))
    }

    pub fn active_ref(&mut self) -> Option<&FormFieldWidget> {
        self.active_field_index.and_then(|idx| self.field_ref(idx))
    }

    pub fn buttons_have_focus(&self) -> bool {
        self.active_field_index.is_none() && self.submit_buttons.is_focused()
    }

    // Initialize the form with a FormData struct
    pub fn with_data<T: FormData>(mut self, data: &T) -> Self {
        self.fields = data.to_fields();
        self.field_keys = T::field_definitions()
            .iter()
            .map(|def| def.id.to_string())
            .collect();
        self.active_field_index = if self.field_keys.is_empty() {
            None
        } else {
            Some(0)
        };
        self
    }
    pub fn with_default<T: FormData>(mut self) -> Self {
        let data = T::default();
        self.fields = data.to_fields();
        self.field_keys = T::field_definitions()
            .iter()
            .map(|def| def.id.to_string())
            .collect();
        self.active_field_index = if self.field_keys.is_empty() {
            None
        } else {
            Some(0)
        };
        self
    }

    // Sets the fields for this form using a HashMap
    pub fn with_fields(mut self, fields: HashMap<String, FormFieldWidget>) -> Self {
        self.field_keys = fields.keys().cloned().collect();
        self.fields = fields;
        self
    }

    // Sets the callback for when the form is cancelled
    pub fn with_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut FormWidget) + Send + Sync + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
        self.submit_buttons = make_buttons(true);
        self
    }

    // Sets the callback for when the form is submitted
    pub fn with_submit<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut FormWidget) + Send + Sync + 'static,
    {
        self.on_submit = Some(Box::new(callback));
        self
    }

    // Sets the fields in this form
    pub fn set_fields(&mut self, fields: HashMap<String, FormFieldWidget>) {
        self.field_keys = fields.keys().cloned().collect();
        self.fields = fields;
        self.active_field_index = None; // Reset to buttons
    }

    // Sets the form data
    pub fn set_data<T: FormData>(&mut self, data: &T) {
        self.fields = data.to_fields();
        self.field_keys = T::field_definitions()
            .iter()
            .map(|def| def.id.to_string())
            .collect();
        self.active_field_index = None; // Reset to buttons
    }

    // Returns a clone of the current fields in the form
    pub fn get_fields(&self) -> &HashMap<String, FormFieldWidget> {
        &self.fields
    }

    // Get the form data
    pub fn get_data<T: FormData>(&self) -> T {
        T::from_fields(&self.fields)
    }

    // Get field value by key
    pub fn get_field(&self, key: &str) -> Option<&FormFieldWidget> {
        self.fields.get(key)
    }

    // Update border style based on focus
    fn update_border_style(&mut self) {
        self.border_style = Style::default().fg(if self.is_focused {
            tui_theme::BORDER_FOCUSED
        } else {
            tui_theme::BORDER_DEFAULT
        });
    }

    // Unfocus all fields
    fn unfocus_all(&mut self) {
        for field in self.fields.values_mut() {
            field.unfocus();
        }
        self.submit_buttons.unfocus();
    }

    // Get the index of the currently active field (if any)
    fn active_field(&self) -> Option<usize> {
        for (i, key) in self.field_keys.iter().enumerate() {
            if let Some(field) = self.fields.get(key) {
                if field.is_focused() {
                    return Some(i);
                }
            }
        }
        None
    }
    /// Check if any field in this form is currently active (in edit mode)
    pub fn has_active_fields(&self) -> bool {
        for field in self.fields.values() {
            if field.is_active() {
                return true;
            }
        }
        false
    }
    // Calculate the height needed for a field
    pub fn calculate_field_height(&self, field_key: &str) -> u16 {
        match self.fields.get(field_key) {
            Some(field) => match &field.inner {
                FormFieldType::Text(field) => field.calculate_height(),
                FormFieldType::Select(field) => field.calculate_height(),
                FormFieldType::List(field) => field.calculate_height(),
                FormFieldType::SubForm(field) => field.calculate_height(),
                FormFieldType::SubFormList(field) => field.calculate_height(),
            },
            None => 0, // Default height if field not found
        }
    }
    fn activate_prev(&mut self) -> bool {
        self.unfocus_all();

        debug!(
            "FormWidget activate_prev start {:?}",
            self.active_field_index
        );

        if let Some(idx) = self.active_field_index {
            if idx > 0 {
                self.active_field_index = Some(idx - 1);
                if let Some(field) = self.active_mut() {
                    field.inner_mut().enter_start();
                }
            } else {
                self.active_field_index = None;
                self.submit_buttons.focus();
            };
            true
        } else if !self.fields.is_empty() {
            self.active_field_index = Some(self.fields.len() - 1);
            if let Some(field) = self.active_mut() {
                field.inner_mut().enter_start();
            }
            true
        } else {
            !self.nested
        }
    }
    fn activate_next(&mut self) -> bool {
        self.unfocus_all();

        if let Some(idx) = self.active_field_index {
            if idx + 1 < self.field_keys.len() {
                self.active_field_index = Some(idx + 1);
                if let Some(field) = self.active_mut() {
                    field.inner_mut().enter_end();
                }
            } else {
                self.active_field_index = None;
            }
            true
        } else if !self.field_keys.is_empty() {
            self.active_field_index = Some(0);
            if let Some(field) = self.active_mut() {
                field.inner_mut().enter_end();
            }
            true
        } else {
            !self.nested
        }
    }
    fn apply_focus(&mut self) {
        self.unfocus_all();

        // When form gets focus, either focus the button widget or selected field
        if let Some(field) = self.active_mut() {
            field.focus();
        } else {
            self.submit_buttons.focus();
        }
    }

    pub fn focus_start(&mut self) {
        self.active_field_index = if !self.field_keys.is_empty() {
            Some(0)
        } else {
            None
        };
        self.apply_focus();
    }
    pub fn focus_end(&mut self) {
        self.active_field_index = if !self.field_keys.is_empty() {
            Some(self.field_keys.len() - 1)
        } else {
            None
        };
        self.apply_focus();
    }
}

impl TuiWidget for FormWidget {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.update_border_style();

        // Calculate inner area for form content
        let inner_area = if self.nested {
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height,
            }
        } else {
            // Create outer block
            let block = Block::default()
                .title(self.title.clone())
                .borders(Borders::ALL)
                .border_style(self.border_style);

            // Render outer block
            block.render(area, buf);
            Rect {
                x: area.x + 2,
                y: area.y + 2,
                width: area.width.saturating_sub(4),
                height: area.height.saturating_sub(4),
            }
        };

        // Calculate heights for all fields
        let mut field_positions = Vec::new();
        let mut current_y = inner_area.y;
        let button_height = 3; // Space reserved for buttons at bottom

        // First pass: calculate positions and heights
        for key in &self.field_keys {
            let height = self.calculate_field_height(key);
            field_positions.push((current_y, height));
            current_y += height + 1; // Add 1 for spacing between fields
        }

        // Determine visible fields based on height constraints
        let buttons_y = inner_area.y + inner_area.height - button_height;
        let mut visible_field_indices = Vec::new();

        // Find the range of visible fields
        let mut first_visible = 0;

        // If selected field would be off-screen, adjust the scroll position
        if let Some(selected_idx) = self.active_field_index {
            if selected_idx < self.field_keys.len() {
                let (selected_y, selected_height) = field_positions[selected_idx];

                // If selected field is above visible area, scroll up
                if selected_y < inner_area.y {
                    // Find how many fields to scroll up
                    while first_visible < selected_idx {
                        first_visible += 1;
                    }
                }

                // If selected field is below visible area or doesn't fit, scroll down
                let selected_bottom = selected_y + selected_height;
                if selected_bottom > buttons_y {
                    // Scroll down until the selected field fits
                    while first_visible < selected_idx && selected_y + selected_height > buttons_y {
                        first_visible += 1;
                    }
                }
            }
        }

        // Determine which fields are visible
        let mut current_y = inner_area.y;
        for i in first_visible..self.field_keys.len() {
            let height = self.calculate_field_height(&self.field_keys[i]);

            // Check if field fits in the visible area
            if current_y + height <= buttons_y {
                visible_field_indices.push(i);
                current_y += height + 1;
            } else {
                break;
            }
        }

        // When rendering fields, don't pass tabs_widget for select fields
        for &field_idx in &visible_field_indices {
            let (y_pos, height) = field_positions[field_idx];
            let y = y_pos - (field_positions[first_visible].0 - inner_area.y);

            if let Some(field) = self.field_mut(field_idx) {
                let field_area = Rect {
                    x: inner_area.x,
                    y,
                    width: inner_area.width,
                    height,
                };

                // Render field
                field.render(buf, field_area, None);
            }
        }

        // Update button selection based on current mode
        if self.active_field_index.is_none() {
            self.submit_buttons.focus();
        } else {
            self.submit_buttons.unfocus();
        }

        if !self.nested {
            // Render buttons at the bottom
            self.submit_buttons.draw(
                Rect {
                    x: inner_area.x,
                    y: buttons_y,
                    width: inner_area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        // Handle escape key specially - it should always move "up" one level
        if key.code == KeyCode::Esc {
            // If any field is active (inner editing mode), exit that mode first
            for field in self.fields.values_mut() {
                if field.is_active() {
                    field.leave();
                    return true;
                }
            }

            // If a field is focused but not active, unfocus it
            let active_field = self.active_field();
            if active_field.is_some() {
                self.unfocus_all();
                return true;
            }

            // Otherwise, escape from the form itself
            self.cancel_form();
            return true;
        }

        // If a field is active, pass keys to it first
        if let Some(field) = self.active_mut() {
            let handled = field.handle_key_event(key);
            match key.code {
                KeyCode::Up if !handled => {
                    if let Some(field) = self.active_mut() {
                        if !field.handle_key_event(key) {
                            return self.activate_prev();
                        } else {
                            true
                        }
                    } else {
                        false
                    }
                }
                KeyCode::Down => {
                    if let Some(field) = self.active_mut() {
                        if !field.handle_key_event(key) {
                            return self.activate_next();
                        } else {
                            true
                        }
                    } else {
                        false
                    }
                }
                _ => handled,
            };
            if handled {
                return true;
            }
        }

        match key.code {
            KeyCode::Up => self.activate_prev(),
            KeyCode::Down => self.activate_next(),
            KeyCode::Tab => self.activate_next(),
            KeyCode::BackTab => self.activate_prev(),
            KeyCode::Enter => {
                // Activate the currently focused field
                if let Some(field) = self.active_mut() {
                    field.enter();
                } else {
                    match self.submit_buttons.selected() {
                        0 => self.submit_form(), // Submit button
                        1 => self.cancel_form(),
                        _ => {}
                    }
                }
                true
            }
            _ => return self.submit_buttons.key_event(key),
        };
        true
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
        self.unfocus_all();
        self.submit_buttons.unfocus();
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
