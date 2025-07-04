// tokio-tui/src/widgets/status/status_cells/file_size_status.rs
use std::{
    any::Any,
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    widgets::{Paragraph, Widget as _},
};

use crate::{CellRef, StatusCell, StatusCellUpdate, ToStatusCell};

pub struct FileSizeStatus {
    pub current: u64,
    pub total: u64,
    needs_redraw: bool,
    last_text: String,
    last_update: Instant,
}

impl Default for FileSizeStatus {
    fn default() -> Self {
        Self {
            current: Default::default(),
            total: Default::default(),
            needs_redraw: Default::default(),
            last_text: Default::default(),
            last_update: Instant::now(),
        }
    }
}

const FILE_SIZE_UPDATE_INTERVAL: Duration = Duration::from_millis(500); // 2 FPS

impl StatusCell for FileSizeStatus {
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
        if self.last_update.elapsed() < FILE_SIZE_UPDATE_INTERVAL {
            return;
        }

        let new_text = format!("{}/{} MB", self.current / 1_000_000, self.total / 1_000_000);
        if self.last_text != new_text {
            self.last_text = new_text;
            self.needs_redraw = true;
        }

        self.last_update = Instant::now();
    }
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.last_text.clone()).render(area, buf);
        self.needs_redraw = false;
    }
    fn constraint(&self) -> Constraint {
        Constraint::Fill(1)
    }
    fn needs_draw(&self) -> bool {
        self.needs_redraw
    }
}

impl CellRef<FileSizeStatus> {
    pub fn set_size(&self, current: u64, total: u64) -> StatusCellUpdate {
        self.update_with(move |file_size_status| {
            if file_size_status.current != current || file_size_status.total != total {
                file_size_status.current = current;
                file_size_status.total = total;
                file_size_status.needs_redraw = true;
            }
        })
    }
}

impl FileSizeStatus {
    pub fn new<T: Into<Self>>(args: T) -> Self {
        <Self as StatusCell>::new(args)
    }
}

impl From<u64> for FileSizeStatus {
    fn from(total: u64) -> Self {
        FileSizeStatus {
            current: 0,
            total,
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<(u64, u64)> for FileSizeStatus {
    fn from((current, total): (u64, u64)) -> Self {
        FileSizeStatus {
            current,
            total,
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<()> for FileSizeStatus {
    fn from(_: ()) -> Self {
        Self {
            current: 0,
            total: 0,
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl ToStatusCell for FileSizeStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
