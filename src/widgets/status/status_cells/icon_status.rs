// tokio-tui/src/widgets/status/status_cells/icon_status.rs
use std::any::Any;
use std::time::{Duration, Instant};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Position, Rect},
    style::{Color, Style},
};

use crate::{CellRef, StatusCell, StatusCellUpdate, ToStatusCell};

pub struct IconStatus {
    pub mode: IconMode,
    pub state: f32,
    needs_redraw: bool,
    last_frame: usize,
    last_update: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum IconMode {
    Spinner,
    Download,
    Pulsate,
    Check,
    Cross,
    Pause,
    Wait,
    Exclamation,
    Question,
    Cancel,
    Alert,
}

const SPINNER_FRAMES: ([char; 10], f32) = (['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'], 2.5);
const PULSATE_FRAMES: ([char; 6], f32) = (['·', '∘', '●', '○', '●', '∘'], 2.0);
const DOWNLOAD_FRAMES: ([char; 8], f32) = (['█', '▇', '▆', '▅', '▄', '▃', '▂', '▁'], 3.0);

impl IconStatus {
    fn update_state(&mut self, delta: Duration, speed: &f32) {
        self.state += delta.as_secs_f32() * speed;
    }

    fn get_current_frame(&self) -> (char, usize) {
        match self.mode {
            IconMode::Spinner => {
                let (frames, _) = &SPINNER_FRAMES;
                let frame_idx = (self.state as usize) % frames.len();
                (frames[frame_idx], frame_idx)
            }
            IconMode::Pulsate => {
                let (frames, _) = &PULSATE_FRAMES;
                let frame_idx = (self.state as usize) % frames.len();
                (frames[frame_idx], frame_idx)
            }
            IconMode::Download => {
                let (frames, _) = &DOWNLOAD_FRAMES;
                let frame_idx = (self.state as usize) % frames.len();
                (frames[frame_idx], frame_idx)
            }
            IconMode::Check => ('✓', 0),
            IconMode::Cancel => ('🚫', 0),
            IconMode::Exclamation => ('!', 0),
            IconMode::Question => ('?', 0),
            IconMode::Cross => ('✗', 0),
            IconMode::Pause => ('⏸', 0),
            IconMode::Alert => ('⚠', 0),
            IconMode::Wait => ('⏳', 0),
        }
    }

    fn get_frame_duration(&self) -> Option<Duration> {
        match self.mode {
            IconMode::Spinner => Some(Duration::from_millis(500)), // 4 FPS
            IconMode::Pulsate => Some(Duration::from_millis(500)), // 2 FPS
            IconMode::Download => Some(Duration::from_millis(500)), // 3 FPS
            _ => None, // Static icons don't need updates
        }
    }
}

impl StatusCell for IconStatus {
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
        let frame_duration = match self.get_frame_duration() {
            Some(duration) => duration,
            None => return, // Static icon, no updates needed
        };

        if self.last_update.elapsed() < frame_duration {
            return;
        }

        let delta = self.last_update.elapsed();

        match self.mode {
            IconMode::Spinner => {
                let (_, rate) = &SPINNER_FRAMES;
                let old_frame = self.last_frame;
                self.update_state(delta, rate);
                let (_, new_frame) = self.get_current_frame();
                if old_frame != new_frame {
                    self.last_frame = new_frame;
                    self.needs_redraw = true;
                    self.last_update = Instant::now();
                }
            }
            IconMode::Pulsate => {
                let (_, rate) = &PULSATE_FRAMES;
                let old_frame = self.last_frame;
                self.update_state(delta, rate);
                let (_, new_frame) = self.get_current_frame();
                if old_frame != new_frame {
                    self.last_frame = new_frame;
                    self.needs_redraw = true;
                    self.last_update = Instant::now();
                }
            }
            IconMode::Download => {
                let (_, rate) = &DOWNLOAD_FRAMES;
                let old_frame = self.last_frame;
                self.update_state(delta, rate);
                let (_, new_frame) = self.get_current_frame();
                if old_frame != new_frame {
                    self.last_frame = new_frame;
                    self.needs_redraw = true;
                    self.last_update = Instant::now();
                }
            }
            _ => {
                // Static icons don't need updates
            }
        }
    }
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer) {
        let (icon, _) = self.get_current_frame();

        if let Some(line) = buf.cell_mut(Position::new(area.left(), area.y)) {
            line.set_char(icon);

            match self.mode {
                IconMode::Check => {
                    line.set_style(Style::default().fg(Color::Green));
                }
                IconMode::Cross => {
                    line.set_style(Style::default().fg(Color::Red));
                }
                IconMode::Question | IconMode::Alert => {
                    line.set_style(Style::default().fg(Color::Yellow));
                }
                IconMode::Download => {
                    let index = (self.state as usize) % 8;
                    let fg_color = Color::DarkGray;
                    let bg_color = Color::Cyan;
                    if index == 0 {
                        line.set_style(Style::default().fg(fg_color))
                    } else {
                        line.set_style(Style::default().fg(fg_color).bg(bg_color))
                    };
                }
                _ => {}
            };
        }

        self.needs_redraw = false;
    }
    fn constraint(&self) -> Constraint {
        Constraint::Length(2)
    }
    fn needs_draw(&self) -> bool {
        self.needs_redraw
    }
}

impl CellRef<IconStatus> {
    pub fn set(&self, mode: IconMode) -> StatusCellUpdate {
        self.update_with(move |icon_status| {
            if icon_status.mode != mode {
                icon_status.mode = mode;
                icon_status.state = 0.0;
                icon_status.last_frame = 0;
                icon_status.needs_redraw = true;
            }
        })
    }
}

impl IconStatus {
    pub fn new<T: Into<Self>>(args: T) -> Self {
        <Self as StatusCell>::new(args)
    }
}

impl Default for IconStatus {
    fn default() -> Self {
        Self {
            mode: IconMode::Spinner,
            state: 0.0,
            needs_redraw: true,
            last_frame: 0,
            last_update: Instant::now(),
        }
    }
}

impl From<IconMode> for IconStatus {
    fn from(mode: IconMode) -> Self {
        IconStatus {
            mode,
            state: 0.0,
            needs_redraw: true,
            last_frame: 0,
            last_update: Instant::now(),
        }
    }
}

impl From<()> for IconStatus {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl ToStatusCell for IconStatus {
    fn into_status_component(self) -> Box<dyn StatusCell> {
        Box::new(self)
    }
}
