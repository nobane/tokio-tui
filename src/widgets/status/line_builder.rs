// tokio-tui/src/widgets/status/line_builder.rs
use crate::{BoxedCell, CellRef, StatusCell, StatusLineId, StatusLineRef, StatusWidget};

/// Builder for creating status lines with strongly-typed cell references
pub struct LineBuilder {
    line_id: StatusLineId,
    cells: Vec<Box<dyn StatusCell>>,
    next_index: usize,
}

impl LineBuilder {
    pub fn new(manager: &mut StatusWidget) -> Self {
        let line_id = manager.next_line_id();
        Self {
            line_id,
            cells: Vec::new(),
            next_index: 0,
        }
    }

    /// Add a cell to the status line and get a typed reference to it
    pub fn add<C: StatusCell + 'static>(&mut self, cell: C) -> CellRef<C> {
        let index = self.next_index;
        self.cells.push(Box::new(cell));
        self.next_index += 1;
        CellRef::new(self.line_id, index)
    }

    /// Build the final status line and register with the manager
    pub fn build(self, manager: &mut StatusWidget) -> StatusLineRef {
        let cells: Vec<BoxedCell> = self
            .cells
            .into_iter()
            .enumerate()
            .map(|(i, cell)| BoxedCell {
                index: i, // Use index as the name
                cell,
            })
            .collect();

        manager.add_line(self.line_id, || cells);

        StatusLineRef(self.line_id)
    }
}

pub fn create_cells<I>(cells: I) -> Vec<BoxedCell>
where
    I: IntoIterator<Item = Box<dyn StatusCell>>,
{
    cells
        .into_iter()
        .enumerate()
        .map(|(index, cell)| BoxedCell { index, cell })
        .collect()
}
// src/widgets/status/line_builder.rs (add this macro)
#[macro_export]
macro_rules! cells {
    ($($cell:expr),* $(,)?) => {
        $crate::create_cells(vec![
            $(Box::new($cell) as Box<dyn $crate::StatusCell>),*
        ])
    };
}
