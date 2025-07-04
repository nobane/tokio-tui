// tokio-tui/src/widgets/scrollbox/scrollbox_widget.rs
//! src/widgets/scrollbox/scrollbox_widget.rs
//!
//! Refactored ScrollbackWidget with granular invalidation flags
//! to avoid unnecessary full–re‑renders.
//! Public API is **unchanged**, so existing call‑sites continue to
//! compile, but internally we keep track of which portions of the
//! widget need to be redrawn:
//!   • Frame & border
//!   • Text lines
//!   • Scrollbars
//!   • Search box
//!
//! Any helper that mutates state now marks one (or several) of those
//! parts dirty; `needs_draw()` checks if _any_ flag is dirty and the
//! `render()` implementation selectively re‑renders only the flagged
//! regions.
//!
//! This keeps performance predictable even with very large scrollback
//! buffers.
//!
//! -------------------------------------------------------------------

use std::time::Instant;
use std::{collections::VecDeque, time::Duration};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::{Margin, Position, Rect},
    style::{Color, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget as _, Widget,
    },
};

use crate::{InputWidget, IntoEitherIter, TuiWidget, tui_theme};

use super::{StyledChar, StyledText, parse_ansi_string};

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragDirection {
    None,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl DragDirection {
    fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ScrollbarDrag {
    None,
    Vertical(u16),   // stores the initial mouse y position relative to thumb
    Horizontal(u16), // stores the initial mouse x position relative to thumb
}

#[derive(Clone, Copy, PartialEq)]
enum SearchMode {
    Closed,
    Input,
    Open,
}

impl SearchMode {
    pub fn is_active(self) -> bool {
        !matches!(self, SearchMode::Closed)
    }
    pub fn is_closed(self) -> bool {
        matches!(self, SearchMode::Closed)
    }
    pub fn has_focus(self) -> bool {
        matches!(self, SearchMode::Input)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SelectionStart {
    line: usize,     // Original line index
    char_idx: usize, // Character index within that line
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SelectionEnd {
    line: usize,     // Original line index
    char_idx: usize, // Character index within that line
}

#[derive(Debug, Clone, PartialEq)]
struct Selection {
    start: SelectionStart,
    end: SelectionEnd,
    active: bool,
}

impl Selection {
    fn new() -> Self {
        Self {
            start: SelectionStart {
                line: 0,
                char_idx: 0,
            },
            end: SelectionEnd {
                line: 0,
                char_idx: 0,
            },
            active: false,
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn clear(&mut self) {
        self.active = false;
    }

    fn start_selection(&mut self, line: usize, char_idx: usize) {
        self.start = SelectionStart { line, char_idx };
        self.end = SelectionEnd { line, char_idx };
        self.active = true;
    }

    fn update_end(&mut self, line: usize, char_idx: usize) {
        if self.active {
            self.end = SelectionEnd { line, char_idx };
        }
    }

    fn normalize(&self) -> (SelectionStart, SelectionEnd) {
        if self.start.line < self.end.line
            || (self.start.line == self.end.line && self.start.char_idx <= self.end.char_idx)
        {
            (self.start, self.end)
        } else {
            (
                SelectionStart {
                    line: self.end.line,
                    char_idx: self.end.char_idx,
                },
                SelectionEnd {
                    line: self.start.line,
                    char_idx: self.start.char_idx,
                },
            )
        }
    }

    fn contains_position(&self, line: usize, char_idx: usize) -> bool {
        if !self.active {
            return false;
        }

        let (start, end) = self.normalize();

        if line < start.line || line > end.line {
            return false;
        }

        if line == start.line && line == end.line {
            char_idx >= start.char_idx && char_idx < end.char_idx
        } else if line == start.line {
            char_idx >= start.char_idx
        } else if line == end.line {
            char_idx < end.char_idx
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CursorState {
    Default,
    Text,       // Over selectable text
    Selecting,  // During active selection
    LineNumber, // Over line numbers (not selectable)
}

const INITIAL_WIDTH: usize = 80;

/// A multi‑purpose scrollback widget with optional line‑wrapping,
/// search, dev‑mode overlay and both vertical & horizontal scrolling.
pub struct ScrollbackWidget {
    scrollbar_drag: ScrollbarDrag,

    /* ---------- rendering & style ----------- */
    style: Style,
    line_number_style: Style,
    borders: Borders,
    border_style: Style,
    border_color: Color,
    scrollbar_style: Style,

    /* ---------- data  ----------- */
    buffer: VecDeque<Vec<StyledChar>>,
    line_capacity: usize,
    lengths: VecDeque<usize>,
    max_line_width: usize,

    /* ---------- wrapping state ----------- */
    wrap_lines: bool,
    wrap_indent: usize,
    wrapped_lines: Vec<(usize, usize, usize)>, // (orig_idx, start, end)
    wrapped_lines_width: usize,

    /* ---------- scrolling state ----------- */
    v_scrollbar: ScrollbarState,
    h_scrollbar: ScrollbarState,
    vertical_offset: usize,
    horizontal_offset: usize,
    auto_scroll: bool,

    /* ---------- selection state ----------- */
    selection: Selection,
    mouse_is_down: bool,

    /* ---------- cursor state ----------- */
    cursor_state: CursorState,
    last_mouse_pos: Option<(u16, u16)>,

    /* ---------- misc flags ----------- */
    redraw_requested: bool,
    is_focused: bool,
    show_line_numbers: bool,
    dev_mode: bool,

    last_area: Rect,
    inner_width: usize,
    inner_height: usize,

    /* ---------- UI strings ----------- */
    title: String,
    info_text: String,

    /* ---------- key handling helpers ----------- */
    waiting_for_g: bool,
    last_g_press: Instant,

    /* ---------- search ----------- */
    search_mode: SearchMode,
    search_input: InputWidget,
    search_term: String,
    search_matches: Vec<(usize, usize)>, // (line_idx, match_start)
    current_match: usize,

    /* ---------- drag-scroll state ----------- */
    drag_scroll_timer: Option<Instant>,
    drag_direction: DragDirection,
    last_mouse_in_bounds: bool,
}

impl TuiWidget for ScrollbackWidget {
    fn need_draw(&self) -> bool {
        self.redraw_requested || self.is_drag_scrolling()
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        // Handle drag-scroll during selection
        if self.is_drag_scrolling() {
            self.perform_drag_scroll();

            // Try to update selection after drag-scroll
            if let Some((x, y)) = self.last_mouse_pos {
                if let Some((line_idx, char_idx)) = self.screen_to_buffer_position(x, y) {
                    self.selection.update_end(line_idx, char_idx);
                    self.last_mouse_in_bounds = true;
                }
            }
        }

        // If the widget got resized – redraw everything.
        if area != self.last_area {
            Self::clear_buffer(area, buf);
            self.last_area = area;
        }

        // Calculate inner area ( minus border – and search box space )
        let mut inner = area.inner(Margin::new(1, 1));
        if self.search_mode.is_active() && inner.height > 1 {
            inner.height -= 2;
        }
        self.inner_width = inner.width as usize;

        if self.inner_height != inner.height as usize {
            self.inner_height = inner.height as usize;
            self.check_and_auto_scroll();
        }

        /* ---------------- frame ---------------- */
        self.recalculate_scrollbars();

        /* ---------------- lines ---------------- */
        if self.wrap_lines {
            self.render_lines_wrapped(inner, buf);
        } else {
            self.render_lines_clipped(inner, buf);
        }

        /* ---------------- search box ----------- */
        self.render_search_input(area, buf);

        self.render_outer_frame(inner, area, buf);

        self.redraw_requested = false;
    }

    fn mouse_event(&mut self, mouse: MouseEvent) -> bool {
        // Store the mouse position for cursor management
        self.last_mouse_pos = Some((mouse.column, mouse.row));

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is on vertical scrollbar
                if self.is_point_in_vertical_scrollbar(mouse.column, mouse.row) {
                    if self.is_point_in_vertical_thumb(mouse.column, mouse.row) {
                        // Start dragging vertical thumb
                        let (thumb_start, _) = self.get_vertical_thumb_position();
                        let drag_offset = mouse.row.saturating_sub(thumb_start);
                        self.scrollbar_drag = ScrollbarDrag::Vertical(drag_offset);
                    } else {
                        // Click on scrollbar track
                        self.handle_vertical_scrollbar_click(mouse.row);
                    }
                    return true;
                }

                // Check if click is on horizontal scrollbar
                if self.is_point_in_horizontal_scrollbar(mouse.column, mouse.row) {
                    if self.is_point_in_horizontal_thumb(mouse.column, mouse.row) {
                        // Start dragging horizontal thumb
                        let (thumb_start, _) = self.get_horizontal_thumb_position();
                        let drag_offset = mouse.column.saturating_sub(thumb_start);
                        self.scrollbar_drag = ScrollbarDrag::Horizontal(drag_offset);
                    } else {
                        // Click on scrollbar track
                        self.handle_horizontal_scrollbar_click(mouse.column);
                    }
                    return true;
                }

                // Regular content selection logic
                if !mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    self.clear_selection();
                }
                self.handle_mouse_press(mouse.column, mouse.row);
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                match self.scrollbar_drag {
                    ScrollbarDrag::Vertical(drag_offset) => {
                        self.handle_vertical_scrollbar_drag(mouse.row, drag_offset);
                        true
                    }
                    ScrollbarDrag::Horizontal(drag_offset) => {
                        self.handle_horizontal_scrollbar_drag(mouse.column, drag_offset);
                        true
                    }
                    ScrollbarDrag::None => {
                        // Regular content selection drag
                        self.handle_mouse_drag(mouse.column, mouse.row);
                        true
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Stop any scrollbar dragging
                self.scrollbar_drag = ScrollbarDrag::None;

                // Handle regular mouse release
                self.handle_mouse_release();
                true
            }
            MouseEventKind::Moved => {
                // Update cursor style based on position
                let cursor_changed = self.update_cursor_state(mouse.column, mouse.row);

                let point_in_thumb = self.is_point_in_vertical_thumb(mouse.column, mouse.row)
                    || self.is_point_in_horizontal_thumb(mouse.column, mouse.row);
                // Change cursor style for scrollbars
                if point_in_thumb && self.cursor_state != CursorState::Selecting {
                    self.cursor_state = CursorState::Text; // or create a new ScrollbarDrag state
                    self.apply_cursor_style(CursorState::Text);
                }

                cursor_changed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(1);
                true
            }
            MouseEventKind::ScrollUp => {
                self.scroll_up(1);
                true
            }
            MouseEventKind::ScrollLeft => {
                self.scroll_left(1);
                true
            }
            MouseEventKind::ScrollRight => {
                self.scroll_right(1);
                true
            }
            _ => false,
        }
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        // Route keys to search input if needed
        if self.search_mode == SearchMode::Input {
            match key.code {
                KeyCode::Esc => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        self.close_search();
                    } else {
                        self.unfocus_search();
                    }
                    return true;
                }
                KeyCode::Enter => {
                    if self.search_term.is_empty() {
                        self.close_search();
                    } else {
                        self.unfocus_search();
                    }
                    return true;
                }
                _ => {
                    let handled = self.search_input.key_event(key);
                    if handled {
                        self.update_search_term();
                    }
                    return handled;
                }
            }
        }

        /* ---------- normal scrollback keys ---------- */
        match key.code {
            /* -------- selection ---------- */
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.copy_selection() {
                    // Auto-scroll to show the copied selection
                    self.drag_scroll_to_selection_bounds();
                    // Clear the selection to indicate action completed
                    self.clear_selection();
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Select all text
                if !self.buffer.is_empty() {
                    let last_line = self.buffer.len() - 1;
                    let last_char = self.buffer[last_line].len();
                    self.selection.start_selection(0, 0);
                    self.selection.update_end(last_line, last_char);
                    self.recalculate_status();

                    // Auto-scroll to show the selection
                    self.drag_scroll_to_selection_bounds();
                    self.request_redraw();
                }
            }
            KeyCode::Esc => {
                if self.search_mode == SearchMode::Open {
                    self.clear_search()
                } else if self.selection.is_active() {
                    self.clear_selection();
                    self.recalculate_status();
                    return true;
                }
            }

            /* -------- search ------------- */
            KeyCode::Char('/') if self.search_mode.is_closed() => self.open_search(),
            KeyCode::Char('/') if self.search_mode == SearchMode::Open => self.focus_search(),
            KeyCode::Char('n') if self.search_mode == SearchMode::Open => self.jump_to_next_match(),
            KeyCode::Char('N') if self.search_mode == SearchMode::Open => self.jump_to_prev_match(),

            /* -------- scrolling ---------- */
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(self.inner_height),
            KeyCode::PageDown => self.scroll_down(self.inner_height),
            KeyCode::Home => self.scroll_to_top(),
            KeyCode::End => self.scroll_to_bottom(),
            KeyCode::Left => {
                let off = if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.inner_width
                } else {
                    1
                };
                self.scroll_left(off);
            }
            KeyCode::Right => {
                let off = if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.inner_width
                } else {
                    1
                };
                self.scroll_right(off);
            }

            /* -------- dev / wrap / misc -- */
            KeyCode::F(12) => {
                self.dev_mode = !self.dev_mode;
                self.request_redraw();
            }
            KeyCode::F(11) => {
                self.set_wrap_lines(!self.wrap_lines);
            }
            KeyCode::F(10) => {
                self.show_line_numbers = !self.show_line_numbers;
                self.request_redraw();
            }
            KeyCode::F(9) => self.request_redraw(),

            /* -------- vim‑style nav ----- */
            KeyCode::Char('g') => {
                let now = Instant::now();
                if self.waiting_for_g && now.duration_since(self.last_g_press).as_secs_f32() < 1.0 {
                    self.scroll_to_top();
                    self.waiting_for_g = false;
                } else {
                    self.waiting_for_g = true;
                    self.last_g_press = now;
                }
            }
            KeyCode::Char('G') => self.scroll_to_bottom(),

            _ => return false,
        }
        true
    }

    fn focus(&mut self) {
        self.apply_focus(true);
    }

    fn unfocus(&mut self) {
        self.apply_focus(false);
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
/* ******************************************************************
 * Cursor and selection management methods
 * *****************************************************************/

impl ScrollbackWidget {
    const DRAG_EDGE_MARGIN: usize = 4; // Start scrolling when within 2 chars of edge
    const DRAG_SPEED_SLOW: Duration = Duration::from_millis(150);
    const DRAG_SPEED_FAST: Duration = Duration::from_millis(50);

    const DRAG_VERTICAL_REPEAT: usize = 2;
    const DRAG_VERTICAL_RESUME: usize = 3;
    const DRAG_HORIZONTAL_REPEAT: usize = 8;
    const DRAG_HORIZONTAL_RESUME: usize = 5;

    fn handle_mouse_drag(&mut self, x: u16, y: u16) {
        if !self.mouse_is_down {
            return;
        }

        // Always update drag-scroll state based on mouse position
        self.update_drag_scroll_state(x, y);

        // Try to convert position to buffer coordinates
        if let Some((line_idx, char_idx)) = self.screen_to_buffer_position(x, y) {
            self.selection.update_end(line_idx, char_idx);
            self.last_mouse_in_bounds = true;
            self.request_redraw();
        } else {
            // If we can't convert position, try to handle edge cases
            self.handle_edge_selection(x, y);
        }
    }

    fn update_drag_scroll_state(&mut self, x: u16, y: u16) {
        let inner = self.last_area.inner(Margin::new(1, 1));
        let mut content_height = inner.height;
        if self.search_mode.is_active() && content_height > 1 {
            content_height -= 2;
        }

        let ln_width = if self.show_line_numbers {
            self.calculate_line_num_width(self.buffer.len() + 1)
        } else {
            0
        };

        let content_start_x = inner.x + if ln_width > 0 { ln_width as u16 + 1 } else { 0 };
        let content_end_x = inner.x + inner.width;
        let content_start_y = inner.y;
        let content_end_y = inner.y + content_height;

        let margin = Self::DRAG_EDGE_MARGIN as u16;

        // Determine scroll direction based on mouse position relative to content area
        let new_direction = if y < content_start_y {
            // Above content area - scroll up
            if x < content_start_x {
                DragDirection::UpLeft
            } else if x >= content_end_x {
                DragDirection::UpRight
            } else {
                DragDirection::Up
            }
        } else if y >= content_end_y {
            // Below content area - scroll down
            if x < content_start_x {
                DragDirection::DownLeft
            } else if x >= content_end_x {
                DragDirection::DownRight
            } else {
                DragDirection::Down
            }
        } else {
            // Within vertical bounds of content area
            if x < content_start_x {
                // To the left of content area
                DragDirection::Left
            } else if x >= content_end_x {
                // To the right of content area
                DragDirection::Right
            } else if x >= content_start_x && x < content_start_x + margin {
                // Near left edge of content (within margin) - this will trigger continuous scrolling
                DragDirection::Left
            } else if x >= content_end_x - margin && x < content_end_x {
                // Near right edge of content (within margin) - this will trigger continuous scrolling
                DragDirection::Right
            } else if y >= content_start_y && y < content_start_y + margin {
                // Near top edge of content (within margin)
                DragDirection::Up
            } else if y >= content_end_y - margin && y < content_end_y {
                // Near bottom edge of content (within margin)
                DragDirection::Down
            } else {
                // Within content area, not near edges
                DragDirection::None
            }
        };

        // Update drag-scroll state - always restart timer when direction changes
        if new_direction != self.drag_direction {
            self.drag_direction = new_direction;
            self.drag_scroll_timer = if new_direction != DragDirection::None {
                Some(Instant::now())
            } else {
                None
            };
        }
    }

    fn perform_drag_scroll(&mut self) {
        if self.drag_direction == DragDirection::None {
            return;
        }

        let Some(timer) = self.drag_scroll_timer else {
            return;
        };

        // Determine scroll speed based on how long we've been scrolling
        let elapsed = timer.elapsed();
        let scroll_interval = if elapsed > Duration::from_millis(500) {
            Self::DRAG_SPEED_FAST
        } else {
            Self::DRAG_SPEED_SLOW
        };

        // Only scroll if enough time has passed
        if elapsed < scroll_interval {
            return;
        }

        // Reset timer for next scroll
        self.drag_scroll_timer = Some(Instant::now());

        // Determine scroll amount - smaller amounts for smoother character-by-character selection
        let vertical_amount = if elapsed > Duration::from_millis(500) {
            Self::DRAG_VERTICAL_RESUME
        } else {
            Self::DRAG_VERTICAL_REPEAT
        };
        let horizontal_amount = if elapsed > Duration::from_millis(500) {
            Self::DRAG_HORIZONTAL_RESUME
        } else {
            Self::DRAG_HORIZONTAL_REPEAT
        };

        // Perform scroll based on direction and check if we actually scrolled
        let mut did_scroll = false;

        match self.drag_direction {
            DragDirection::None => {}
            DragDirection::Up => {
                let old_offset = self.vertical_offset;
                self.scroll_up(vertical_amount);
                did_scroll = old_offset != self.vertical_offset;
            }
            DragDirection::Down => {
                let old_offset = self.vertical_offset;
                self.scroll_down(vertical_amount);
                did_scroll = old_offset != self.vertical_offset;
            }
            DragDirection::Left => {
                if !self.wrap_lines {
                    let old_offset = self.horizontal_offset;
                    self.scroll_left(horizontal_amount);
                    did_scroll = old_offset != self.horizontal_offset;
                }
            }
            DragDirection::Right => {
                if !self.wrap_lines {
                    let old_offset = self.horizontal_offset;
                    self.scroll_right(horizontal_amount);
                    did_scroll = old_offset != self.horizontal_offset;
                }
            }
            DragDirection::UpLeft => {
                let old_v = self.vertical_offset;
                let old_h = self.horizontal_offset;
                self.scroll_up(vertical_amount);
                if !self.wrap_lines {
                    self.scroll_left(horizontal_amount);
                }
                did_scroll = old_v != self.vertical_offset || old_h != self.horizontal_offset;
            }
            DragDirection::UpRight => {
                let old_v = self.vertical_offset;
                let old_h = self.horizontal_offset;
                self.scroll_up(vertical_amount);
                if !self.wrap_lines {
                    self.scroll_right(horizontal_amount);
                }
                did_scroll = old_v != self.vertical_offset || old_h != self.horizontal_offset;
            }
            DragDirection::DownLeft => {
                let old_v = self.vertical_offset;
                let old_h = self.horizontal_offset;
                self.scroll_down(vertical_amount);
                if !self.wrap_lines {
                    self.scroll_left(horizontal_amount);
                }
                did_scroll = old_v != self.vertical_offset || old_h != self.horizontal_offset;
            }
            DragDirection::DownRight => {
                let old_v = self.vertical_offset;
                let old_h = self.horizontal_offset;
                self.scroll_down(vertical_amount);
                if !self.wrap_lines {
                    self.scroll_right(horizontal_amount);
                }
                did_scroll = old_v != self.vertical_offset || old_h != self.horizontal_offset;
            }
        }

        // If we couldn't scroll further, stop the drag scroll
        if !did_scroll {
            self.drag_direction = DragDirection::None;
            self.drag_scroll_timer = None;
        }
    }

    fn screen_to_buffer_position(&self, x: u16, y: u16) -> Option<(usize, usize)> {
        let inner = self.last_area.inner(Margin::new(1, 1));
        let mut content_height = inner.height;
        if self.search_mode.is_active() && content_height > 1 {
            content_height -= 2;
        }

        if y < inner.y || y >= inner.y + content_height {
            return None;
        }

        let ln_width = if self.show_line_numbers {
            self.calculate_line_num_width(self.buffer.len() + 1)
        } else {
            0
        };

        let content_start_x = inner.x + if ln_width > 0 { ln_width as u16 + 1 } else { 0 };

        // Allow selection beyond the visible area for drag-scrolling
        if x < content_start_x {
            return None;
        }

        let content_x = (x - content_start_x) as usize;
        let content_y = (y - inner.y) as usize;

        if self.wrap_lines {
            self.screen_to_buffer_position_wrapped(content_x, content_y)
        } else {
            self.screen_to_buffer_position_clipped_progressive(content_x, content_y)
        }
    }

    fn screen_to_buffer_position_clipped_progressive(
        &self,
        content_x: usize,
        content_y: usize,
    ) -> Option<(usize, usize)> {
        let line_idx = self.vertical_offset + content_y;

        if line_idx >= self.buffer.len() {
            return None;
        }

        let line = &self.buffer[line_idx];

        // Calculate the absolute character position including horizontal scroll
        let char_idx = self.horizontal_offset + content_x;

        // Don't clamp to line length - allow selection beyond visible line end
        // This enables continuous scrolling selection
        let final_char_idx = char_idx.min(line.len());

        Some((line_idx, final_char_idx))
    }

    fn handle_edge_selection(&mut self, x: u16, y: u16) {
        let inner = self.last_area.inner(Margin::new(1, 1));
        let mut content_height = inner.height;
        if self.search_mode.is_active() && content_height > 1 {
            content_height -= 2;
        }

        // Check if we're vertically within bounds
        if y < inner.y || y >= inner.y + content_height {
            self.last_mouse_in_bounds = false;
            return;
        }

        let ln_width = if self.show_line_numbers {
            self.calculate_line_num_width(self.buffer.len() + 1)
        } else {
            0
        };

        let content_start_x = inner.x + if ln_width > 0 { ln_width as u16 + 1 } else { 0 };
        let content_end_x = inner.x + inner.width;

        // Handle selection beyond right edge
        if x >= content_end_x {
            let content_y = (y - inner.y) as usize;
            let line_idx = self.vertical_offset + content_y;

            if line_idx < self.buffer.len() {
                let line = &self.buffer[line_idx];
                // Calculate how far beyond the edge we are
                let pixels_beyond = (x - content_end_x) as usize;
                let chars_beyond = pixels_beyond + 1; // Each character is roughly 1 pixel wide

                // Calculate the target character position
                let visible_width = (content_end_x - content_start_x) as usize;
                let target_char_idx = self.horizontal_offset + visible_width + chars_beyond;

                // Clamp to line length
                let final_char_idx = target_char_idx.min(line.len());

                self.selection.update_end(line_idx, final_char_idx);
                self.last_mouse_in_bounds = false; // Mark as out of bounds to trigger more scrolling
                self.request_redraw();
            }
        }
    }

    fn handle_mouse_release(&mut self) {
        self.mouse_is_down = false;
        self.drag_direction = DragDirection::None;
        self.drag_scroll_timer = None;

        // Update cursor state since we're no longer selecting
        if let Some((x, y)) = self.last_mouse_pos {
            self.update_cursor_state(x, y);
        }
    }

    fn handle_mouse_press(&mut self, x: u16, y: u16) {
        // Convert screen coordinates to line and character position
        if let Some((line_idx, char_idx)) = self.screen_to_buffer_position(x, y) {
            self.selection.start_selection(line_idx, char_idx);
            self.recalculate_status();
            self.mouse_is_down = true;
            self.request_redraw();

            // Auto-scroll to ensure the selection start is visible
            if !self.wrap_lines {
                self.drag_scroll_to_char(line_idx, char_idx);
            }
        }
    }

    fn drag_scroll_to_char(&mut self, line_idx: usize, char_idx: usize) {
        if self.wrap_lines || line_idx >= self.buffer.len() {
            return;
        }

        let line_len = self.buffer[line_idx].len();
        if char_idx >= line_len {
            return;
        }

        let visible_start = self.horizontal_offset;
        let visible_end = visible_start + self.inner_width;

        // Check if character is outside visible area
        if char_idx < visible_start {
            // Character is to the left of visible area - scroll left
            let new_offset = char_idx.saturating_sub(self.inner_width / 4); // Leave some margin
            self.horizontal_offset = new_offset;
            self.request_redraw();
        } else if char_idx >= visible_end {
            // Character is to the right of visible area - scroll right
            let new_offset = char_idx + self.inner_width / 4; // Leave some margin
            self.horizontal_offset = new_offset.min(self.max_line_width);
            self.request_redraw();
        }
    }

    fn drag_scroll_to_selection_bounds(&mut self) {
        if !self.selection.is_active() || self.wrap_lines {
            return;
        }

        let (start, end) = self.selection.normalize();

        // Find the bounds of the selection
        let mut min_char = usize::MAX;
        let mut max_char = 0;

        for line_idx in start.line..=end.line {
            if line_idx >= self.buffer.len() {
                break;
            }

            let (start_char, end_char) = if line_idx == start.line && line_idx == end.line {
                (start.char_idx, end.char_idx)
            } else if line_idx == start.line {
                (start.char_idx, self.buffer[line_idx].len())
            } else if line_idx == end.line {
                (0, end.char_idx)
            } else {
                (0, self.buffer[line_idx].len())
            };

            min_char = min_char.min(start_char);
            max_char = max_char.max(end_char);
        }

        if min_char == usize::MAX {
            return;
        }

        let visible_start = self.horizontal_offset;
        let visible_end = visible_start + self.inner_width;

        // Check if selection bounds are outside visible area
        if min_char < visible_start || max_char > visible_end {
            // Try to center the selection in the view
            let selection_width = max_char - min_char;
            let margin = (self.inner_width.saturating_sub(selection_width)) / 2;
            let new_offset = min_char.saturating_sub(margin);

            self.horizontal_offset = new_offset.min(self.max_line_width);
            self.request_redraw();
        }
    }

    fn apply_cursor_style(&self, state: CursorState) {
        use crossterm::ExecutableCommand;
        use crossterm::cursor::SetCursorStyle;

        let style = match state {
            CursorState::Default => SetCursorStyle::DefaultUserShape,
            CursorState::Text => SetCursorStyle::BlinkingBar,
            CursorState::Selecting => SetCursorStyle::SteadyBlock,
            CursorState::LineNumber => SetCursorStyle::DefaultUserShape,
        };

        let _ = std::io::stdout().execute(style);
    }

    fn update_cursor_state(&mut self, x: u16, y: u16) -> bool {
        let new_state = if self.mouse_is_down {
            CursorState::Selecting
        } else if self.is_position_in_line_numbers(x, y) {
            CursorState::LineNumber
        } else if self.is_position_in_content_area(x, y) {
            // Check if we're in a wrap indent area for a continuation line
            if self.wrap_lines && self.is_in_wrap_indent_area(x, y) {
                CursorState::Default // Indent areas are not selectable
            } else {
                CursorState::Text
            }
        } else {
            CursorState::Default
        };

        if self.cursor_state != new_state {
            self.cursor_state = new_state;
            self.apply_cursor_style(new_state);
            return true;
        }
        false
    }

    fn is_in_wrap_indent_area(&self, x: u16, y: u16) -> bool {
        if !self.wrap_lines || self.wrap_indent == 0 {
            return false;
        }

        let inner = self.last_area.inner(Margin::new(1, 1));
        let ln_width = if self.show_line_numbers {
            self.calculate_line_num_width(self.buffer.len() + 1)
        } else {
            0
        };

        let content_start_x = inner.x + if ln_width > 0 { ln_width as u16 + 1 } else { 0 };
        let content_x = (x - content_start_x) as usize;
        let content_y = (y - inner.y) as usize;

        let wrapped_line_idx = self.vertical_offset + content_y;

        if wrapped_line_idx >= self.wrapped_lines.len() {
            return false;
        }

        let (_, start_char, _) = self.wrapped_lines[wrapped_line_idx];

        // If this is a continuation line (start_char > 0) and we're in the indent area
        start_char > 0 && content_x < self.wrap_indent
    }

    fn reset_cursor(&mut self) {
        if self.cursor_state != CursorState::Default {
            self.cursor_state = CursorState::Default;
            self.apply_cursor_style(CursorState::Default);
        }
    }

    /// Get the currently selected text as a string
    pub fn get_selected_text(&self) -> Option<String> {
        if !self.selection.is_active() {
            return None;
        }

        let (start, end) = self.selection.normalize();
        let mut result = String::new();

        for line_idx in start.line..=end.line {
            if line_idx >= self.buffer.len() {
                break;
            }

            let line = &self.buffer[line_idx];

            // Determine the character range for this line
            let (start_char, end_char) = if line_idx == start.line && line_idx == end.line {
                // Selection is entirely within one line
                (start.char_idx, end.char_idx.min(line.len()))
            } else if line_idx == start.line {
                // First line of selection
                if self.wrap_lines {
                    // In wrap mode, always select to end of line for clipped content
                    (start.char_idx, line.len())
                } else {
                    // In clip mode, select entire line if it extends beyond visible area
                    let visible_start = self.horizontal_offset;
                    let visible_end = visible_start + self.inner_width;

                    if line.len() > visible_end || self.horizontal_offset > 0 {
                        // Line is clipped, select entire line
                        (start.char_idx, line.len())
                    } else {
                        // Line fits entirely, select to visible end
                        (start.char_idx, line.len().min(visible_end))
                    }
                }
            } else if line_idx == end.line {
                // Last line of selection
                (0, end.char_idx.min(line.len()))
            } else {
                // Middle line - select entire line
                (0, line.len())
            };

            // Extract the text
            if !result.is_empty() {
                result.push('\n');
            }

            for i in start_char..end_char {
                if i < line.len() {
                    result.push(line[i].ch);
                }
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Copy selected text to clipboard (if available)
    pub fn copy_selection(&self) -> bool {
        let Some(text) = self.get_selected_text() else {
            return false;
        };
        use clipboard::{ClipboardContext, ClipboardProvider};
        if let Ok(mut ctx) = ClipboardContext::new() {
            let _ = ctx.set_contents(text.clone());
        }
        true
    }

    /// Clear current selection
    pub fn clear_selection(&mut self) {
        if self.selection.is_active() {
            self.selection.clear();
            self.mouse_is_down = false;
            self.recalculate_status();
            self.request_redraw();
        }
    }

    fn screen_to_buffer_position_wrapped(
        &self,
        content_x: usize,
        content_y: usize,
    ) -> Option<(usize, usize)> {
        // For wrapped mode, we need to map back from wrapped lines to original lines
        let wrapped_line_idx = self.vertical_offset + content_y;

        if wrapped_line_idx >= self.wrapped_lines.len() {
            return None;
        }

        let (orig_line_idx, start_char, end_char) = self.wrapped_lines[wrapped_line_idx];

        // Adjust for wrap indent - continuation lines are indented
        let char_idx_in_segment = if start_char > 0 {
            // This is a continuation line, account for wrap indent
            content_x.saturating_sub(self.wrap_indent)
        } else {
            // First line of wrapped content, no indent
            content_x
        };

        let absolute_char_idx = start_char + char_idx_in_segment;

        // Clamp to the segment bounds
        let final_char_idx = absolute_char_idx.min(end_char);

        Some((orig_line_idx, final_char_idx))
    }

    fn is_position_in_content_area(&self, x: u16, y: u16) -> bool {
        // Check if we're within the widget bounds first
        let inner = self.last_area.inner(Margin::new(1, 1));
        if self.search_mode.is_active() && inner.height > 1 {
            // Account for search box space
            let content_height = inner.height - 2;
            if y >= inner.y + content_height {
                return false;
            }
        }

        if x < inner.x || x >= inner.x + inner.width || y < inner.y || y >= inner.y + inner.height {
            return false;
        }

        // Check if we're in the line numbers area
        let ln_width = if self.show_line_numbers {
            self.calculate_line_num_width(self.buffer.len() + 1)
        } else {
            0
        };

        let content_start_x = inner.x + if ln_width > 0 { ln_width as u16 + 1 } else { 0 };

        // We're in content area if x is at or after content start
        x >= content_start_x
    }

    fn is_position_in_line_numbers(&self, x: u16, y: u16) -> bool {
        if !self.show_line_numbers {
            return false;
        }

        let inner = self.last_area.inner(Margin::new(1, 1));
        if self.search_mode.is_active() && inner.height > 1 {
            // Account for search box space
            let content_height = inner.height - 2;
            if y >= inner.y + content_height {
                return false;
            }
        }

        let ln_width = self.calculate_line_num_width(self.buffer.len() + 1);
        let line_num_end = inner.x + ln_width as u16;

        x >= inner.x && x < line_num_end && y >= inner.y && y < inner.y + inner.height
    }

    fn is_drag_scrolling(&self) -> bool {
        self.mouse_is_down && self.drag_direction.is_some()
    }
}

impl ScrollbackWidget {
    /* ******************************************************************
     * Constructors
     * *****************************************************************/
    pub fn untitled(capacity: usize) -> Self {
        Self::new("", capacity)
    }

    pub fn new(title: impl AsRef<str>, capacity: usize) -> Self {
        let mut widget = ScrollbackWidget {
            scrollbar_drag: ScrollbarDrag::None,

            /* style */
            style: Style::default(),
            line_number_style: Style::default().fg(tui_theme::GRAY1_FG),
            borders: Borders::all(),
            border_style: Style::default().fg(tui_theme::BORDER_DEFAULT),
            border_color: tui_theme::BORDER_DEFAULT,
            scrollbar_style: Style::default().fg(tui_theme::SCROLLBAR_DEFAULT),

            /* data */
            buffer: VecDeque::with_capacity(capacity),
            line_capacity: capacity,
            lengths: VecDeque::with_capacity(capacity),
            max_line_width: 0,

            /* wrapping */
            wrap_lines: true,
            wrap_indent: 0,
            wrapped_lines: Vec::new(),
            wrapped_lines_width: 0,

            /* scrolling */
            v_scrollbar: ScrollbarState::default(),
            h_scrollbar: ScrollbarState::default(),
            vertical_offset: 0,
            horizontal_offset: 0,
            auto_scroll: true,

            /* selection */
            selection: Selection::new(),
            mouse_is_down: false,

            /* cursor */
            cursor_state: CursorState::Default,
            last_mouse_pos: None,

            /* misc flags */
            redraw_requested: true,
            is_focused: false,
            show_line_numbers: true,
            dev_mode: false,

            last_area: Rect::new(0, 0, 1, 1),
            inner_width: INITIAL_WIDTH,
            inner_height: 1,

            /* UI strings */
            title: title.as_ref().to_string(),
            info_text: String::new(),

            /* key helpers */
            waiting_for_g: false,
            last_g_press: Instant::now(),

            /* search */
            search_mode: SearchMode::Closed,
            search_input: InputWidget::new().with_border(Borders::TOP),
            search_term: String::new(),
            search_matches: Vec::new(),
            current_match: 0,

            /* drag-scroll */
            drag_scroll_timer: None,
            drag_direction: DragDirection::None,
            last_mouse_in_bounds: true,
        };

        widget
            .search_input
            .set_hint("Search (Enter to find, Esc to cancel)");

        widget.recalculate_status();
        widget
    }

    /* ******************************************************************
     * Public builder helpers
     * *****************************************************************/
    pub fn title(mut self, title: impl AsRef<str>) -> Self {
        self.title = title.as_ref().to_string();
        self.request_redraw();
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn wrap_indent(mut self, wrap_indent: usize) -> Self {
        self.wrap_indent = wrap_indent;
        self
    }

    pub fn set_borders(&mut self, borders: Borders) {
        self.borders = borders;
        self.request_redraw();
    }

    /// Force the widget to be considered dirty.
    pub fn redraw(&mut self) {
        self.request_redraw();
    }

    /// Adjust spaces inserted at the beginning of wrapped continuation lines.
    pub fn set_wrap_indent(&mut self, wrap_indent: usize) {
        if self.wrap_indent != wrap_indent {
            self.wrap_indent = wrap_indent;
            self.wrapped_lines_width = 0;
            self.request_redraw();
        }
    }

    /// Toggle line wrapping on/off.
    pub fn set_wrap_lines(&mut self, wrap_lines: bool) {
        if self.wrap_lines != wrap_lines {
            self.wrap_lines = wrap_lines;
            self.set_vertical_offset(self.vertical_offset.min(self.max_scroll_position()));
            self.wrapped_lines_width = 0;
            self.request_redraw();
            self.recalculate_status();
        }
    }

    /* ******************************************************************
     * Convenience helpers
     * *****************************************************************/
    fn set_border_color(&mut self) {
        self.border_color = if self.is_focused {
            tui_theme::BORDER_FOCUSED
        } else {
            tui_theme::BORDER_DEFAULT
        };

        self.border_style = Style::default().fg(self.border_color);
    }

    fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    fn recalculate_status(&mut self) {
        let mut parts = vec![];
        let mut _lines_from_bottom = String::new();
        if self.wrap_lines {
            parts.push("Wrap");
        } else {
            parts.push("Clip");
        }

        if self.search_mode.is_active() {
            if self.search_term.is_empty() {
                parts.push("Search");
            } else {
                parts.push("Filtering");
            }
        }

        if self.selection.is_active() {
            parts.push("Select");
        }

        if self.auto_scroll {
            parts.push("Auto");
        } else {
            let num_lines = self.lines_from_bottom();
            if num_lines > 0 {
                _lines_from_bottom = format!("+{num_lines}",);
                parts.push(&_lines_from_bottom)
            }
        }
        let info_text = format!("| {} |", parts.join(" | "));

        if info_text != self.info_text {
            self.info_text = info_text;
            self.request_redraw();
        }
    }

    pub fn set_title(&mut self, title: impl AsRef<str>) {
        let title = title.as_ref();
        if !self.title.eq(&title) {
            self.title = title.into();
            self.request_redraw();
        }
    }

    /* ******************************************************************
     * Buffer management
     * *****************************************************************/
    fn update_max_width(&mut self, max_width: usize) {
        if max_width > self.max_line_width {
            self.max_line_width = max_width;
            if self.horizontal_offset > self.max_line_width {
                self.horizontal_offset = self.max_line_width;
            }
            self.request_redraw();
        }
    }

    pub fn add_ansi_line(&mut self, entry: impl AsRef<str>) {
        self.add_styled_line(parse_ansi_string(entry));
    }

    pub fn add_ansi_lines<T: AsRef<str>>(&mut self, entries: impl IntoEitherIter<T>) {
        let entries = entries.into_either_iter();
        let parsed: Vec<_> = entries.map(parse_ansi_string).collect();
        if !parsed.is_empty() {
            self.add_styled_lines(parsed);
        }
    }

    pub fn add_styled_line(&mut self, line: StyledText) {
        let lines_removed = if self.buffer.len() >= self.line_capacity {
            1
        } else {
            0
        };

        if self.buffer.len() >= self.line_capacity {
            self.buffer.pop_front();
            self.lengths.pop_front();
        }

        self.update_max_width(line.len());
        self.lengths.push_back(line.len());
        self.buffer.push_back(line.chars);

        // Update selection after buffer change
        self.update_selection_after_buffer_change(lines_removed);

        self.update_search_highlights();
        self.invalidate_after_buffer_change();
        self.recalculate_status();
    }

    pub fn add_styled_lines<I: Into<StyledText>>(&mut self, items: impl IntoEitherIter<I>) {
        // Collect into Vec since we need to know length and potentially skip items
        let parsed: Vec<I> = items.into_either_iter().collect();

        if parsed.is_empty() {
            return;
        }

        let lines_removed;

        // Case 1: If incoming lines alone exceed capacity, take only the last N lines
        if parsed.len() >= self.line_capacity {
            // Clear existing buffer since we're replacing everything
            lines_removed = self.buffer.len(); // All existing lines are removed
            self.buffer.clear();
            self.lengths.clear();

            // Take only the last line_capacity lines from the new data
            let start_index = parsed.len() - self.line_capacity;
            for entry in parsed.into_iter().skip(start_index) {
                let entry: StyledText = entry.into();
                self.update_max_width(entry.len());
                self.lengths.push_back(entry.len());
                self.buffer.push_back(entry.chars);
            }
        } else {
            // Case 2: Adding to existing buffer - remove old lines if we'd exceed capacity
            let total_after_adding = self.buffer.len() + parsed.len();
            lines_removed = total_after_adding.saturating_sub(self.line_capacity);

            // Remove old lines from the front
            for _ in 0..lines_removed {
                self.buffer.pop_front();
                self.lengths.pop_front();
            }

            // Add all new lines
            for entry in parsed {
                let entry: StyledText = entry.into();
                self.update_max_width(entry.len());
                self.lengths.push_back(entry.len());
                self.buffer.push_back(entry.chars);
            }
        }

        // Update selection after buffer change
        self.update_selection_after_buffer_change(lines_removed);

        self.update_search_highlights();
        self.invalidate_after_buffer_change();
        self.recalculate_status();
    }

    fn update_selection_after_buffer_change(&mut self, lines_removed: usize) {
        if !self.selection.is_active() || lines_removed == 0 {
            return;
        }

        // If we removed all lines or more lines than we had, clear selection
        if lines_removed >= self.buffer.len() + lines_removed {
            self.selection.clear();
            self.mouse_is_down = false;
            return;
        }

        // Adjust selection positions
        let mut selection_invalid = false;

        // Update start position
        if self.selection.start.line < lines_removed {
            // Selection start was removed
            selection_invalid = true;
        } else {
            // Shift selection start by the number of lines removed
            self.selection.start.line = self.selection.start.line.saturating_sub(lines_removed);
        }

        // Update end position
        if self.selection.end.line < lines_removed {
            // Selection end was removed
            selection_invalid = true;
        } else {
            // Shift selection end by the number of lines removed
            self.selection.end.line = self.selection.end.line.saturating_sub(lines_removed);
        }

        // Clear selection if it became invalid
        if selection_invalid {
            self.selection.clear();
            self.mouse_is_down = false;
        } else {
            // Validate that the adjusted selection is still within bounds
            let buffer_len = self.buffer.len();
            if self.selection.start.line >= buffer_len || self.selection.end.line >= buffer_len {
                self.selection.clear();
                self.mouse_is_down = false;
            } else {
                // Clamp character indices to line lengths
                if self.selection.start.line < buffer_len {
                    let line_len = self.buffer[self.selection.start.line].len();
                    self.selection.start.char_idx = self.selection.start.char_idx.min(line_len);
                }
                if self.selection.end.line < buffer_len {
                    let line_len = self.buffer[self.selection.end.line].len();
                    self.selection.end.char_idx = self.selection.end.char_idx.min(line_len);
                }
            }
        }
    }

    /// Remove all content and reset scrolling state.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.lengths.clear();
        self.wrapped_lines.clear();
        self.wrapped_lines_width = 0;
        self.max_line_width = 0;
        self.vertical_offset = 0;
        self.horizontal_offset = 0;
        self.set_auto_scroll(true);
        self.search_term.clear();
        self.search_matches.clear();
        self.current_match = 0;

        // Clear selection when buffer is cleared
        self.selection.clear();
        self.mouse_is_down = false;

        self.request_redraw();
    }

    #[inline]
    fn invalidate_after_buffer_change(&mut self) {
        self.request_redraw();
        self.request_redraw();
        self.check_and_auto_scroll();
    }

    /* ******************************************************************
     * Search helpers
     * *****************************************************************/
    fn open_search(&mut self) {
        self.search_input.set_text(&self.search_term);
        self.focus_search();
        self.request_redraw();
    }

    fn focus_search(&mut self) {
        self.search_mode = SearchMode::Input;
        self.search_input.focus();
        self.recalculate_status();
        self.request_redraw();
        self.request_redraw();
    }

    fn unfocus_search(&mut self) {
        self.search_mode = SearchMode::Open;
        self.search_input.unfocus();
        self.recalculate_status();
        self.request_redraw();
        self.request_redraw();
    }

    fn close_search(&mut self) {
        self.search_mode = SearchMode::Closed;
        self.search_input.clear_and_unfocus();
        self.recalculate_status();
        self.request_redraw();
        self.request_redraw();
    }

    fn clear_search(&mut self) {
        self.search_term.clear();
        self.search_matches.clear();
        self.current_match = 0;
        self.close_search();
    }

    fn update_search_highlights(&mut self) {
        if self.search_mode.is_active() && !self.search_term.is_empty() {
            self.find_all_matches();
            self.redraw_search_status();
        }
    }

    fn redraw_search_status(&mut self) {
        if self.search_mode.is_active() {
            let text = if self.search_matches.is_empty() {
                if self.search_term.is_empty() {
                    "".to_string()
                } else {
                    "[no matches]".into()
                }
            } else {
                let total = self.search_matches.len();
                let current = if self.auto_scroll {
                    "-".to_string()
                } else {
                    format!("{}", self.current_match + 1)
                };
                format!("[{current}/{total}] ")
            };
            self.search_input.set_tl_text(text);
        } else {
            self.search_input.clear_tl_text();
        }
        self.request_redraw();
    }

    fn update_search_term(&mut self) {
        self.search_term = self.search_input.text().to_string();
        if self.search_term.is_empty() {
            self.search_matches.clear();
            self.current_match = 0;
        } else {
            self.find_all_matches();
            if !self.search_matches.is_empty() {
                self.current_match = 0;
                self.jump_to_current_match();
            }
        }
        self.redraw_search_status();
    }

    fn find_all_matches(&mut self) {
        self.search_matches.clear();

        for (idx, line) in self.buffer.iter().enumerate() {
            let plain: String = line.iter().map(|sc| sc.ch).collect();
            let mut start = 0;
            while let Some(pos) = plain[start..]
                .to_lowercase()
                .find(&self.search_term.to_lowercase())
            {
                let abs = start + pos;
                self.search_matches.push((idx, abs));
                start = abs + 1;
            }
        }
        self.request_redraw();
    }

    fn jump_to_current_match(&mut self) {
        if self.search_matches.is_empty() || self.current_match >= self.search_matches.len() {
            return;
        }

        let (line_idx, _) = self.search_matches[self.current_match];

        if self.wrap_lines {
            // translate to wrapped index
            let mut wrapped = 0;
            for i in 0..line_idx {
                let len = self.buffer[i].len();
                let segs = len.div_ceil(self.inner_width);
                wrapped += segs;
            }
            self.set_vertical_offset(wrapped);
        } else {
            self.set_vertical_offset(line_idx);
        }

        self.auto_scroll = false;
        self.request_redraw();
    }

    fn jump_to_next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.current_match = (self.current_match + 1) % self.search_matches.len();
        self.jump_to_current_match();
    }

    fn jump_to_prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.current_match == 0 {
            self.current_match = self.search_matches.len() - 1;
        } else {
            self.current_match -= 1;
        }
        self.jump_to_current_match();
    }

    /* ******************************************************************
     * Wrapping / scrolling helpers
     * *****************************************************************/
    #[inline]
    fn line_count(&self) -> usize {
        if self.wrap_lines {
            self.wrapped_lines.len()
        } else {
            self.buffer.len()
        }
    }

    #[inline]
    fn max_scroll_position(&self) -> usize {
        self.line_count().saturating_sub(self.inner_height)
    }

    fn set_auto_scroll(&mut self, enable: bool) {
        if self.auto_scroll != enable {
            if !enable {
                self.set_vertical_offset(self.max_scroll_position());
            }
            self.auto_scroll = enable;
            self.recalculate_status();
            self.request_redraw();
        }
    }

    fn check_and_auto_scroll(&mut self) {
        if self.auto_scroll {
            self.set_vertical_offset(self.max_scroll_position());
        }
    }

    pub fn lines_from_bottom(&self) -> usize {
        let total_lines = self.line_count();
        let current_bottom_line = self.vertical_offset + self.inner_height;

        total_lines.saturating_sub(current_bottom_line)
    }

    fn recalculate_scrollbars(&mut self) {
        self.v_scrollbar = self
            .v_scrollbar
            .content_length(self.max_scroll_position())
            .position(self.vertical_offset);

        self.h_scrollbar = self
            .h_scrollbar
            .content_length(self.max_line_width)
            .position(self.horizontal_offset);

        self.wrapped_lines_width = 0; // force re‑calc on next render
    }

    /* ******************************************************************
     * Public scrolling API (called from key / mouse events)
     * *****************************************************************/
    pub fn scroll_to_top(&mut self) {
        if self.set_vertical_offset(0) {
            self.set_auto_scroll(false);
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        if self.set_vertical_offset(self.max_scroll_position()) {
            self.set_auto_scroll(true);
        }
    }

    pub fn scroll_up(&mut self, offset: usize) {
        if self.set_vertical_offset(self.vertical_offset.saturating_sub(offset)) {
            self.set_auto_scroll(false);
        }
    }

    pub fn scroll_down(&mut self, offset: usize) {
        let max = self.max_scroll_position();
        if self.vertical_offset == max && offset > 0 {
            self.set_auto_scroll(true);
        }
        self.set_vertical_offset((self.vertical_offset + offset).min(max));
    }

    fn set_vertical_offset(&mut self, vertical_offset: usize) -> bool {
        if vertical_offset != self.vertical_offset {
            self.vertical_offset = vertical_offset;
            self.recalculate_status();
            self.request_redraw();
            true
        } else {
            false
        }
    }

    pub fn scroll_left(&mut self, offset: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(offset);
        self.request_redraw();
    }

    pub fn scroll_right(&mut self, offset: usize) {
        self.horizontal_offset = (self.horizontal_offset + offset).min(self.max_line_width);
        self.request_redraw();
    }

    /* ******************************************************************
     * Focus handling
     * *****************************************************************/
    fn apply_focus(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;

            // Reset cursor when losing focus
            if !focused {
                self.reset_cursor();
            }

            if self.search_mode.has_focus() {
                if focused {
                    self.search_input.focus();
                } else {
                    self.search_input.unfocus();
                }
            }
            self.set_border_color();
            self.request_redraw();
        }
    }
}

/* **********************************************************************
 *  Rendering helpers (unchanged logic, but now invoked conditionally)
 * *********************************************************************/

impl ScrollbackWidget {
    /* ---- helpers to clear underlying buffer when resizing ---- */
    fn clear_buffer(area: Rect, buf: &mut Buffer) {
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell_mut(Position::new(area.left() + x, area.top() + y)) {
                    cell.reset();
                }
            }
        }
    }

    /* ---- line‑number utilities ---- */
    fn calculate_line_num_width(&self, total_lines: usize) -> usize {
        if self.show_line_numbers {
            let digits = total_lines.to_string().len().min(4);
            digits.max(2)
        } else {
            0
        }
    }

    fn render_line_numbers(
        &self,
        buf: &mut Buffer,
        y: u16,
        inner_area: Rect,
        line_num: usize,
        ln_width: usize,
        is_continuation: bool,
    ) {
        if ln_width == 0 {
            return;
        }

        if !is_continuation {
            let s = format!("{line_num:>ln_width$}");
            for (x, ch) in s.chars().enumerate() {
                if let Some(cell) = buf.cell_mut(Position::new(inner_area.left() + x as u16, y)) {
                    cell.set_char(ch).set_style(self.line_number_style);
                }
            }
        } else {
            for x in 0..ln_width {
                if let Some(cell) = buf.cell_mut(Position::new(inner_area.left() + x as u16, y)) {
                    cell.set_char(' ').set_style(self.line_number_style);
                }
            }
        }

        // separator
        if let Some(cell) = buf.cell_mut(Position::new(inner_area.left() + ln_width as u16, y)) {
            cell.set_char('│').set_style(self.line_number_style);
        }
    }

    fn render_line_content(
        &self,
        buf: &mut Buffer,
        y: u16,
        content_start: u16,
        line: &[StyledChar],
        (start, end, line_idx): (usize, usize, usize),
        content_width: usize,
    ) {
        // clear line area
        for x in 0..content_width {
            if let Some(cell) = buf.cell_mut(Position::new(content_start + x as u16, y)) {
                cell.set_char(' ').set_style(Style::default());
            }
        }

        // Handle selection highlighting and search highlighting
        for (x, ch) in line[start..end].iter().enumerate() {
            let absolute_char_idx = start + x;
            let mut style = ch.style;

            // Check if this character is selected
            let is_selected = self
                .selection
                .contains_position(line_idx, absolute_char_idx);

            // Apply selection styling
            if is_selected {
                style = Style::default()
                    .fg(tui_theme::SELECTED_FG)
                    .bg(tui_theme::SELECTED_BG);
            }
            // Apply search highlighting if not selected (selection takes priority)
            else if self.search_mode.is_active() && !self.search_term.is_empty() {
                let plain: String = line.iter().map(|sc| sc.ch).collect();
                let lower = plain.to_lowercase();
                let s = self.search_term.to_lowercase();

                // Check if this character is part of a search match
                let mut is_search_match = false;
                let mut is_current_match = false;

                let mut pos = 0;
                while let Some(idx) = lower[pos..].find(&s) {
                    let m_start = pos + idx;
                    let m_end = m_start + s.len();

                    if absolute_char_idx >= m_start && absolute_char_idx < m_end {
                        is_search_match = true;

                        // Check if this is the current match
                        if let Some(&(match_line_idx, match_start)) =
                            self.search_matches.get(self.current_match)
                        {
                            if match_line_idx == line_idx && match_start == m_start {
                                is_current_match = true;
                            }
                        }
                        break;
                    }

                    pos = m_start + 1;
                    if pos >= plain.len() {
                        break;
                    }
                }

                if is_search_match {
                    if is_current_match {
                        style = Style::default()
                            .fg(tui_theme::CURRENT_MATCH_COLOR)
                            .bg(Color::DarkGray);
                    } else {
                        style = Style::default().fg(tui_theme::SEARCH_HIGHLIGHT_COLOR);
                    }
                }
            }

            if let Some(cell) = buf.cell_mut(Position::new(content_start + x as u16, y)) {
                cell.set_char(ch.ch).set_style(style);
            }
        }
    }

    /* ---- non‑wrapped render ---- */
    fn render_lines_clipped(&self, inner: Rect, buf: &mut Buffer) {
        let max_h = inner.height as usize;
        let max_w = inner.width as usize;
        let total_lines = self.buffer.len();

        let start_line = self.vertical_offset.min(total_lines.saturating_sub(max_h));
        let end_line = (start_line + max_h).min(total_lines);

        let ln_width = self.calculate_line_num_width(total_lines + 1);
        let content_w = max_w.saturating_sub(if ln_width > 0 { ln_width + 1 } else { 0 });

        for (i, line) in self
            .buffer
            .iter()
            .skip(start_line)
            .take(end_line - start_line)
            .enumerate()
        {
            let idx = start_line + i;
            let y = inner.top() + i as u16;
            self.render_line_numbers(buf, y, inner, idx + 1, ln_width, false);

            let content_start = if ln_width > 0 {
                inner.left() + (ln_width + 1) as u16
            } else {
                inner.left()
            };
            let start_char = self.horizontal_offset.min(line.len());
            let end_char = line.len().min(start_char + content_w);
            self.render_line_content(
                buf,
                y,
                content_start,
                line,
                (start_char, end_char, idx),
                content_w,
            );
        }
    }

    /* ---- wrapped render ---- */
    fn render_lines_wrapped(&mut self, inner: Rect, buf: &mut Buffer) {
        let max_h = inner.height as usize;
        let max_w = inner.width as usize;
        let orig_lines = self.buffer.len();

        let ln_width = self.calculate_line_num_width(orig_lines);
        let content_w = max_w.saturating_sub(if ln_width > 0 { ln_width + 1 } else { 0 });
        if content_w == 0 {
            return;
        }

        let needs_recalc = self.wrapped_lines_width != content_w
            || self
                .wrapped_lines
                .last()
                .map(|(idx, _, _)| *idx + 1 != self.buffer.len())
                .unwrap_or(!self.buffer.is_empty());

        if needs_recalc {
            self.wrapped_lines.clear();

            for (orig_idx, line) in self.buffer.iter().enumerate() {
                let first_w = content_w;
                let rest_w = content_w.saturating_sub(self.wrap_indent);

                if line.is_empty() {
                    self.wrapped_lines.push((orig_idx, 0, 0));
                    continue;
                }

                let mut pos = 0;
                let seg_end = find_break(line, pos, first_w);
                self.wrapped_lines.push((orig_idx, pos, seg_end));
                pos = seg_end;

                while pos < line.len() {
                    let end = find_break(line, pos, rest_w);
                    self.wrapped_lines.push((orig_idx, pos, end));
                    pos = end;
                }
            }
            self.wrapped_lines_width = content_w;
            if self.auto_scroll {
                self.set_vertical_offset(self.max_scroll_position());
            }
        }

        fn find_break(line: &[StyledChar], start: usize, limit: usize) -> usize {
            if start + limit >= line.len() {
                return line.len();
            }
            let end = start + limit;
            for i in (start..end).rev() {
                if line[i].ch == ' ' {
                    return i + 1;
                }
            }
            if start == end { start + 1 } else { end }
        }

        let total = self.wrapped_lines.len();
        let start = self.vertical_offset.min(total.saturating_sub(max_h));
        let end = (start + max_h).min(total);

        let mut prev_orig = usize::MAX;

        for (render_idx, wrapped_idx) in (start..end).enumerate() {
            let (orig_idx, start_char, end_char) = self.wrapped_lines[wrapped_idx];
            let y = inner.top() + render_idx as u16;
            let is_first = orig_idx != prev_orig;
            prev_orig = orig_idx;

            self.render_line_numbers(buf, y, inner, orig_idx + 1, ln_width, !is_first);

            let mut content_start = if ln_width > 0 {
                inner.left() + (ln_width + 1) as u16
            } else {
                inner.left()
            };
            if start_char != 0 {
                content_start += self.wrap_indent as u16;
            }

            let line = &self.buffer[orig_idx];
            self.render_line_content(
                buf,
                y,
                content_start,
                line,
                (start_char, end_char, orig_idx),
                content_w,
            );
        }
    }

    /* ---- outer widgets (frame, scrollbars, search) ---- */
    fn render_outer_frame(&mut self, inner: Rect, area: Rect, buf: &mut Buffer) {
        let mut block = Block::bordered()
            .borders(self.borders)
            .title(self.title.as_str())
            .border_type(BorderType::Rounded)
            .border_style(self.border_style);

        if self.dev_mode {
            let Rect {
                x,
                y,
                width,
                height,
            } = area;
            let area_info = format!("{x},{y} {width}x{height}");
            let scroll_info = format!(
                "V:{}/{} H:{}/{}",
                self.vertical_offset,
                self.line_count(),
                self.horizontal_offset,
                self.max_line_width
            );
            let line_info = format!("B:{} W:{}", self.buffer.len(), self.wrapped_lines.len());
            block = block.title_top(
                Line::from(Span::raw(format!(
                    "{area_info}  {scroll_info}  {line_info}  {}",
                    self.info_text
                )))
                .right_aligned(),
            );
        } else {
            block = block.title_top(Line::from(Span::raw(&self.info_text)).right_aligned());
        }

        block.render(area, buf);

        // scrollbars
        self.render_v_scrollbar(inner, area, buf);
        self.render_h_scrollbar(area, buf);
    }

    fn render_v_scrollbar(&mut self, inner: Rect, area: Rect, buf: &mut Buffer) {
        if self.line_count() > inner.height as usize {
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .end_symbol(None)
                .begin_symbol(None)
                .track_symbol(Some(line::VERTICAL))
                .track_style(self.border_style)
                .thumb_style(self.scrollbar_style)
                .render(area.inner(Margin::new(0, 1)), buf, &mut self.v_scrollbar);
        }
    }

    fn render_h_scrollbar(&mut self, area: Rect, buf: &mut Buffer) {
        if !self.wrap_lines {
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
                .thumb_symbol(tui_theme::THUMB_SYMBOL)
                .end_symbol(None)
                .begin_symbol(None)
                .track_symbol(Some(line::HORIZONTAL))
                .track_style(self.border_style)
                .thumb_style(self.scrollbar_style)
                .render(area.inner(Margin::new(1, 0)), buf, &mut self.h_scrollbar);
        }
    }

    fn render_search_input(&mut self, area: Rect, buf: &mut Buffer) {
        if self.search_mode.is_active() {
            let input_h = 3;
            let input_area = Rect {
                x: area.x + 1,
                y: area.y + area.height - input_h,
                width: area.width - 2,
                height: input_h,
            };
            self.search_input.draw(input_area, buf);
        }
    }
}

impl ScrollbackWidget {
    fn is_point_in_vertical_thumb(&self, x: u16, y: u16) -> bool {
        if !self.is_point_in_vertical_scrollbar(x, y) {
            return false;
        }

        let (thumb_start, thumb_end) = self.get_vertical_thumb_position();
        y >= thumb_start && y < thumb_end
    }

    fn is_point_in_horizontal_thumb(&self, x: u16, y: u16) -> bool {
        if !self.is_point_in_horizontal_scrollbar(x, y) {
            return false;
        }

        let (thumb_start, thumb_end) = self.get_horizontal_thumb_position();
        x >= thumb_start && x < thumb_end
    }

    fn handle_vertical_scrollbar_click(&mut self, y: u16) {
        let (thumb_start, thumb_end) = self.get_vertical_thumb_position();

        if y < thumb_start {
            // Click above thumb - page up
            self.scroll_up(self.inner_height);
        } else if y >= thumb_end {
            // Click below thumb - page down
            self.scroll_down(self.inner_height);
        }
    }

    fn handle_horizontal_scrollbar_click(&mut self, x: u16) {
        let (thumb_start, thumb_end) = self.get_horizontal_thumb_position();

        if x < thumb_start {
            // Click left of thumb - page left
            self.scroll_left(self.inner_width);
        } else if x >= thumb_end {
            // Click right of thumb - page right
            self.scroll_right(self.inner_width);
        }
    }

    fn get_vertical_thumb_position(&self) -> (u16, u16) {
        let area = self.last_area;
        let scrollbar_height = area.height.saturating_sub(2);
        let content_height = self.line_count();
        let visible_height = self.inner_height;

        if content_height <= visible_height || scrollbar_height == 0 {
            return (area.top() + 1, area.top() + 1);
        }

        // Use saturating arithmetic and check for zero division
        let thumb_size = if content_height == 0 {
            1
        } else {
            ((scrollbar_height as u32 * visible_height as u32) / content_height as u32)
                .min(scrollbar_height as u32) as u16
        }
        .max(1);

        let scrollbar_range = scrollbar_height.saturating_sub(thumb_size);
        if scrollbar_range == 0 {
            return (area.top() + 1, area.top() + 1 + thumb_size);
        }

        let scroll_range = content_height.saturating_sub(visible_height);
        let thumb_pos = if scroll_range == 0 {
            0
        } else {
            ((self.vertical_offset as u32 * scrollbar_range as u32) / scroll_range as u32)
                .min(scrollbar_range as u32) as u16
        };

        let thumb_start = area.top() + 1 + thumb_pos;
        let thumb_end = thumb_start + thumb_size;

        (thumb_start, thumb_end)
    }

    fn get_horizontal_thumb_position(&self) -> (u16, u16) {
        let area = self.last_area;
        let scrollbar_width = area.width.saturating_sub(2);
        let content_width = self.max_line_width;
        let visible_width = self.inner_width;

        if content_width <= visible_width || scrollbar_width == 0 {
            return (area.left() + 1, area.left() + 1);
        }

        // Use saturating arithmetic and check for zero division
        let thumb_size = if content_width == 0 {
            1
        } else {
            ((scrollbar_width as u32 * visible_width as u32) / content_width as u32)
                .min(scrollbar_width as u32) as u16
        }
        .max(1);

        let scrollbar_range = scrollbar_width.saturating_sub(thumb_size);
        if scrollbar_range == 0 {
            return (area.left() + 1, area.left() + 1 + thumb_size);
        }

        let scroll_range = content_width.saturating_sub(visible_width);
        let thumb_pos = if scroll_range == 0 {
            0
        } else {
            ((self.horizontal_offset as u32 * scrollbar_range as u32) / scroll_range as u32)
                .min(scrollbar_range as u32) as u16
        };

        let thumb_start = area.left() + 1 + thumb_pos;
        let thumb_end = thumb_start + thumb_size;

        (thumb_start, thumb_end)
    }

    fn handle_vertical_scrollbar_drag(&mut self, y: u16, drag_offset: u16) {
        let area = self.last_area;
        let scrollbar_height = area.height.saturating_sub(2);
        let content_height = self.line_count();
        let visible_height = self.inner_height;

        if content_height <= visible_height || scrollbar_height == 0 {
            return;
        }

        let thumb_size = if content_height == 0 {
            1
        } else {
            ((scrollbar_height as u32 * visible_height as u32) / content_height as u32)
                .min(scrollbar_height as u32) as u16
        }
        .max(1);

        let scrollbar_range = scrollbar_height.saturating_sub(thumb_size);
        if scrollbar_range == 0 {
            return;
        }

        // Calculate desired thumb position based on mouse position and drag offset
        let mouse_relative_y = y.saturating_sub(area.top() + 1);
        let desired_thumb_y = mouse_relative_y.saturating_sub(drag_offset);
        let clamped_thumb_y = desired_thumb_y.min(scrollbar_range);

        // Convert thumb position to scroll offset with overflow protection
        let scroll_range = content_height.saturating_sub(visible_height);
        let new_offset = if scrollbar_range == 0 {
            0
        } else {
            ((clamped_thumb_y as u32 * scroll_range as u32) / scrollbar_range as u32) as usize
        };

        self.set_auto_scroll(false);
        self.set_vertical_offset(new_offset.min(self.max_scroll_position()));
        self.request_redraw();
    }

    fn handle_horizontal_scrollbar_drag(&mut self, x: u16, drag_offset: u16) {
        let area = self.last_area;
        let scrollbar_width = area.width.saturating_sub(2);
        let content_width = self.max_line_width;
        let visible_width = self.inner_width;

        if content_width <= visible_width || scrollbar_width == 0 {
            return;
        }

        let thumb_size = if content_width == 0 {
            1
        } else {
            ((scrollbar_width as u32 * visible_width as u32) / content_width as u32)
                .min(scrollbar_width as u32) as u16
        }
        .max(1);

        let scrollbar_range = scrollbar_width.saturating_sub(thumb_size);
        if scrollbar_range == 0 {
            return;
        }

        // Calculate desired thumb position based on mouse position and drag offset
        let mouse_relative_x = x.saturating_sub(area.left() + 1);
        let desired_thumb_x = mouse_relative_x.saturating_sub(drag_offset);
        let clamped_thumb_x = desired_thumb_x.min(scrollbar_range);

        // Convert thumb position to scroll offset with overflow protection
        let scroll_range = content_width.saturating_sub(visible_width);
        let new_offset = if scrollbar_range == 0 {
            0
        } else {
            ((clamped_thumb_x as u32 * scroll_range as u32) / scrollbar_range as u32) as usize
        };

        self.horizontal_offset = new_offset.min(self.max_line_width);
        self.request_redraw();
    }

    fn is_point_in_vertical_scrollbar(&self, x: u16, y: u16) -> bool {
        if self.line_count() <= self.inner_height {
            return false; // No scrollbar if content fits
        }

        let area = self.last_area;
        let scrollbar_x = area.right().saturating_sub(1);
        let scrollbar_top = area.top().saturating_add(1);
        let scrollbar_bottom = area.bottom().saturating_sub(1);

        x == scrollbar_x && y >= scrollbar_top && y < scrollbar_bottom
    }

    fn is_point_in_horizontal_scrollbar(&self, x: u16, y: u16) -> bool {
        if self.wrap_lines || self.max_line_width <= self.inner_width {
            return false; // No scrollbar in wrap mode or if content fits
        }

        let area = self.last_area;
        let scrollbar_y = area.bottom().saturating_sub(1);
        let scrollbar_left = area.left().saturating_add(1);
        let scrollbar_right = area.right().saturating_sub(1);

        y == scrollbar_y && x >= scrollbar_left && x < scrollbar_right
    }
}
