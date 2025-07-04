// tokio-tui/src/widgets/scrollbox/tabbed_scrollbox.rs
//! src/widgets/scrollbox/tabbed_scrollbox.rs
//!
//! FINAL cleaned‑up TabbedScrollbox **including ALL legacy helpers** so
//! tracer_widget and other older modules compile without changes.
//!
//! --------------------------------------------------------------------
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;

use crossterm::event::KeyModifiers;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, MouseEvent},
    layout::Rect,
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Borders, Widget as _},
};

use crate::{
    IntoEitherIter, OverflowMode, ScrollbackWidget, StyledText, TabsWidget, TuiWidget, tui_theme,
};

/* **********************************************************************
 * Main struct
 * *********************************************************************/
pub struct TabbedScrollbox<T: Send + Sync + Hash + Eq + Clone + Display + 'static> {
    /* data */
    tabs: HashMap<T, ScrollbackWidget>,
    tab_order: Vec<T>,
    tab_titles: HashMap<T, String>,
    selected_tab: usize,

    /* appearance */
    style: Style,
    border_color: Color,
    border_style: Style,
    tab_divider: String,
    tab_padding_left: String,
    tab_padding_right: String,

    /* options */
    overflow_mode: OverflowMode,
    title: String,
    borders: Borders,
    wrap_indent: usize,
    wrap_lines: bool,

    /* runtime */
    rendered_tab_titles: Vec<String>,
    titles_cache_dirty: bool,
    redraw_requested: bool,
    is_focused: bool,
}

impl<T: Send + Sync + Hash + Eq + Clone + Display + 'static> TabbedScrollbox<T> {
    pub fn new(title: impl AsRef<str>) -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            tab_titles: HashMap::new(),
            selected_tab: 0,
            style: Style::default(),
            border_color: tui_theme::BORDER_DEFAULT,
            border_style: Style::default().fg(tui_theme::BORDER_DEFAULT),
            tab_divider: symbols::line::VERTICAL.to_string(),
            tab_padding_left: " ".into(),
            tab_padding_right: " ".into(),
            overflow_mode: OverflowMode::Scroll,
            title: title.as_ref().into(),
            borders: Borders::all(),
            wrap_indent: 0,
            wrap_lines: false,
            rendered_tab_titles: Vec::new(),
            titles_cache_dirty: true,
            redraw_requested: true,
            is_focused: false,
        }
    }

    /* ******************************************************************
     * Builder helpers
     * *****************************************************************/
    pub fn with_borders(mut self, borders: Borders) -> Self {
        self.set_borders(borders);
        self
    }
    pub fn with_wrap_indent(mut self, indent: usize) -> Self {
        self.set_wrap_indent(indent);
        self
    }
    pub fn with_wrap_lines(mut self, wrap: bool) -> Self {
        self.set_all_wrap_lines(wrap);
        self
    }
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn title(mut self, title: impl AsRef<str>) -> Self {
        self.title = title.as_ref().into();
        self
    }
    pub fn tab_divider(mut self, divider: impl AsRef<str>) -> Self {
        self.tab_divider = divider.as_ref().into();
        self
    }
    pub fn tab_padding(mut self, left: impl AsRef<str>, right: impl AsRef<str>) -> Self {
        self.tab_padding_left = left.as_ref().into();
        self.tab_padding_right = right.as_ref().into();
        self
    }
    pub fn overflow_mode(mut self, mode: OverflowMode) -> Self {
        self.overflow_mode = mode;
        self
    }

    /* ******************************************************************
     * Internal helpers
     * *****************************************************************/

    /* internal utilities */
    #[inline]
    fn request_redraw(&mut self) {}
    #[inline]
    pub fn current_scrollbox_mut(&mut self) -> Option<&mut ScrollbackWidget> {
        self.tab_order
            .get(self.selected_tab)
            .and_then(|n| self.tabs.get_mut(n))
    }
    #[inline]
    pub fn current_scrollbox_ref(&self) -> Option<&ScrollbackWidget> {
        self.tab_order
            .get(self.selected_tab)
            .and_then(|n| self.tabs.get(n))
    }
    #[inline]
    fn set_border_color(&mut self) {
        self.border_color = if self.is_focused {
            tui_theme::BORDER_FOCUSED
        } else {
            tui_theme::BORDER_DEFAULT
        };
        self.border_style = Style::default().fg(self.border_color);
    }

    fn sync_child_state(&mut self) {
        let is_focused = self.is_focused; // <- borrow first!
        if let Some(sb) = self.current_scrollbox_mut() {
            if is_focused {
                sb.focus();
            } else {
                sb.unfocus();
            }
            sb.redraw();
        }
    }

    /* ******************************************************************
     * Public tab/scrollbox management
     * *****************************************************************/
    pub fn add_tab(&mut self, name: impl Into<T>, title: impl AsRef<str>) -> &mut Self {
        let mut sb = ScrollbackWidget::new("", 1000).wrap_indent(self.wrap_indent);
        sb.set_borders(self.borders);
        sb.set_wrap_indent(self.wrap_indent);
        sb.set_wrap_lines(self.wrap_lines);

        let name: T = name.into();
        if !title.as_ref().is_empty() {
            self.tab_titles.insert(name.clone(), title.as_ref().into());
        }

        self.tabs.insert(name.clone(), sb);
        self.tab_order.push(name);
        self.titles_cache_dirty = true;
        self.request_redraw();
        self
    }

    pub fn select_tab(&mut self, name: &T) -> &mut Self {
        if let Some(idx) = self.tab_order.iter().position(|n| n == name) {
            self.selected_tab = idx;
            self.sync_child_state();
            self.request_redraw();
        }
        self
    }
    pub fn select_tab_index(&mut self, idx: usize) -> &mut Self {
        if idx < self.tab_order.len() {
            self.selected_tab = idx;
            self.sync_child_state();
            self.request_redraw();
        }
        self
    }
    pub fn next_tab(&mut self) -> &mut Self {
        if !self.tab_order.is_empty() {
            self.selected_tab = (self.selected_tab + 1) % self.tab_order.len();
            self.sync_child_state();
            self.request_redraw();
        }
        self
    }
    pub fn prev_tab(&mut self) -> &mut Self {
        if !self.tab_order.is_empty() {
            self.selected_tab = self
                .selected_tab
                .checked_sub(1)
                .unwrap_or(self.tab_order.len() - 1);
            self.sync_child_state();
            self.request_redraw();
        }
        self
    }

    /* ---- style after construction ---- */
    pub fn set_borders(&mut self, borders: Borders) {
        self.borders = borders;
        for sb in self.tabs.values_mut() {
            sb.set_borders(borders);
        }
        self.request_redraw();
    }
    pub fn set_wrap_indent(&mut self, indent: usize) {
        self.wrap_indent = indent;
        for sb in self.tabs.values_mut() {
            sb.set_wrap_indent(indent);
        }
    }
    pub fn set_all_wrap_lines(&mut self, wrap: bool) {
        self.wrap_lines = wrap;
        for sb in self.tabs.values_mut() {
            sb.set_wrap_lines(wrap);
        }
    }

    /* ******************************************************************
     * Content helpers for CURRENT tab
     * *****************************************************************/
    /* --- ergonomic content helpers (iterator) --- */

    /* --- legacy helpers for specific tabs (iterator aware) --- */
    pub fn tab_exists(&self, name: &T) -> bool {
        self.tabs.contains_key(name)
    }
    pub fn get_tab_mut(&mut self, name: &T) -> Option<&mut ScrollbackWidget> {
        self.tabs.get_mut(name)
    }

    pub fn add_ansi_to_tab<I: AsRef<str>>(&mut self, name: &T, entries: impl IntoEitherIter<I>) {
        if let Some(sb) = self.get_tab_mut(name) {
            sb.add_ansi_lines(entries);
        }
    }
    pub fn add_styled_to_tab<I: Into<StyledText>>(
        &mut self,
        name: &T,

        entries: impl IntoEitherIter<I>,
    ) {
        if let Some(sb) = self.get_tab_mut(name) {
            sb.add_styled_lines(entries);
        }
    }

    pub fn add_ansi_to_current<I: IntoEitherIter<String>>(&mut self, entries: I) {
        if let Some(sb) = self.current_scrollbox_mut() {
            sb.add_ansi_lines(entries);
        }
    }
    pub fn add_styled_to_current<I>(&mut self, entries: I)
    where
        I: IntoEitherIter<StyledText>,
    {
        if let Some(sb) = self.current_scrollbox_mut() {
            sb.add_styled_lines(entries);
        }
    }

    pub fn clear_current_tab(&mut self) -> bool {
        if let Some(sb) = self.current_scrollbox_mut() {
            sb.clear();
            true
        } else {
            false
        }
    }
}

/* **********************************************************************
 * TuiWidget implementation
 * *********************************************************************/
impl<T: Send + Sync + Hash + Eq + Clone + Display + 'static> TuiWidget for TabbedScrollbox<T> {
    fn need_draw(&self) -> bool {
        self.redraw_requested
            || self
                .tab_order
                .get(self.selected_tab)
                .and_then(|name| self.tabs.get(name))
                .is_some_and(|sb| sb.need_draw())
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        if self.tab_order.is_empty() {
            return;
        }

        if self.titles_cache_dirty {
            self.rendered_tab_titles = self
                .tab_order
                .iter()
                .map(|name| {
                    self.tab_titles
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| name.to_string())
                })
                .collect();
            self.titles_cache_dirty = false;
        }

        /* child */
        if let Some(sb) = self.current_scrollbox_mut() {
            sb.draw(area, buf);
        }

        /* tabs */
        let tabs_area = Rect::new(area.x + 1, area.y, area.width, 1);
        let lines: Vec<Line> = self
            .rendered_tab_titles
            .iter()
            .map(|t| Line::from(Span::raw(t)))
            .collect();

        TabsWidget::new(lines)
            .select(self.selected_tab)
            .divider(&self.tab_divider)
            .padding(
                self.tab_padding_left.as_str(),
                self.tab_padding_right.as_str(),
            )
            .overflow_mode(self.overflow_mode)
            .highlight_style(Style::default().fg(tui_theme::ACTIVE_FG))
            .render(tabs_area, buf);

        self.redraw_requested = false;
    }

    fn mouse_event(&mut self, mouse: MouseEvent) -> bool {
        self.current_scrollbox_mut()
            .is_some_and(|sb| sb.mouse_event(mouse))
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.prev_tab();
                } else {
                    self.next_tab();
                }
                true
            }
            _ => self
                .current_scrollbox_mut()
                .is_some_and(|sb| sb.key_event(key)),
        }
    }

    fn focus(&mut self) {
        self.is_focused = true;
        self.set_border_color();
        self.sync_child_state();
        self.request_redraw();
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
        self.set_border_color();
        self.sync_child_state();
        self.request_redraw();
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
