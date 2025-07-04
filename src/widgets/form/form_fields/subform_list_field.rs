// tokio-tui/src/widgets/form/form_fields/subform_list_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph, Widget as _},
};
use serde::Serialize;

use crate::{ButtonsWidget, FormValue, FormWidget, SubFormData, TuiWidget as _};

use super::{FormFieldType, FormFieldWidget};

#[derive(Clone, Serialize, Debug, Default)]
pub struct TuiList<T: SubFormData + Serialize + std::fmt::Debug + Default>(pub Vec<T>);

impl<T: SubFormData + Serialize + std::fmt::Debug + Default> TuiList<T> {
    pub fn empty() -> Self {
        Self(vec![])
    }
}

// Implement FormValue for the SubFormListWrapper
impl<T: SubFormData + Serialize + std::fmt::Debug + Default> FormValue for TuiList<T> {
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget {
        let template_creator = || T::default().to_form_widget();

        let mut field = FormFieldWidget::subform_list(label, template_creator, required);

        if let FormFieldType::SubFormList(subform_list) = &mut field.inner {
            for item in &self.0 {
                subform_list.form_widgets.push(item.to_form_widget());
            }
        }

        field
    }

    fn from_field_widget(field: &FormFieldWidget) -> Self {
        match &field.inner {
            FormFieldType::SubFormList(subform_list) => {
                let mut result = Vec::with_capacity(subform_list.form_widgets.len());
                for form in &subform_list.form_widgets {
                    result.push(T::from_form_widget(form));
                }
                TuiList(result)
            }
            _ => TuiList(Vec::new()), // Fallback
        }
    }
}

// SubFormListField for Vec<SubForm> relationships
pub struct SubFormListField {
    pub form_widgets: Vec<FormWidget>,
    pub template_creator: Box<dyn Fn() -> FormWidget + Send + Sync>,
    pub selected_form: Option<usize>,
    pub active: bool,
    pub editing_index: Option<usize>,
    pub edit_buttons: ButtonsWidget,
}
impl FormFieldWidget {
    /// Creates a subform list field (Vec<SubForm> relationship)
    pub fn subform_list<F>(label: impl AsRef<str>, template_creator: F, required: bool) -> Self
    where
        F: Fn() -> FormWidget + Send + Sync + 'static,
    {
        Self {
            label: label.as_ref().to_string(),
            inner: FormFieldType::SubFormList(SubFormListField::new(template_creator)),
            required,
            help_text: None,
            is_focused: false,
        }
    }
}
impl std::fmt::Debug for SubFormListField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubFormListField")
            .field("items", &self.form_widgets.len())
            .field("selected", &self.selected_form)
            .field("active", &self.active)
            .field("editing_index", &self.editing_index)
            .field("action_buttons", &self.edit_buttons)
            .finish()
    }
}

impl SubFormListField {
    pub fn new<F>(template_creator: F) -> Self
    where
        F: Fn() -> FormWidget + Send + Sync + 'static,
    {
        Self {
            form_widgets: Vec::new(),
            template_creator: Box::new(template_creator),
            selected_form: None,
            active: false,
            editing_index: None,
            edit_buttons: ButtonsWidget::new()
                .add_button(
                    "Edit",
                    Style::default().fg(Color::Blue),
                    Style::default().fg(Color::Black).bg(Color::Blue),
                )
                .add_button(
                    "Delete",
                    Style::default().fg(Color::Red),
                    Style::default().fg(Color::Black).bg(Color::Red),
                )
                .with_padding(2),
        }
    }
    pub fn calculate_height(&self) -> u16 {
        if self.active {
            if let Some(idx) = self.editing_index {
                if idx < self.form_widgets.len() {
                    // When editing a specific form, calculate its full height
                    let nested_form = &self.form_widgets[idx];
                    let mut total_height = 3; // Base height

                    // Add height for each child field
                    for child_key in nested_form.keys() {
                        total_height += nested_form.calculate_field_height(child_key.as_str()) + 1;
                        // +1 for spacing
                    }

                    // Add height for the buttons area
                    total_height += 3;

                    total_height
                } else {
                    8 // Fallback height if index is invalid
                }
            } else {
                // When in navigation mode but not editing, show all forms with all fields
                let mut total_height = 0;

                for form in &self.form_widgets {
                    // Each form needs:
                    // 1 line for title
                    // 1 line per field
                    // 1 line for spacing
                    total_height += 1 + form.get_fields().len() as u16 + 1;
                }

                // Add 1 for the Add button
                if self.active {
                    total_height += 2;
                }

                // Add 1 for help text if any
                total_height += 1;

                // Minimum height of 3
                total_height.max(3)
            }
        } else {
            // When not active, still show all forms with all fields
            let mut total_height = 0;

            for form in &self.form_widgets {
                // Each form needs:
                // 1 line for title
                // 1 line per field
                // 1 line for spacing
                total_height += 1 + form.get_fields().len() as u16 + 1;
            }

            // Add 1 for help text if any
            total_height += 1;

            // Minimum height of 3
            total_height.max(3)
        }
    }
    pub fn get_value(&self) -> String {
        if self.form_widgets.is_empty() {
            return "[Empty]".to_string();
        }

        // Show a summary of all items
        let mut result = String::new();

        for (i, form) in self.form_widgets.iter().enumerate() {
            // Add separator between items
            if i > 0 {
                result.push_str(" | ");
            }

            // Add form title
            result.push_str(&form.title);

            // Try to add the first field value for context
            if let Some((key, field)) = form.get_fields().iter().next() {
                let value = field.get_value_as_string();
                if !value.is_empty() {
                    result.push_str(&format!(" ({key}={value})"));
                }
            }
        }

        result
    }

    pub fn is_valid(&self) -> bool {
        !self.form_widgets.is_empty()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn enter_start(&mut self) {
        self.enter();
        self.selected_form = if self.form_widgets.is_empty() {
            None
        } else {
            Some(0)
        }
    }
    pub fn enter_end(&mut self) {
        self.enter();
        self.selected_form = None;
    }

    fn select_up(&mut self) -> bool {
        if let Some(idx) = self.selected_form {
            if idx > 0 {
                self.selected_form = Some(idx - 1);
                self.focus_edit();
                true
            } else {
                self.unfocus_all();
                false
            }
        } else if !self.form_widgets.is_empty() {
            self.selected_form = Some(self.form_widgets.len() - 1);
            true
        } else {
            self.unfocus_all();
            false
        }
    }
    // Navigation methods (unchanged)
    fn select_down(&mut self) -> bool {
        if let Some(idx) = self.selected_form {
            if idx + 1 < self.form_widgets.len() {
                self.selected_form = Some(idx + 1);
                self.focus_edit();
            } else {
                // Move to Add button
                self.selected_form = None;
            }
            true
        } else {
            self.unfocus_all();
            false
        }
    }

    fn focus_edit(&mut self) {
        self.edit_buttons.focus();
        self.edit_buttons.set_selected(0);
    }

    fn focus_delete(&mut self) {
        self.edit_buttons.focus();
        self.edit_buttons.set_selected(1);
    }

    fn unfocus_all(&mut self) {
        for widget in self.form_widgets.iter_mut() {
            widget.unfocus();
        }
        self.selected_form = None;
        self.edit_buttons.unfocus();
        self.active = false;
    }

    fn start_editing(&mut self, idx: usize) {
        if idx < self.form_widgets.len() {
            self.editing_index = Some(idx);
            self.form_widgets[idx].focus();
            self.edit_buttons.unfocus();
        }
    }

    fn stop_editing(&mut self) {
        if let Some(idx) = self.editing_index {
            if idx < self.form_widgets.len() {
                self.form_widgets[idx].unfocus();
            }
        }
        self.editing_index = None;
    }

    fn delete_selected_item(&mut self) {
        if let Some(idx) = self.selected_form {
            if idx < self.form_widgets.len() {
                self.form_widgets.remove(idx);

                if self.form_widgets.is_empty() {
                    self.selected_form = None;
                } else if idx >= self.form_widgets.len() {
                    self.selected_form = Some(self.form_widgets.len() - 1);
                }

                self.edit_buttons.unfocus();
            }
        }
    }

    fn add_new_item(&mut self) {
        // Create a new form from the template
        let mut new_form = (self.template_creator)();

        // Make sure the form is properly focused before adding it
        new_form.focus();

        // Add the form to the list
        self.form_widgets.push(new_form);

        // Get the index of the newly added form
        let new_idx = self.form_widgets.len() - 1;

        // Update selection and start editing the new form
        self.selected_form = Some(new_idx);
        self.editing_index = Some(new_idx);

        // Ensure the edit buttons are unfocused so the form gets proper focus
        self.edit_buttons.unfocus();
    }

    // Key event handling (unchanged)
    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Previous implementation...
        if !self.active {
            return false;
        }

        // If we're currently editing a form
        if let Some(idx) = self.editing_index {
            if idx < self.form_widgets.len() {
                // If the form has submit buttons and Enter was pressed
                if key.code == KeyCode::Enter && self.form_widgets[idx].buttons_have_focus() {
                    let result = self.form_widgets[idx].key_event(key);

                    // After handling, check if we should exit edit mode
                    // This happens when a form button was clicked
                    self.stop_editing();
                    self.focus_edit();

                    return result;
                }

                // Pass the key to the form being edited
                if self.form_widgets[idx].key_event(key) {
                    return true;
                }

                // If Esc was pressed and not handled by form, exit edit mode
                if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
                    self.stop_editing();
                    return true;
                }
            }
            return false;
        }

        // Handle main navigation
        match key.code {
            KeyCode::Up => self.select_up(),
            KeyCode::Down => self.select_down(),
            KeyCode::Left => {
                if self.selected_form.is_some() {
                    self.focus_edit();
                    true
                } else {
                    self.edit_buttons.key_event(key)
                }
            }
            KeyCode::Right => {
                if self.selected_form.is_some() {
                    self.focus_delete();
                    true
                } else {
                    self.edit_buttons.key_event(key)
                }
            }
            KeyCode::Enter => {
                // If action buttons are focused
                if self.selected_form.is_some() {
                    let selected_button = self.edit_buttons.selected();

                    if selected_button == 0 && self.selected_form.is_some() {
                        // Edit button
                        if let Some(idx) = self.selected_form {
                            self.start_editing(idx);
                        }
                    } else if selected_button == 1 {
                        // Delete button
                        self.delete_selected_item();
                    }
                } else {
                    // Add button selected - create new item
                    self.add_new_item();
                }
                true
            }
            KeyCode::Delete => {
                self.delete_selected_item();
                true
            }
            _ => false,
        }
    }

    // Fully updated render method for SubFormListField
    pub fn render(&mut self, buf: &mut Buffer, area: Rect, block: Block<'_>) {
        // Render the block
        block.render(area, buf);

        let content_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        // If we're editing a form, render just that
        if let Some(idx) = self.editing_index {
            if idx < self.form_widgets.len() {
                self.form_widgets[idx].draw(content_area, buf);
                return;
            }
        }

        // When there are no items, just show empty state
        if self.form_widgets.is_empty() {
            Paragraph::new("[Empty]")
                .style(Style::default().fg(Color::White))
                .render(content_area, buf);

            // Show Add button if active
            if self.active {
                let add_style = if self.selected_form.is_none() {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default().fg(Color::Green)
                };

                Paragraph::new("+ Add").style(add_style).render(
                    Rect {
                        x: content_area.x,
                        y: content_area.y + 2,
                        width: content_area.width,
                        height: 1,
                    },
                    buf,
                );
            }
            return;
        }

        // For all other cases, always show ALL forms with their fields
        let mut current_y = content_area.y;
        let max_y = area.y + area.height - 1;

        for (form_idx, form) in self.form_widgets.iter().enumerate() {
            // Stop rendering if we run out of space
            if current_y >= max_y {
                break;
            }

            // Form header with special styling for selected item in navigation mode
            let is_selected = self.selected_form == Some(form_idx) && self.active;
            let title_style = if is_selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            // Form header
            Paragraph::new(format!("{}. {}", form_idx + 1, form.title))
                .style(title_style)
                .render(
                    Rect {
                        x: content_area.x,
                        y: current_y,
                        width: content_area
                            .width
                            .saturating_sub(if is_selected { 20 } else { 0 }), // Leave space for buttons if selected
                        height: 1,
                    },
                    buf,
                );

            // If item is selected in navigation mode, show action buttons
            if is_selected {
                let button_area = Rect {
                    x: content_area.x + 3,
                    y: current_y,
                    width: 19,
                    height: 1,
                };
                self.edit_buttons.draw(button_area, buf);
            }

            current_y += 1;

            // Use the field_keys vector to maintain proper field order
            for key in form.keys() {
                if let Some(field) = form.get_fields().get(key) {
                    // Check if we have space
                    if current_y >= max_y {
                        break;
                    }

                    // Get and display the field value
                    let value = field.get_value_as_string();
                    let field_text = format!("  {key}: {value}");

                    Paragraph::new(field_text)
                        .style(Style::default().fg(Color::Gray))
                        .render(
                            Rect {
                                x: content_area.x,
                                y: current_y,
                                width: content_area.width,
                                height: 1,
                            },
                            buf,
                        );

                    current_y += 1;
                }
            }

            // Add a blank line between forms if we have space
            if current_y < max_y {
                current_y += 1;
            }
        }

        // If in active navigation mode, always render the Add button at the bottom
        if self.active {
            // Only render if we have space
            if current_y < max_y {
                let add_style = if self.selected_form.is_none() {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default().fg(Color::Green)
                };

                Paragraph::new("+ Add").style(add_style).render(
                    Rect {
                        x: content_area.x,
                        y: current_y,
                        width: content_area.width,
                        height: 1,
                    },
                    buf,
                );
            }
        }
    }
    pub fn enter(&mut self) {
        self.active = true;
        if !self.form_widgets.is_empty() && self.selected_form.is_none() {
            self.selected_form = Some(0);
            self.focus_edit();
        }
    }

    pub fn leave(&mut self) {
        // If we're editing a form, exit edit mode first
        if self.editing_index.is_some() {
            self.stop_editing();
        } else {
            self.active = false;
            self.selected_form = None;
            self.edit_buttons.unfocus();
        }
    }
}
