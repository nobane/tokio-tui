// tokio-tui/src/widgets/tabs/tabs_widget.rs

use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Position, Rect},
    style::{Modifier, Style, Styled},
    symbols,
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::TuiWidget;

const DEFAULT_HIGHLIGHT_STYLE: Style = Style::new().add_modifier(Modifier::REVERSED);

/// Controls how tabs are handled when they don't fit in the available width
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum OverflowMode {
    /// Don't handle overflow - just show as many tabs as fit (original behavior)
    None,
    /// Scroll tabs horizontally to ensure the selected tab is visible
    Scroll,
    /// Wrap tabs to multiple lines when they don't fit on a single line
    Wrap,
}

impl Default for OverflowMode {
    fn default() -> Self {
        Self::None
    }
}

/// A widget that displays tabs with overflow handling capabilities.
///
/// This widget extends the functionality of the standard `Tabs` widget by adding
/// support for handling tabs that don't fit in the available width.
///
/// # Example
///
/// ```
/// use ratatui::{
///     style::{Style, Stylize},
///     symbols,
///     widgets::{Block, TabsWidget, OverflowMode},
/// };
///
/// TabsWidget::new(vec!["Tab1", "Tab2", "Tab3", "Tab4", "Tab5", "Tab6", "Tab7", "Tab8"])
///     .block(Block::bordered().title("Tabs"))
///     .style(Style::default().white())
///     .highlight_style(Style::default().yellow())
///     .select(5)
///     .divider(symbols::DOT)
///     .overflow_mode(OverflowMode::Scroll)
///     .padding("->", "<-");
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TabsWidget<'a> {
    /// A block to wrap this widget in if necessary
    block: Option<Block<'a>>,
    /// One title for each tab
    titles: Vec<Line<'a>>,
    /// The index of the selected tabs
    selected: Option<usize>,
    /// The style used to draw the text
    style: Style,
    /// Style to apply to the selected item
    highlight_style: Style,
    /// Tab divider
    divider: Span<'a>,
    /// Tab Left Padding
    padding_left: Line<'a>,
    /// Tab Right Padding
    padding_right: Line<'a>,
    /// Mode for handling overflow
    overflow_mode: OverflowMode,
    /// Left indicator for scrolling mode (e.g., "«")
    scroll_left_indicator: Span<'a>,
    /// Right indicator for scrolling mode (e.g., "»")
    scroll_right_indicator: Span<'a>,
    /// Whether the widget is focused
    is_focused: bool,
}

impl Default for TabsWidget<'_> {
    fn default() -> Self {
        Self::new(Vec::<Line>::new())
    }
}

impl<'a> TabsWidget<'a> {
    /// Creates new `TabsWidget` from their titles.
    ///
    /// `titles` can be a [`Vec`] of [`&str`], [`String`] or anything that can be converted into
    /// [`Line`]. As such, titles can be styled independently.
    ///
    /// The selected tab can be set with [`TabsWidget::select`]. The first tab has index 0 (this is also
    /// the default index).
    pub fn new<Iter>(titles: Iter) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Line<'a>>,
    {
        let titles = titles.into_iter().map(Into::into).collect_vec();
        let selected = if titles.is_empty() { None } else { Some(0) };
        Self {
            block: None,
            titles,
            selected,
            style: Style::default(),
            highlight_style: DEFAULT_HIGHLIGHT_STYLE,
            divider: Span::raw(symbols::line::VERTICAL),
            padding_left: Line::from(" "),
            padding_right: Line::from(" "),
            overflow_mode: OverflowMode::default(),
            scroll_left_indicator: Span::raw("«"),
            scroll_right_indicator: Span::raw("»"),
            is_focused: false,
        }
    }

    /// Sets the titles of the tabs.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn titles<Iter>(mut self, titles: Iter) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Line<'a>>,
    {
        self.titles = titles.into_iter().map(Into::into).collect_vec();
        self.selected = if self.titles.is_empty() {
            None
        } else {
            // Ensure selected is within bounds, and default to 0 if no selected tab
            self.selected
                .map(|selected| selected.min(self.titles.len() - 1))
                .or(Some(0))
        };
        self
    }

    /// Mutable access to set the titles
    pub fn set_titles<Iter>(&mut self, titles: Iter)
    where
        Iter: IntoIterator,
        Iter::Item: Into<Line<'a>>,
    {
        self.titles = titles.into_iter().map(Into::into).collect_vec();
        self.selected = if self.titles.is_empty() {
            None
        } else {
            // Ensure selected is within bounds, and default to 0 if no selected tab
            self.selected
                .map(|selected| selected.min(self.titles.len() - 1))
                .or(Some(0))
        };
    }

    /// Surrounds the `TabsWidget` with a [`Block`].
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Mutable access to set the block
    pub fn set_block(&mut self, block: Block<'a>) {
        self.block = Some(block);
    }

    /// Sets the selected tab.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn select<T: Into<Option<usize>>>(mut self, selected: T) -> Self {
        self.selected = selected.into();
        self
    }

    /// Mutable access to set the selected tab
    pub fn set_selected(&mut self, selected: Option<usize>) {
        self.selected = selected.map(|idx| idx.min(self.titles.len().saturating_sub(1)));
    }

    /// Returns the currently selected tab index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    /// Sets the selected tab without consuming self
    pub fn set_select(&mut self, selected: impl Into<Option<usize>>) {
        self.selected = selected.into();
        if let Some(idx) = self.selected {
            if idx >= self.titles.len() && !self.titles.is_empty() {
                self.selected = Some(self.titles.len() - 1);
            }
        }
    }

    /// Sets the style of the tabs.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Mutable access to set the style
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// Sets the style for the highlighted tab.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn highlight_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.highlight_style = style.into();
        self
    }

    /// Mutable access to set the highlight style
    pub fn set_highlight_style(&mut self, style: Style) {
        self.highlight_style = style;
    }

    /// Sets the string to use as tab divider.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn divider<T>(mut self, divider: T) -> Self
    where
        T: Into<Span<'a>>,
    {
        self.divider = divider.into();
        self
    }

    /// Mutable access to set the divider
    pub fn set_divider<T>(&mut self, divider: T)
    where
        T: Into<Span<'a>>,
    {
        self.divider = divider.into();
    }

    /// Sets the padding between tabs.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn padding<T, U>(mut self, left: T, right: U) -> Self
    where
        T: Into<Line<'a>>,
        U: Into<Line<'a>>,
    {
        self.padding_left = left.into();
        self.padding_right = right.into();
        self
    }

    /// Mutable access to set the padding between tabs
    pub fn set_padding<T, U>(&mut self, left: T, right: U)
    where
        T: Into<Line<'a>>,
        U: Into<Line<'a>>,
    {
        self.padding_left = left.into();
        self.padding_right = right.into();
    }

    /// Sets the left side padding between tabs.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn padding_left<T>(mut self, padding: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.padding_left = padding.into();
        self
    }

    /// Mutable access to set the left padding
    pub fn set_padding_left<T>(&mut self, padding: T)
    where
        T: Into<Line<'a>>,
    {
        self.padding_left = padding.into();
    }

    /// Sets the right side padding between tabs.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn padding_right<T>(mut self, padding: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.padding_right = padding.into();
        self
    }

    /// Mutable access to set the right padding
    pub fn set_padding_right<T>(&mut self, padding: T)
    where
        T: Into<Line<'a>>,
    {
        self.padding_right = padding.into();
    }

    /// Sets the overflow mode for handling tabs that don't fit.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn overflow_mode(mut self, mode: OverflowMode) -> Self {
        self.overflow_mode = mode;
        self
    }

    /// Mutable access to set the overflow mode
    pub fn set_overflow_mode(&mut self, mode: OverflowMode) {
        self.overflow_mode = mode;
    }

    /// Sets the indicators used for scroll mode.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn scroll_indicators<T, U>(mut self, left: T, right: U) -> Self
    where
        T: Into<Span<'a>>,
        U: Into<Span<'a>>,
    {
        self.scroll_left_indicator = left.into();
        self.scroll_right_indicator = right.into();
        self
    }

    /// Mutable access to set the scroll indicators
    pub fn set_scroll_indicators<T, U>(&mut self, left: T, right: U)
    where
        T: Into<Span<'a>>,
        U: Into<Span<'a>>,
    {
        self.scroll_left_indicator = left.into();
        self.scroll_right_indicator = right.into();
    }

    /// Select the next tab
    pub fn next_tab(&mut self) {
        if self.titles.is_empty() {
            return;
        }

        self.selected = match self.selected {
            Some(idx) if idx + 1 < self.titles.len() => Some(idx + 1),
            _ => Some(0), // Wrap around to first tab
        };
    }

    /// Select the previous tab
    pub fn prev_tab(&mut self) {
        if self.titles.is_empty() {
            return;
        }

        self.selected = match self.selected {
            Some(idx) if idx > 0 => Some(idx - 1),
            _ => Some(self.titles.len().saturating_sub(1)), // Wrap around to last tab
        };
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.titles.len()
    }

    /// Set a specific title at the given index
    pub fn set_title_at(&mut self, index: usize, title: impl Into<Line<'a>>) {
        if index < self.titles.len() {
            self.titles[index] = title.into();
        }
    }

    /// Add a new tab
    pub fn add_tab(&mut self, title: impl Into<Line<'a>>) {
        self.titles.push(title.into());

        // If this is the first tab, select it by default
        if self.titles.len() == 1 {
            self.selected = Some(0);
        }
    }

    /// Remove a tab at the given index
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.titles.len() {
            self.titles.remove(index);

            // Adjust selected index if needed
            if let Some(selected) = self.selected {
                if selected >= self.titles.len() {
                    self.selected = if self.titles.is_empty() {
                        None
                    } else {
                        Some(self.titles.len() - 1)
                    };
                } else if selected == index && selected > 0 {
                    // If we removed the selected tab, select the previous one
                    self.selected = Some(selected - 1);
                }
            }
        }
    }

    // Calculate the widths of all tabs including padding
    fn calculate_tab_widths(&self) -> Vec<u16> {
        self.titles
            .iter()
            .map(|title| {
                self.padding_left.width() as u16
                    + title.width() as u16
                    + self.padding_right.width() as u16
            })
            .collect()
    }

    // Render tabs with standard mode (original behavior)
    fn render_tabs_normal(&self, tabs_area: Rect, buf: &mut Buffer) {
        if tabs_area.is_empty() {
            return;
        }

        let mut x = tabs_area.left();
        let titles_length = self.titles.len();
        for (i, title) in self.titles.iter().enumerate() {
            let last_title = titles_length - 1 == i;
            let remaining_width = tabs_area.right().saturating_sub(x);

            if remaining_width == 0 {
                break;
            }

            // Calculate the region for this tab
            let tab_start_x = x;

            // Left Padding
            let pos = buf.set_line(x, tabs_area.top(), &self.padding_left, remaining_width);
            x = pos.0;
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 {
                break;
            }

            // Title
            let pos = buf.set_line(x, tabs_area.top(), title, remaining_width);
            x = pos.0;
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 {
                break;
            }

            // Right Padding
            let pos = buf.set_line(x, tabs_area.top(), &self.padding_right, remaining_width);
            let padding_end_x = pos.0;
            x = pos.0;

            // Set style for the entire tab area
            let tab_style = if Some(i) == self.selected {
                self.highlight_style
            } else {
                self.style
            };

            // Apply style to each cell in the tab (padding + title + padding)
            for cell_x in tab_start_x..padding_end_x {
                if let Some(cell) = buf.cell_mut(Position::new(cell_x, tabs_area.top())) {
                    cell.set_style(tab_style);
                }
            }

            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 || last_title {
                break;
            }

            // Divider
            let pos = buf.set_span(x, tabs_area.top(), &self.divider, remaining_width);
            x = pos.0;
        }
    }

    // Render tabs with scroll mode
    fn render_tabs_scroll(&self, tabs_area: Rect, buf: &mut Buffer) {
        if tabs_area.is_empty() || self.titles.is_empty() {
            return;
        }

        // Default to first tab if none selected
        let selected = self.selected.unwrap_or(0).min(self.titles.len() - 1);

        // Calculate tab widths including padding
        let tab_widths = self.calculate_tab_widths();
        let divider_width = self.divider.width() as u16;

        // Calculate total width needed for all tabs
        let total_tabs_width: u16 = tab_widths.iter().sum::<u16>()
            + (self.titles.len().saturating_sub(1) as u16 * divider_width);

        // If all tabs fit, just render normally
        if total_tabs_width <= tabs_area.width {
            self.render_tabs_normal(tabs_area, buf);
            return;
        }

        // Start by showing as many tabs from the left as possible
        let mut visible_range = (0, 0);
        let mut visible_width = tab_widths[0];

        // Expand to the right as much as possible
        let mut right_idx = 1;
        while right_idx < self.titles.len()
            && visible_width + divider_width + tab_widths[right_idx] <= tabs_area.width
        {
            visible_width += divider_width + tab_widths[right_idx];
            visible_range.1 = right_idx;
            right_idx += 1;
        }

        // If selected tab is not in visible range, adjust the range
        if selected > visible_range.1 {
            // Need to shift right to show selected tab
            // Start with just the selected tab and expand left and right
            visible_range = (selected, selected);
            visible_width = tab_widths[selected];

            // Add tabs to the left of selected (prioritize showing tabs to the left)
            let mut left_idx = selected.saturating_sub(1);
            while left_idx < selected && // Handle potential underflow
              visible_width + divider_width + tab_widths[left_idx] <= tabs_area.width
            {
                visible_width += divider_width + tab_widths[left_idx];
                visible_range.0 = left_idx;
                if left_idx == 0 {
                    break;
                }
                left_idx -= 1;
            }

            // Add tabs to the right of selected with remaining space
            let mut right_idx = selected + 1;
            while right_idx < self.titles.len()
                && visible_width + divider_width + tab_widths[right_idx] <= tabs_area.width
            {
                visible_width += divider_width + tab_widths[right_idx];
                visible_range.1 = right_idx;
                right_idx += 1;
            }
        } else if selected < visible_range.0 {
            // Need to shift left to show selected tab
            visible_range = (selected, selected);
            visible_width = tab_widths[selected];

            // Add tabs to the right of selected (prioritize showing tabs to the right)
            let mut right_idx = selected + 1;
            while right_idx < self.titles.len()
                && visible_width + divider_width + tab_widths[right_idx] <= tabs_area.width
            {
                visible_width += divider_width + tab_widths[right_idx];
                visible_range.1 = right_idx;
                right_idx += 1;
            }

            // Add tabs to the left of selected with remaining space
            let mut left_idx = selected.saturating_sub(1);
            while left_idx < selected && // Handle potential underflow
              visible_width + divider_width + tab_widths[left_idx] <= tabs_area.width
            {
                visible_width += divider_width + tab_widths[left_idx];
                visible_range.0 = left_idx;
                if left_idx == 0 {
                    break;
                }
                left_idx -= 1;
            }
        }

        // Need indicators?
        let need_left_indicator = visible_range.0 > 0;
        let need_right_indicator = visible_range.1 < self.titles.len() - 1;

        // Adjust visible range if indicators are needed
        if need_left_indicator {
            let indicator_width = self.scroll_left_indicator.width() as u16;
            if visible_width + indicator_width > tabs_area.width {
                // Remove tabs from the right to make room for left indicator
                while visible_range.0 < visible_range.1
                    && visible_width + indicator_width > tabs_area.width
                {
                    visible_width -= divider_width + tab_widths[visible_range.1];
                    visible_range.1 -= 1;
                }
            }
            visible_width += indicator_width;
        }

        if need_right_indicator {
            let indicator_width = self.scroll_right_indicator.width() as u16;
            if visible_width + indicator_width > tabs_area.width {
                // Remove tabs from the left to make room for right indicator
                while visible_range.0 < visible_range.1
                    && visible_width + indicator_width > tabs_area.width
                {
                    visible_width -= divider_width + tab_widths[visible_range.0];
                    visible_range.0 += 1;
                }
            }
        }

        // Render visible tabs
        let mut x = tabs_area.left();

        // Render left indicator if needed
        if need_left_indicator {
            let pos = buf.set_span(
                x,
                tabs_area.top(),
                &self.scroll_left_indicator,
                tabs_area.width,
            );
            x = pos.0;
        }

        // Render tabs in the visible range
        for i in visible_range.0..=visible_range.1 {
            let last_title = i == visible_range.1;
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 {
                break;
            }

            // Calculate the region for this tab
            let tab_start_x = x;

            // Left Padding
            let pos = buf.set_line(x, tabs_area.top(), &self.padding_left, remaining_width);
            x = pos.0;

            // Title
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 {
                break;
            }

            let pos = buf.set_line(x, tabs_area.top(), &self.titles[i], remaining_width);
            x = pos.0;

            // Right Padding
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width == 0 {
                break;
            }

            let pos = buf.set_line(x, tabs_area.top(), &self.padding_right, remaining_width);
            let padding_end_x = pos.0;
            x = pos.0;

            // Set style for the entire tab area
            let tab_style = if Some(i) == self.selected {
                self.highlight_style
            } else {
                self.style
            };

            // Apply style to each cell in the tab (padding + title + padding)
            for cell_x in tab_start_x..padding_end_x {
                if let Some(cell) = buf.cell_mut(Position::new(cell_x, tabs_area.top())) {
                    cell.set_style(tab_style);
                }
            }

            // Divider (if not last tab)
            if !last_title {
                let remaining_width = tabs_area.right().saturating_sub(x);
                if remaining_width == 0 {
                    break;
                }

                let pos = buf.set_span(x, tabs_area.top(), &self.divider, remaining_width);
                x = pos.0;
            }
        }

        // Render right indicator if needed
        if need_right_indicator {
            let remaining_width = tabs_area.right().saturating_sub(x);
            if remaining_width > 0 {
                buf.set_span(
                    x,
                    tabs_area.top(),
                    &self.scroll_right_indicator,
                    remaining_width,
                );
            }
        }
    }

    // Render tabs with wrap mode
    fn render_tabs_wrap(&self, tabs_area: Rect, buf: &mut Buffer) {
        if tabs_area.is_empty() || self.titles.is_empty() || tabs_area.height == 0 {
            return;
        }

        // Calculate tab widths
        let tab_widths = self.calculate_tab_widths();
        let divider_width = self.divider.width() as u16;

        // Group tabs into lines
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current_line = Vec::new();
        let mut current_width = 0;

        for (i, &width) in tab_widths.iter().enumerate() {
            let width_with_divider = if current_line.is_empty() {
                width
            } else {
                width + divider_width
            };

            if current_width + width_with_divider <= tabs_area.width {
                // Tab fits on current line
                current_line.push(i);
                current_width += width_with_divider;
            } else {
                // Tab doesn't fit, start a new line
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = Vec::new();
                }

                // Try to fit tab on a new line
                if width <= tabs_area.width {
                    current_line.push(i);
                    current_width = width;
                } else {
                    // Tab is too wide even for an empty line (will be truncated)
                    current_line.push(i);
                    current_width = tabs_area.width;
                }
            }
        }

        // Add the last line if not empty
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Render tabs line by line
        let mut y = tabs_area.top();
        for line_tabs in lines {
            if y >= tabs_area.bottom() {
                break; // No more vertical space
            }

            let mut x = tabs_area.left();
            for (i, &tab_idx) in line_tabs.iter().enumerate() {
                let last_in_line = i == line_tabs.len() - 1;
                let remaining_width = tabs_area.right().saturating_sub(x);
                if remaining_width == 0 {
                    break;
                }

                // Calculate the region for this tab
                let tab_start_x = x;

                // Left Padding
                let pos = buf.set_line(x, y, &self.padding_left, remaining_width);
                x = pos.0;

                // Title
                let remaining_width = tabs_area.right().saturating_sub(x);
                if remaining_width == 0 {
                    break;
                }

                let pos = buf.set_line(x, y, &self.titles[tab_idx], remaining_width);
                x = pos.0;

                // Right Padding
                let remaining_width = tabs_area.right().saturating_sub(x);
                if remaining_width == 0 {
                    break;
                }

                let pos = buf.set_line(x, y, &self.padding_right, remaining_width);
                let padding_end_x = pos.0;
                x = pos.0;

                // Set style for the entire tab area
                let tab_style = if Some(tab_idx) == self.selected {
                    self.highlight_style
                } else {
                    self.style
                };

                // Apply style to each cell in the tab (padding + title + padding)
                for cell_x in tab_start_x..padding_end_x {
                    if let Some(cell) = buf.cell_mut(Position::new(cell_x, y)) {
                        cell.set_style(tab_style);
                    }
                }

                // Divider (if not last tab in line)
                if !last_in_line {
                    let remaining_width = tabs_area.right().saturating_sub(x);
                    if remaining_width == 0 {
                        break;
                    }

                    let pos = buf.set_span(x, y, &self.divider, remaining_width);
                    x = pos.0;
                }
            }

            y += 1; // Move to next line
        }
    }
}

impl Styled for TabsWidget<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Widget for TabsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &TabsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Don't set style for the entire area - let each tab control its own style
        // This is the key fix - removing buf.set_style(area, self.style);

        match self.overflow_mode {
            OverflowMode::None => self.render_tabs_normal(area, buf),
            OverflowMode::Scroll => self.render_tabs_scroll(area, buf),
            OverflowMode::Wrap => self.render_tabs_wrap(area, buf),
        }
    }
}

impl<'a, Item> FromIterator<Item> for TabsWidget<'a>
where
    Item: Into<Line<'a>>,
{
    fn from_iter<Iter: IntoIterator<Item = Item>>(iter: Iter) -> Self {
        Self::new(iter)
    }
}

// Implement PanelWidget trait for TabsWidget
impl TuiWidget for TabsWidget<'_> {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        // Call the reference implementation
        Widget::render(self as &Self, area, buf);
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        match key.code {
            KeyCode::Left => {
                self.prev_tab();
                true
            }
            KeyCode::Right => {
                self.next_tab();
                true
            }
            KeyCode::Home => {
                if !self.titles.is_empty() {
                    self.set_selected(Some(0));
                }
                true
            }
            KeyCode::End => {
                if !self.titles.is_empty() {
                    self.set_selected(Some(self.titles.len() - 1));
                }
                true
            }
            KeyCode::Char(c) => {
                // Quick numeric selection (1-9) with Ctrl modifier
                if key.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_digit() {
                    if let Some(digit) = c.to_digit(10) {
                        let idx = (digit as usize).saturating_sub(1); // 1 maps to index 0
                        if idx < self.titles.len() {
                            self.set_selected(Some(idx));
                            return true;
                        }
                    }
                }
                false
            }
            KeyCode::Tab => {
                // Continuing the PanelWidget implementation for TabsWidget
                self.next_tab();
                true
            }
            KeyCode::BackTab => {
                self.prev_tab();
                true
            }
            _ => false,
        }
    }

    fn focus(&mut self) {
        self.is_focused = true;
        // No visual changes needed here as the tabs already have highlight styling
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
        // No visual changes needed here as the tabs already have highlight styling
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
