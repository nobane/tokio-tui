// tokio-tui/src/widgets/status/status_cells/eta_status.rs
use std::{
    any::Any,
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    widgets::{Paragraph, Widget as _},
};

use crate::{CellRef, StatusCell, StatusCellUpdate,  ToStatusCell};

use super::ProgressStatus;

pub struct ETAStatus {
    pub start_time: Instant,
    pub progress: f64,
    needs_redraw: bool,
    last_eta_text: String,
    last_update: Instant,
}

const ETA_UPDATE_INTERVAL: Duration = Duration::from_millis(1000); // 1 FPS

impl StatusCell for ETAStatus {
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
        if self.last_update.elapsed() < ETA_UPDATE_INTERVAL {
            return;
        }

        let new_text = if let Some(eta) = Self::calculate_eta(self.start_time, self.progress) {
            format!("ETA: {}", Self::format_duration(eta))
        } else {
            "ETA: --:--:--".to_string()
        };

        if self.last_eta_text != new_text {
            self.last_eta_text = new_text;
            self.needs_redraw = true;
        }

        self.last_update = Instant::now();
    }
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.last_eta_text.clone()).render(area, buf);
        self.needs_redraw = false;
    }
    fn constraint(&self) -> Constraint {
        Constraint::Fill(1)
    }
    fn needs_draw(&self) -> bool {
        self.needs_redraw
    }
}

impl CellRef<ETAStatus> {
    pub fn update_progress(&self, current: u64, total: u64) -> StatusCellUpdate {
        self.update_with(move |eta_status| {
            let new_progress = ProgressStatus::calc_percent(current, total);
            if (eta_status.progress - new_progress).abs() > 0.01 {
                eta_status.progress = new_progress;
                eta_status.needs_redraw = true;
            }
        })
    }
}

impl ETAStatus {
    pub fn new<T: Into<Self>>(args: T) -> Self {
        <Self as StatusCell>::new(args)
    }

    pub fn calculate_eta(start_time: Instant, progress: f64) -> Option<Duration> {
        if progress > 0.0 {
            let elapsed = start_time.elapsed();
            let total_estimated = elapsed.as_secs_f64() / progress;
            let remaining = total_estimated - elapsed.as_secs_f64();
            Some(Duration::from_secs_f64(remaining))
        } else {
            None
        }
    }

    pub fn format_duration(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

impl Default for ETAStatus {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            progress: 0.0,
            needs_redraw: true,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<(Instant, f64)> for ETAStatus {
    fn from((start_time, progress): (Instant, f64)) -> Self {
        ETAStatus {
            start_time,
            progress,
            needs_redraw: true,
            last_eta_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<()> for ETAStatus {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl ToStatusCell for ETAStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
