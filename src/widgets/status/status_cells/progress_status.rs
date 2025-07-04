// tokio-tui/src/widgets/status/status_cells/progress_status.rs
use std::{
    any::Any,
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    widgets::{Paragraph, Widget as _},
};

use crate::{CellRef, StatusCell, StatusCellUpdate,  ToStatusCell};

use super::ETAStatus;

pub struct ProgressStatus {
    pub current: u64,
    pub total: u64,
    pub percent: f64,
    pub start_time: Instant,
    pub show_eta: bool,
    needs_redraw: bool,
    last_percent: f64,
    last_eta_text: String,
    last_update: Instant,
}

const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_millis(100); // 10 FPS for smooth progress

impl StatusCell for ProgressStatus {
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
        if self.last_update.elapsed() < PROGRESS_UPDATE_INTERVAL {
            return;
        }

        // Check if progress changed enough to warrant redraw
        if (self.last_percent - self.percent).abs() > 0.001 {
            self.last_percent = self.percent;
            self.needs_redraw = true;
        }

        // Check if ETA changed (only update ETA once per second)
        if self.show_eta && self.last_update.elapsed() >= Duration::from_secs(1) {
            let new_eta_text =
                if let Some(eta) = ETAStatus::calculate_eta(self.start_time, self.percent) {
                    format!(" ETA: {}", ETAStatus::format_duration(eta))
                } else {
                    " ETA: --:--:--".to_string()
                };

            if self.last_eta_text != new_eta_text {
                self.last_eta_text = new_eta_text;
                self.needs_redraw = true;
            }
        }

        self.last_update = Instant::now();
    }
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer) {
        if self.show_eta {
            let layouts = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(10), Constraint::Length(14)])
                .split(area);

            self.render_progress_bar(layouts[0], buf);
            self.render_eta(layouts[1], buf);
        } else {
            self.render_progress_bar(area, buf);
        }
        self.needs_redraw = false;
    }
    fn constraint(&self) -> Constraint {
        Constraint::Fill(1)
    }
    fn needs_draw(&self) -> bool {
        self.needs_redraw
    }
}

impl CellRef<ProgressStatus> {
    pub fn set_progress(&self, current: u64, total: u64) -> StatusCellUpdate {
        self.update_with(move |progress_status| {
            if progress_status.current != current || progress_status.total != total {
                progress_status.current = current;
                progress_status.total = total;
                progress_status.percent = ProgressStatus::calc_percent(current, total);
                progress_status.needs_redraw = true;
            }
        })
    }
}

impl ProgressStatus {
    pub fn new<T: Into<Self>>(args: T) -> Self {
        <Self as StatusCell>::new(args)
    }
}

const PROGRESS_BAR_SHOW_ETA_DEFAULT: bool = true;

impl Default for ProgressStatus {
    fn default() -> Self {
        Self {
            current: 0,
            total: 100,
            percent: 0.0,
            start_time: Instant::now(),
            show_eta: PROGRESS_BAR_SHOW_ETA_DEFAULT,
            needs_redraw: true,
            last_percent: -1.0,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl ProgressStatus {
    fn render_progress_bar(&self, area: Rect, buf: &mut Buffer) {
        let filled_width = (area.width as f64 * self.percent) as u16;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.left() + filled_width {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    cell.set_symbol("█");
                }
            }
            for x in area.left() + filled_width..area.right() {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    cell.set_symbol("░");
                }
            }
        }
    }

    fn render_eta(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.last_eta_text.clone()).render(area, buf);
    }

    pub fn calc_percent(current: u64, total: u64) -> f64 {
        (current as f64 / total as f64).min(1.0)
    }

    pub fn with_eta(mut self, show_eta: bool) -> Self {
        self.show_eta = show_eta;
        self
    }
}

impl From<u64> for ProgressStatus {
    fn from(total: u64) -> Self {
        ProgressStatus {
            current: 0,
            total,
            percent: 0.0,
            start_time: Instant::now(),
            show_eta: true,
            needs_redraw: true,
            last_percent: -1.0,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<(u64, bool)> for ProgressStatus {
    fn from((total, show_eta): (u64, bool)) -> Self {
        ProgressStatus {
            current: 0,
            total,
            percent: 0.0,
            start_time: Instant::now(),
            show_eta,
            needs_redraw: true,
            last_percent: -1.0,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<(u64, u64)> for ProgressStatus {
    fn from((current, total): (u64, u64)) -> Self {
        ProgressStatus {
            current,
            total,
            percent: ProgressStatus::calc_percent(current, total),
            start_time: Instant::now(),
            show_eta: true,
            needs_redraw: true,
            last_percent: -1.0,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<(u64, u64, bool)> for ProgressStatus {
    fn from((current, total, show_eta): (u64, u64, bool)) -> Self {
        ProgressStatus {
            current,
            total,
            percent: ProgressStatus::calc_percent(current, total),
            start_time: Instant::now(),
            show_eta,
            needs_redraw: true,
            last_percent: -1.0,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<()> for ProgressStatus {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl ToStatusCell for ProgressStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
