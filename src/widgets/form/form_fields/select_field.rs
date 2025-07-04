// tokio-tui/src/widgets/form/form_fields/select_field.rs
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph, Widget},
};

use super::{FormFieldType, FormFieldWidget};

#[derive(Debug)]
pub struct SelectFormField {
    pub options: Vec<String>,
    pub selected: usize,
    pub dropdown_open: bool,
}

impl FormFieldWidget {
    /// Creates a selection field with options
    pub fn select(
        label: impl Into<String>,
        options: Vec<String>,
        selected: usize,
        required: bool,
    ) -> Self {
        Self {
            label: label.into(),
            inner: FormFieldType::Select(SelectFormField {
                options,
                selected,
                dropdown_open: false,
            }),
            required,
            help_text: None,
            is_focused: false,
        }
    }
}

impl SelectFormField {
    pub fn calculate_height(&self) -> u16 {
        if self.dropdown_open {
            // When dropdown is open, show all options + field itself
            3 + self.options.len() as u16
        } else {
            3
        }
    }
    pub fn get_value(&self) -> String {
        if self.selected < self.options.len() {
            self.options[self.selected].clone()
        } else {
            String::new()
        }
    }

    pub fn is_valid(&self) -> bool {
        self.selected < self.options.len()
    }

    pub fn is_active(&self) -> bool {
        self.dropdown_open
    }

    pub fn enter(&mut self) {
        self.dropdown_open = true;
    }

    pub fn leave(&mut self) {
        self.dropdown_open = false;
    }

    pub fn is_open(&self) -> bool {
        self.dropdown_open
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if !self.dropdown_open {
            return false;
        }

        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.options.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Enter => {
                self.dropdown_open = false;
            }
            _ => return false,
        };
        true
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect, block: Block<'_>) {
        // Render the block
        block.render(area, buf);

        // Calculate content area
        let content_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        // When dropdown is closed, just show the selected value
        if !self.dropdown_open {
            let selected_value = if self.selected < self.options.len() {
                &self.options[self.selected]
            } else {
                ""
            };

            let value_style = if self.is_active() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            let value_display = format!("{selected_value} ▼");
            Paragraph::new(value_display)
                .style(value_style)
                .render(content_area, buf);
        } else {
            // When dropdown is open, render options as a list

            // First render the selected value
            let selected_value = if self.selected < self.options.len() {
                &self.options[self.selected]
            } else {
                ""
            };

            let value_style = Style::default().fg(Color::Yellow);
            let value_display = format!("{selected_value} ▲");

            Paragraph::new(value_display)
                .style(value_style)
                .render(content_area, buf);

            // Calculate dropdown list area
            let dropdown_area = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(3), // Leave room for the field itself
            };

            // Determine visible range based on dropdown area height
            let max_visible_options = dropdown_area.height as usize;
            let total_options = self.options.len();

            if max_visible_options == 0 || total_options == 0 {
                return;
            }

            // Calculate visible range with the selected option centered if possible
            let mut start_idx = 0;

            if self.selected >= max_visible_options / 2 && total_options > max_visible_options {
                start_idx = self.selected - max_visible_options / 2;

                // Make sure we don't go past the end
                if start_idx + max_visible_options > total_options {
                    start_idx = total_options - max_visible_options;
                }
            }

            let end_idx = (start_idx + max_visible_options).min(total_options);

            // Render visible options
            for (i, idx) in (start_idx..end_idx).enumerate() {
                let option = &self.options[idx];
                let is_selected = idx == self.selected;

                let option_style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                // Prefix selected option with a marker
                let display_text = if is_selected {
                    format!("▶ {option}")
                } else {
                    format!("  {option}")
                };

                Paragraph::new(display_text).style(option_style).render(
                    Rect {
                        x: dropdown_area.x,
                        y: dropdown_area.y + i as u16,
                        width: dropdown_area.width,
                        height: 1,
                    },
                    buf,
                );
            }

            // If we're showing a subset of options, show scroll indicators
            if start_idx > 0 {
                let indicator_style = Style::default().fg(Color::DarkGray);
                Paragraph::new("▲ more").style(indicator_style).render(
                    Rect {
                        x: dropdown_area.x,
                        y: dropdown_area.y,
                        width: dropdown_area.width,
                        height: 1,
                    },
                    buf,
                );
            }

            if end_idx < total_options {
                let indicator_style = Style::default().fg(Color::DarkGray);
                Paragraph::new("▼ more").style(indicator_style).render(
                    Rect {
                        x: dropdown_area.x,
                        y: dropdown_area.y + (end_idx - start_idx) as u16,
                        width: dropdown_area.width,
                        height: 1,
                    },
                    buf,
                );
            }
        }
    }
}
