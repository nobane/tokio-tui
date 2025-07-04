// tokio-tui/src/widgets/form/form_fields/subform_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph, Widget as _},
};
use serde::Serialize;

use crate::{FormValue, FormWidget, SubFormData, TuiWidget as _};

use super::{FormFieldType, FormFieldWidget};

#[derive(Clone, Serialize, Debug, Default)]
pub struct TuiForm<T: SubFormData + Serialize + std::fmt::Debug + Default>(pub T);

impl<T: SubFormData + Serialize + std::fmt::Debug + Default> FormValue for TuiForm<T> {
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget {
        let form_widget = self.0.to_form_widget();
        FormFieldWidget::subform(label, form_widget, required)
    }

    fn from_field_widget(field: &FormFieldWidget) -> Self {
        match &field.inner {
            FormFieldType::SubForm(subform_field) => {
                TuiForm(T::from_form_widget(&subform_field.form_widget))
            }
            _ => TuiForm(T::default()), // Fallback
        }
    }
}
pub struct SubFormField {
    pub form_widget: FormWidget,
    pub active: bool,
}
impl FormFieldWidget {
    /// Creates a subform field (1:1 relationship)
    pub fn subform(label: impl Into<String>, form_widget: FormWidget, required: bool) -> Self {
        Self {
            label: label.into(),
            inner: FormFieldType::SubForm(SubFormField {
                form_widget,
                active: false,
            }),
            required,
            help_text: None,
            is_focused: false,
        }
    }
}

impl std::fmt::Debug for SubFormField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubFormField")
            .field("active", &self.active)
            .finish()
    }
}

// Implementation for SubFormField (1:1 relationship)
impl SubFormField {
    pub fn get_value(&self) -> String {
        format!("[{}]", self.form_widget.title.clone())
    }
    pub fn calculate_height(&self) -> u16 {
        if self.active {
            // When in full edit mode, calculate recursive height
            let nested_form = &self.form_widget;
            let mut total_height = 3; // Base height for the form container

            // Add height for each child field
            for child_key in nested_form.keys() {
                total_height += nested_form.calculate_field_height(child_key.as_str()) + 1;
                // +1 for spacing
            }

            // Add height for the buttons area
            total_height += 3;

            total_height
        } else {
            // When not in edit mode, calculate height for displaying all fields
            // Base height (2) for the field title and border
            let mut total_height = 1;

            // Add 1 line for each field in the subform
            total_height += self.form_widget.get_fields().len() as u16;

            // Add 1 for help text/hint
            total_height += 1;

            total_height
        }
    }
    pub fn is_valid(&self) -> bool {
        for field in self.form_widget.get_fields().values() {
            if field.required && !field.is_valid() {
                return false;
            }
        }
        true
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.form_widget.focus();
    }
    pub fn enter_start(&mut self) {
        self.form_widget.focus_start();
        self.enter();
    }
    pub fn enter_end(&mut self) {
        self.form_widget.focus_end();
        self.enter();
    }
    pub fn leave(&mut self) {
        self.active = false;
        self.form_widget.unfocus();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if self.active {
            if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
                self.leave();
                return true;
            }

            let handled = self.form_widget.key_event(key);

            match key.code {
                KeyCode::Up if !handled => {
                    self.leave();
                    false
                }
                KeyCode::Down if !handled => {
                    self.leave();
                    false
                }
                _ => handled,
            }
        } else {
            false
        }
    }
    // Updated render method for SubFormField
    pub fn render(&mut self, buf: &mut Buffer, area: Rect, block: Block<'_>) {
        // Render the block
        block.render(area, buf);

        let content_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if self.active {
            // When expanded and active, render the full form
            self.form_widget.draw(content_area, buf);
        } else {
            // Always show ALL fields and values
            let mut y_offset = 1;

            // Maintain field order using field_keys
            for key in self.form_widget.keys() {
                if let Some(field) = self.form_widget.get_fields().get(key) {
                    // Get field value
                    let value = field.get_value_as_string();

                    // Display field and value (no truncation)
                    let field_text = format!("{key}: {value}");

                    // Only render if we have space left
                    if content_area.y + y_offset < area.y + area.height - 1 {
                        Paragraph::new(field_text)
                            .style(Style::default().fg(Color::Gray))
                            .render(
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: 1,
                                },
                                buf,
                            );
                    }
                    y_offset += 1;
                }
            }
        }
    }
}
