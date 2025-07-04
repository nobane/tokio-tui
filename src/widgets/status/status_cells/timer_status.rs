// tokio-tui/src/widgets/status/status_cells/timer_status.rs
use std::any::Any;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Paragraph, Widget as _};

use crate::{CellRef, StatusCell, StatusCellUpdate,  ToStatusCell};

/// Update frequency – 1 FPS is good enough for a text timer.
const TIMER_UPDATE_INTERVAL: Duration = Duration::from_millis(1_000);

/// The timer can either **count up** from a starting instant or **count down** to a target instant.
#[derive(Debug, Clone, Copy)]
pub enum TimerMode {
    /// `start_time` → the instant from which we are counting *up*.
    CountUp { start_time: Instant },
    /// `end_time` → the instant at which the countdown *ends*.
    CountDown { end_time: Instant },
}

impl TimerMode {
    /// Return the *duration* to display (elapsed or remaining) given `now`.
    fn duration(&self, now: Instant) -> Duration {
        match *self {
            TimerMode::CountUp { start_time } => now.saturating_duration_since(start_time),
            TimerMode::CountDown { end_time } => end_time.saturating_duration_since(now),
        }
    }

    /// Reset the mode to start *now* (keeps the same mode).
    fn reset(&mut self) {
        let now = Instant::now();
        *self = match *self {
            TimerMode::CountUp { .. } => TimerMode::CountUp { start_time: now },
            TimerMode::CountDown { .. } => TimerMode::CountDown { end_time: now },
        };
    }
}

pub struct TimerStatus {
    mode: TimerMode,
    /// Has the textual representation changed since the previous draw?
    needs_redraw: bool,
    /// Cached formatted string – avoids allocating every draw.
    last_text: String,
    /// Last time `preprocess` updated the value; governs the update rate.
    last_update: Instant,
}

impl StatusCell for TimerStatus {
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
        // Limit updates to the configured interval.
        if self.last_update.elapsed() < TIMER_UPDATE_INTERVAL {
            return;
        }

        let now = Instant::now();
        let duration = self.mode.duration(now);
        let new_text = format!(
            "{:02}:{:02}:{:02}",
            duration.as_secs() / 3600,
            (duration.as_secs() % 3600) / 60,
            duration.as_secs() % 60
        );

        if self.last_text != new_text {
            self.last_text = new_text;
            self.needs_redraw = true;
        }

        self.last_update = now;
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

// === Convenience helpers ===
impl TimerStatus {
    /// Create a new *count‑up* timer starting **now**.
    pub fn new_count_up() -> Self {
        Self::from(())
    }

    /// Create a new *count‑down* timer that ends at `end_time`.
    pub fn new_count_down(end_time: Instant) -> Self {
        TimerStatus {
            mode: TimerMode::CountDown { end_time },
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }

    /// Convenience constructor that takes the remaining `duration` and calculates `end_time`.
    pub fn new_count_down_from(duration: Duration) -> Self {
        Self::new_count_down(Instant::now() + duration)
    }
}

// === `CellRef` helpers to mutate an existing timer ===
impl CellRef<TimerStatus> {
    /// Reset the timer: *count‑up* restarts from zero, *count‑down* starts a new countdown of the
    /// same duration (“reset to full”).
    pub fn reset(&self) -> StatusCellUpdate {
        self.update_with(|timer| {
            timer.mode.reset();
            timer.needs_redraw = true;
        })
    }
}

// === Default & `From` impls ===
impl Default for TimerStatus {
    fn default() -> Self {
        TimerStatus {
            mode: TimerMode::CountUp {
                start_time: Instant::now(),
            },
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl From<()> for TimerStatus {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

/// *Count‑up* starting at the given `start_time`.
impl From<Instant> for TimerStatus {
    fn from(start_time: Instant) -> Self {
        TimerStatus {
            mode: TimerMode::CountUp { start_time },
            needs_redraw: true,
            last_text: String::new(),
            last_update: Instant::now(),
        }
    }
}

impl ToStatusCell for TimerStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
