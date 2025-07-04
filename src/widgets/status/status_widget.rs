// tokio-tui/src/widgets/status/status_widget.rs
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};

use crate::{IntoStatusUpdates, LineBuilder, TuiWidget};

use super::{StatusCell, StatusCellUpdate, StatusLineId, StatusUpdate};

pub struct BoxedCell {
    pub index: usize,
    pub cell: Box<dyn StatusCell>,
}

#[derive(Default)]
pub struct CellVisibility(pub HashMap<(StatusLineId, String), bool>);

impl CellVisibility {
    pub fn set_visibility(&mut self, line_id: StatusLineId, cell_id: usize, visible: bool) {
        self.0.insert((line_id, cell_id.to_string()), visible);
    }

    pub fn set_visibility_by_index(&mut self, line_id: StatusLineId, index: usize, visible: bool) {
        self.0.insert((line_id, index.to_string()), visible);
    }

    pub fn is_visible(&self, line_id: StatusLineId, cell_id: usize) -> bool {
        *self.0.get(&(line_id, cell_id.to_string())).unwrap_or(&true)
    }

    pub fn is_visible_by_index(&self, line_id: StatusLineId, index: usize) -> bool {
        *self.0.get(&(line_id, index.to_string())).unwrap_or(&true)
    }

    pub fn toggle_cell_visibility(&mut self, line_id: StatusLineId, cell_id: usize) {
        let current_visibility = self.is_visible(line_id, cell_id);
        self.set_visibility(line_id, cell_id, !current_visibility);
    }

    pub fn toggle_cell_visibility_by_index(&mut self, line_id: StatusLineId, index: usize) {
        let current_visibility = self.is_visible_by_index(line_id, index);
        self.set_visibility_by_index(line_id, index, !current_visibility);
    }
}

pub struct StatusLineHandle {
    cells: Vec<BoxedCell>,
    line_id: StatusLineId,
}

#[derive(Clone)]
pub struct LineCounter(Arc<AtomicU64>);

impl Default for LineCounter {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(9)))
    }
}

impl LineCounter {
    pub fn next(&self) -> StatusLineId {
        StatusLineId(self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}

pub struct StatusWidget {
    line_handles: HashMap<StatusLineId, StatusLineHandle>,
    pub line_counter: LineCounter,
    last_update: Instant,
    line_visibility: HashMap<StatusLineId, bool>,
    render_order: Vec<StatusLineId>,
    cell_visibility: CellVisibility,
    margin: Margin,
}

impl StatusWidget {
    pub fn new() -> Self {
        StatusWidget {
            line_handles: HashMap::new(),
            line_counter: LineCounter::default(),
            last_update: Instant::now(),
            line_visibility: HashMap::new(),
            render_order: Vec::new(),
            cell_visibility: CellVisibility::default(),
            margin: Margin::new(1, 0),
        }
    }

    pub fn new_builder(&mut self) -> LineBuilder {
        LineBuilder::new(self)
    }

    pub fn next_line_id(&mut self) -> StatusLineId {
        self.line_counter.next()
    }

    pub fn add_line<F>(&mut self, line_id: StatusLineId, create_cells: F) -> StatusLineId
    where
        F: FnOnce() -> Vec<BoxedCell>,
    {
        let cells = create_cells();

        // Set visibility for all cells
        for boxed in &cells {
            self.cell_visibility
                .set_visibility(line_id, boxed.index, true);
        }

        // Initialize line as invisible
        self.line_visibility.insert(line_id, false);

        // Store the line handle
        self.line_handles
            .insert(line_id, StatusLineHandle { cells, line_id });

        line_id
    }

    pub fn apply_update<'a>(
        &'a mut self,
        handle: &'a mut StatusLineHandle,
        cell_update: StatusCellUpdate,
    ) -> &'a mut StatusLineHandle {
        // Apply the update function to the cell
        if cell_update.cell_id < handle.cells.len() {
            let cell = &mut handle.cells[cell_update.cell_id].cell;
            (cell_update.update_fn)(cell.as_any_mut());
        }

        handle
    }

    pub fn process_updates(&mut self, updates: impl IntoStatusUpdates) {
        for update in updates.into_status_updates() {
            match update {
                StatusUpdate::CellUpdate(update_info) => {
                    self.process_cell_update(update_info);
                }
                StatusUpdate::LineVisibility { line_id, visible } => {
                    self.set_line_visibility(line_id, visible);
                }
            }
        }
    }

    pub fn process_cell_update(&mut self, cell_update: StatusCellUpdate) {
        let id = cell_update.line_id;
        if let Some(mut handle) = self.line_handles.remove(&cell_update.line_id) {
            self.apply_update(&mut handle, cell_update);
            self.line_handles.insert(id, handle);
        }
    }

    pub fn insert_line(&mut self, line_id: StatusLineId, cells: Vec<BoxedCell>, visible: bool) {
        let line_handle = StatusLineHandle { cells, line_id };

        // Set visibility for all cells
        for (i, boxed) in line_handle.cells.iter().enumerate() {
            self.cell_visibility
                .set_visibility(line_id, boxed.index, true);
            self.cell_visibility
                .set_visibility_by_index(line_id, i, true);
        }

        self.line_visibility.insert(line_id, visible);

        self.line_handles.insert(line_id, line_handle);

        if visible {
            self.render_order.push(line_id);
        }
    }

    pub fn set_cell_visibility(&mut self, line_id: StatusLineId, id: usize, visible: bool) {
        self.cell_visibility.set_visibility(line_id, id, visible)
    }

    pub fn set_cell_visibility_by_index(
        &mut self,
        line_id: StatusLineId,
        index: usize,
        visible: bool,
    ) {
        self.cell_visibility
            .set_visibility_by_index(line_id, index, visible)
    }

    pub fn is_cell_visible(&self, line_id: StatusLineId, cell_id: usize) -> bool {
        self.cell_visibility.is_visible(line_id, cell_id)
    }

    pub fn is_cell_visible_by_index(&self, line_id: StatusLineId, index: usize) -> bool {
        self.cell_visibility.is_visible_by_index(line_id, index)
    }

    pub fn set_line_visibility(&mut self, line_id: StatusLineId, visible: bool) {
        self.line_visibility.insert(line_id, visible);

        self.render_order.retain(|i| *i != line_id);
        if visible {
            self.render_order.push(line_id)
        }
    }
}

impl Default for StatusWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiWidget for StatusWidget {
    fn need_draw(&self) -> bool {
        // Check if any visible line has cells that need drawing
        for line_id in &self.render_order {
            if let Some(line_handle) = self.line_handles.get(line_id) {
                for (i, boxed) in line_handle.cells.iter().enumerate() {
                    if (self.cell_visibility.is_visible(*line_id, boxed.index)
                        || self.cell_visibility.is_visible_by_index(*line_id, i))
                        && boxed.cell.needs_draw()
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn preprocess(&mut self) {
        let now = Instant::now();
        self.last_update = now;

        // Preprocess all visible cells
        for line_id in &self.render_order {
            if let Some(line_handle) = self.line_handles.get_mut(line_id) {
                for (i, boxed) in line_handle.cells.iter_mut().enumerate() {
                    if self.cell_visibility.is_visible(*line_id, boxed.index)
                        || self.cell_visibility.is_visible_by_index(*line_id, i)
                    {
                        boxed.cell.preprocess();
                    }
                }
            }
        }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let now = Instant::now();
        self.last_update = now;

        let area = area.inner(self.margin);

        let row_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1); self.render_order.len()])
            .split(area);

        for (row_id, row_area) in self.render_order.iter().zip(row_layout.iter()) {
            if let Some(row) = self.line_handles.get_mut(row_id) {
                let constraints: Vec<_> = row
                    .cells
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        if self.cell_visibility.is_visible(row.line_id, c.index)
                            || self.cell_visibility.is_visible_by_index(row.line_id, i)
                        {
                            Some(c.cell.constraint())
                        } else {
                            None
                        }
                    })
                    .collect();

                let col_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(constraints)
                    .split(*row_area);

                for (i, (boxed, layout)) in row.cells.iter_mut().zip(col_layout.iter()).enumerate()
                {
                    if self.cell_visibility.is_visible(row.line_id, boxed.index)
                        || self.cell_visibility.is_visible_by_index(row.line_id, i)
                    {
                        boxed.cell.draw_cell(*layout, buf);
                    }
                }
            }
        }
    }

    fn key_event(&mut self, _key: ratatui::crossterm::event::KeyEvent) -> bool {
        false
    }

    fn focus(&mut self) {}

    fn unfocus(&mut self) {}

    fn is_focused(&self) -> bool {
        false
    }
}
