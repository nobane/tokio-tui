// tokio-tui/src/widgets/status/status_cells/text_status.rs
use std::{
    any::Any,
    time::{Duration, Instant},
};

use ratatui::{buffer::Buffer, layout::Constraint, widgets::Widget as _};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use crate::{CellRef, StatusCell, StatusCellUpdate, ToStatusCell};

pub struct TextStatus {
    pub text: Vec<(String, Style)>,
    pub clip_mode: ClipMode,
    pub alignment: TextAlignment,
    needs_redraw: bool,
    last_rendered_text: String,
    last_update: Instant,
}

const TEXT_UPDATE_INTERVAL: Duration = Duration::from_millis(200); // 5 FPS

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum ClipMode {
    Truncate,
    EllipsisEnd(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum TextAlignment {
    Left,
    Right,
}

impl StatusCell for TextStatus {
    fn new<T: Into<Self>>(args: T) -> Self {
        args.into()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn preprocess(&mut self) {
        if self.last_update.elapsed() < TEXT_UPDATE_INTERVAL {
            return;
        }

        let new_text: String = self.text.iter().map(|(s, _)| s.as_str()).collect();
        if self.last_rendered_text != new_text {
            self.last_rendered_text = new_text;
            self.needs_redraw = true;
        }

        self.last_update = Instant::now();
    }
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer) {
        let available_width = area.width as usize;
        let clipped_message = match self.clip_mode {
            ClipMode::Truncate => self.truncate_message(available_width),
            ClipMode::EllipsisEnd(n) => self.ellipsis_end_message(available_width, n),
        };

        let content_width = clipped_message.width();
        let padding = if content_width < available_width {
            " ".repeat(available_width - content_width)
        } else {
            String::new()
        };

        let final_message = match self.alignment {
            TextAlignment::Left => clipped_message,
            TextAlignment::Right => {
                let mut spans = vec![Span::raw(padding)];
                spans.extend(
                    clipped_message
                        .lines
                        .into_iter()
                        .flat_map(|line| line.spans),
                );
                Text::from(Line::from(spans))
            }
        };

        Paragraph::new(final_message).render(area, buf);
        self.needs_redraw = false;
    }
    fn constraint(&self) -> Constraint {
        Constraint::Fill(1)
    }
    fn needs_draw(&self) -> bool {
        self.needs_redraw
    }
}

impl TextStatus {
    pub fn new<T: Into<Self>>(args: T) -> Self {
        <Self as StatusCell>::new(args)
    }
}

impl CellRef<TextStatus> {
    pub fn set_text(&self, text: impl Into<String>, style: Style) -> StatusCellUpdate {
        let text = text.into();
        self.update_with(move |text_status| {
            let new_message = vec![(text.clone(), style)];
            if text_status.text != new_message {
                text_status.text = new_message;
                text_status.needs_redraw = true;
            }
        })
    }

    pub fn append(&self, text: impl Into<String>, style: Style) -> StatusCellUpdate {
        let text = text.into();
        self.update_with(move |text_status| {
            text_status.text.push((text.clone(), style));
            text_status.needs_redraw = true;
        })
    }

    pub fn align(&self, alignment: TextAlignment) -> StatusCellUpdate {
        self.update_with(move |text_status| {
            if text_status.alignment != alignment {
                text_status.alignment = alignment;
                text_status.needs_redraw = true;
            }
        })
    }
}

impl From<String> for TextStatus {
    fn from(message: String) -> Self {
        TextStatus {
            text: vec![(message.clone(), Style::default())],
            clip_mode: ClipMode::Truncate,
            alignment: TextAlignment::Left,
            needs_redraw: true,
            last_rendered_text: message,
            last_update: Instant::now(),
        }
    }
}

impl Default for TextStatus {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            clip_mode: ClipMode::Truncate,
            alignment: TextAlignment::Left,
            needs_redraw: true,
            last_rendered_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl TextStatus {
    fn truncate_message(&self, available_width: usize) -> Text<'static> {
        let mut current_width = 0;
        let mut clipped = Vec::new();

        let message_iter = self.text.iter();

        for (content, style) in message_iter {
            let content_width = content.len();
            if current_width + content_width <= available_width {
                clipped.push(Span::styled(content.clone(), *style));
                current_width += content_width;
            } else {
                let remaining = available_width - current_width;
                let truncated_content = content.chars().take(remaining).collect::<String>();
                clipped.push(Span::styled(truncated_content, *style));
                break;
            }
        }

        Text::from(Line::from(clipped))
    }

    fn ellipsis_end_message(&self, available_width: usize, n: usize) -> Text<'static> {
        let total_length: usize = self.text.iter().map(|(content, _)| content.len()).sum();

        if total_length <= available_width {
            return Text::from(Line::from(
                self.text
                    .iter()
                    .map(|(content, style)| Span::styled(content.clone(), *style))
                    .collect::<Vec<Span>>(),
            ));
        }

        let ellipsis = "..";
        let effective_width = available_width.saturating_sub(ellipsis.len());

        let mut current_width = 0;
        let mut clipped = Vec::new();
        let mut end_spans = Vec::new();

        // Process end spans first
        for (content, style) in self.text.iter().rev().take(n) {
            let span = Span::styled(content.clone(), *style);
            end_spans.push(span);
            current_width += content.len();
        }

        // Process main content
        for (content, style) in self.text.iter() {
            if current_width >= effective_width {
                break;
            }

            let remaining = effective_width - current_width;
            if content.len() <= remaining {
                clipped.push(Span::styled(content.clone(), *style));
                current_width += content.len();
            } else {
                let truncated_content = content.chars().take(remaining).collect::<String>();
                clipped.push(Span::styled(truncated_content, *style));
                break;
            }
        }

        // Add ellipsis
        clipped.push(Span::raw(ellipsis));

        // Combine clipped content and end spans
        clipped.extend(end_spans.into_iter().rev());

        Text::from(Line::from(clipped))
    }
}

impl From<Vec<(String, Style)>> for TextStatus {
    fn from(val: Vec<(String, Style)>) -> Self {
        let last_rendered_text: String = val.iter().map(|(s, _)| s.as_str()).collect();
        TextStatus {
            text: val,
            clip_mode: ClipMode::Truncate,
            alignment: TextAlignment::Left,
            needs_redraw: true,
            last_rendered_text,
            last_update: Instant::now(),
        }
    }
}

impl From<(Vec<(String, Style)>, ClipMode)> for TextStatus {
    fn from((message, clip_mode): (Vec<(String, Style)>, ClipMode)) -> Self {
        let last_rendered_text: String = message.iter().map(|(s, _)| s.as_str()).collect();
        TextStatus {
            text: message,
            clip_mode,
            alignment: TextAlignment::Left,
            needs_redraw: true,
            last_rendered_text,
            last_update: Instant::now(),
        }
    }
}

impl From<(Vec<(String, Style)>, ClipMode, TextAlignment)> for TextStatus {
    fn from(
        (message, clip_mode, alignment): (Vec<(String, Style)>, ClipMode, TextAlignment),
    ) -> Self {
        let last_rendered_text: String = message.iter().map(|(s, _)| s.as_str()).collect();
        TextStatus {
            text: message,
            clip_mode,
            alignment,
            needs_redraw: true,
            last_rendered_text,
            last_update: Instant::now(),
        }
    }
}

impl From<&str> for TextStatus {
    fn from(message: &str) -> Self {
        TextStatus {
            text: vec![(message.to_string(), Style::default())],
            clip_mode: ClipMode::Truncate,
            alignment: TextAlignment::Left,
            needs_redraw: true,
            last_rendered_text: message.to_string(),
            last_update: Instant::now(),
        }
    }
}

impl From<(&str, TextAlignment)> for TextStatus {
    fn from((message, alignment): (&str, TextAlignment)) -> Self {
        TextStatus {
            text: vec![(message.to_string(), Style::default())],
            clip_mode: ClipMode::Truncate,
            alignment,
            needs_redraw: true,
            last_rendered_text: message.to_string(),
            last_update: Instant::now(),
        }
    }
}

impl From<()> for TextStatus {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl ToStatusCell for TextStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
