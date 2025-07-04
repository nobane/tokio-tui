// tokio-tui/src/widgets/form/form_fields/list_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph, Widget},
};

use crate::{ButtonsWidget, InputWidget, TuiWidget};

use super::{FormFieldType, FormFieldWidget};

#[derive(Debug)]
pub struct ListField {
    pub input_box: InputWidget,
    pub items: Vec<String>,
    pub selected: Option<usize>, // Selected item index or None for Add button
    pub active: bool,            // Whether the list field is in active mode
    pub action: ListAction,      // Current action (None, Edit, Delete, Add)
    pub action_buttons: ButtonsWidget, // Buttons for item actions
    pub max_display: Option<usize>, // Maximum number of items to display when not active
}

#[derive(Debug, PartialEq)]
pub enum ListAction {
    None,
    Edit,
    Add,
}
impl FormFieldWidget {
    /// Creates a string list field
    pub fn string_list(label: impl Into<String>, items: Vec<String>, required: bool) -> Self {
        Self {
            label: label.into(),
            inner: FormFieldType::List(ListField {
                input_box: InputWidget::new(),
                items,
                selected: None,
                action: ListAction::None,
                active: false,
                action_buttons: ButtonsWidget::new()
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
                max_display: None,
            }),
            required,
            help_text: None,
            is_focused: false,
        }
    }
}

impl Default for ListField {
    fn default() -> Self {
        Self {
            input_box: InputWidget::new(),
            items: Vec::new(),
            selected: None,
            active: false,
            action: ListAction::None,
            action_buttons: ButtonsWidget::new()
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
            max_display: None,
        }
    }
}

impl ListField {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_display(mut self, max: usize) -> Self {
        self.max_display = Some(max);
        self
    }

    pub fn get_value(&self) -> String {
        if self.items.is_empty() {
            String::new()
        } else {
            self.items.join(", ")
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn calculate_height(&self) -> u16 {
        self.items.len() as u16 + if self.active { 3 } else { 2 }
    }

    pub fn enter(&mut self) {
        // When entering, become active and select the first item or Add if empty
        self.active = true;

        if !self.items.is_empty() {
            self.selected = Some(0);
        } else {
            self.selected = None; // None means Add button
        }
        self.action = ListAction::None;
        self.action_buttons.unfocus();
    }
    pub fn enter_start(&mut self) {
        self.enter();
        self.selected = if self.items.is_empty() { None } else { Some(0) }
    }
    pub fn enter_end(&mut self) {
        self.enter();
        self.selected = None;
    }
    pub fn leave(&mut self) {
        // When leaving, reset all state
        self.active = false;
        self.selected = None;
        self.action = ListAction::None;
        self.input_box.unfocus();
        self.action_buttons.unfocus();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    fn focus_edit(&mut self) {
        self.action_buttons.focus();
        self.action_buttons.set_selected(0);
    }

    fn focus_delete(&mut self) {
        self.action_buttons.focus();
        self.action_buttons.set_selected(1);
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // If not active, don't handle keys
        if !self.active {
            return false;
        }

        // If currently editing or adding
        if self.action == ListAction::Edit || self.action == ListAction::Add {
            match key.code {
                KeyCode::Enter => {
                    if self.action == ListAction::Add {
                        // Finish adding a new item
                        let new_item = self.input_box.text().to_string();
                        if !new_item.trim().is_empty() {
                            self.items.push(new_item);
                            self.selected = Some(self.items.len() - 1);
                        }
                    } else if let Some(idx) = self.selected {
                        // Finish editing an existing item
                        if idx < self.items.len() {
                            let updated_value = self.input_box.text().to_string();
                            if !updated_value.trim().is_empty() {
                                self.items[idx] = updated_value;
                            }
                        }
                    }
                    self.input_box.unfocus();
                    self.action = ListAction::None;
                    true
                }
                KeyCode::Esc => {
                    // Cancel editing/adding
                    self.input_box.unfocus();
                    self.action = ListAction::None;
                    true
                }
                _ => {
                    // Pass other keys to the input box
                    self.input_box.key_event(key)
                }
            }
        } else {
            // If we're focused on the action buttons
            if self.action_buttons.is_focused() {
                if key.code == KeyCode::Esc {
                    self.action_buttons.unfocus();
                    return true;
                }

                if self.action_buttons.key_event(key) {
                    // If Enter was pressed, execute the selected action
                    if key.code == KeyCode::Enter {
                        let selected_button = self.action_buttons.selected();
                        if selected_button == 0 {
                            // Edit button
                            if let Some(idx) = self.selected {
                                self.action = ListAction::Edit;
                                self.input_box.focus_and_set_text(&self.items[idx]);
                            }
                        } else if selected_button == 1 {
                            // Delete button
                            if let Some(idx) = self.selected {
                                if idx < self.items.len() {
                                    self.items.remove(idx);
                                    if self.items.is_empty() {
                                        self.selected = None;
                                    } else if idx >= self.items.len() {
                                        self.selected = Some(self.items.len() - 1);
                                    }
                                }
                            }
                            self.action_buttons.unfocus();
                        }
                    }
                    return true;
                }
            }

            // Handle main navigation
            match key.code {
                KeyCode::Up => {
                    // Move selection up
                    if let Some(idx) = self.selected {
                        if idx > 0 {
                            self.selected = Some(idx - 1);
                            // Reset button state when changing selection
                            self.focus_edit();
                        } else {
                            return false;
                        }
                    } else if !self.items.is_empty() {
                        // Move from Add button to last item
                        self.selected = Some(self.items.len() - 1);
                        // Reset button state when changing selection
                        self.focus_edit();
                    } else {
                        return false;
                    }
                }
                KeyCode::Down => {
                    // Move selection down
                    if let Some(idx) = self.selected {
                        if idx + 1 < self.items.len() {
                            self.selected = Some(idx + 1);
                            // Reset button state when changing selection
                            self.focus_edit();
                        } else {
                            // Move to Add button
                            self.selected = None;
                        }
                    } else {
                        return false;
                    }
                }
                KeyCode::Left => {
                    if self.selected.is_some() {
                        // Focus the action buttons for an item
                        self.focus_edit();
                        return true;
                    }
                    return false;
                }
                KeyCode::Right => {
                    if self.selected.is_some() {
                        // Focus the action buttons for an item
                        self.focus_delete();
                        return true;
                    }
                    return false;
                }
                KeyCode::Enter => {
                    if self.selected.is_some() {
                        // For item selection, focus the action buttons by default
                        self.focus_edit();
                    } else {
                        // Add button selected - start adding
                        self.action = ListAction::Add;
                        self.input_box.focus_and_clear();
                    }
                }
                KeyCode::Delete => {
                    // Shortcut to delete the selected item
                    if let Some(idx) = self.selected {
                        if idx < self.items.len() {
                            self.items.remove(idx);
                            if self.items.is_empty() {
                                self.selected = None;
                            } else if idx >= self.items.len() {
                                self.selected = Some(self.items.len() - 1);
                            }
                        }
                    }
                }
                _ => return false,
            }
            true
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
            height: area.height.saturating_sub(2),
        };

        // When not focused, just show a summary
        if !self.active {
            // Handle empty list case
            if self.items.is_empty() {
                Paragraph::new("[Empty]")
                    .style(Style::default().fg(Color::White))
                    .render(content_area, buf);
            } else {
                // Block mode - show items on separate lines
                let max_items = self.max_items().min(content_area.height as usize);

                for (i, item) in self.items.iter().take(max_items).enumerate() {
                    let y = content_area.y + i as u16;
                    Paragraph::new(item.as_str())
                        .style(Style::default().fg(Color::White))
                        .render(
                            Rect {
                                x: content_area.x,
                                y,
                                width: content_area.width,
                                height: 1,
                            },
                            buf,
                        );
                }

                // Show "more" indicator if needed
                let hidden_count = self.items.len().saturating_sub(max_items);
                if hidden_count > 0 {
                    let more_text = format!("(+{hidden_count} more...)");
                    let more_y = content_area.y + max_items as u16;

                    Paragraph::new(more_text)
                        .style(Style::default().fg(Color::Gray))
                        .render(
                            Rect {
                                x: content_area.x,
                                y: more_y,
                                width: content_area.width,
                                height: 1,
                            },
                            buf,
                        );
                }
            }

            return;
        }

        // Always render items when focused
        let max_visible_items = content_area
            .height
            .saturating_sub(if self.active { 1 } else { 0 })
            as usize; // Reserve space for Add button
        let items_to_show = self.items.len().min(max_visible_items);

        for i in 0..items_to_show {
            let y = content_area.y + i as u16;
            let is_selected = self.selected == Some(i) && self.active;

            // If selected and editing
            if is_selected && self.action == ListAction::Edit && self.input_box.is_focused() {
                self.input_box.no_border();
                self.input_box.draw(
                    Rect {
                        x: content_area.x,
                        y,
                        width: content_area.width.saturating_sub(20), // Leave space for buttons
                        height: 1,
                    },
                    buf,
                );
            } else {
                // Normal item display
                let style = if is_selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                // Display the item
                Paragraph::new(self.items[i].as_str()).style(style).render(
                    Rect {
                        x: content_area.x,
                        y,
                        width: content_area.width.saturating_sub(20), // Leave space for buttons
                        height: 1,
                    },
                    buf,
                );
            }

            // Render action buttons for selected item when active
            if is_selected && self.action != ListAction::Edit && self.active {
                // Configure buttons if this is the selected row
                let button_area = Rect {
                    x: content_area.x + content_area.width - 19,
                    y,
                    width: 19,
                    height: 1,
                };

                self.action_buttons.draw(button_area, buf);
            }
        }

        // Render Add button as the last item only when active
        if self.active {
            let add_y = content_area.y + items_to_show as u16;

            // Show input box if adding
            if self.selected.is_none()
                && self.action == ListAction::Add
                && self.input_box.is_focused()
            {
                self.input_box.no_border();
                self.input_box.draw(
                    Rect {
                        x: content_area.x,
                        y: add_y,
                        width: content_area.width,
                        height: 1,
                    },
                    buf,
                );
            } else {
                // Show Add button
                let add_style = if self.selected.is_none() {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default().fg(Color::Green)
                };

                Paragraph::new("+ Add").style(add_style).render(
                    Rect {
                        x: content_area.x,
                        y: add_y,
                        width: content_area.width,
                        height: 1,
                    },
                    buf,
                );
            }

            // If there are more items than can be shown, indicate scrolling is possible
            if self.items.len() > max_visible_items {
                let indicator_style = Style::default().fg(Color::DarkGray);
                Paragraph::new("(more...)").style(indicator_style).render(
                    Rect {
                        x: content_area.x + content_area.width - 15,
                        y: add_y,
                        width: 15,
                        height: 1,
                    },
                    buf,
                );
            }
        }
    }

    fn max_items(&self) -> usize {
        self.max_display.unwrap_or(self.items.len())
    }
}
