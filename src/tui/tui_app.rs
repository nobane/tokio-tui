// tokio-tui/src/tui/tui_app.rs
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::{
    io::stdout,
    time::{Duration, Instant},
};

use crate::tui::input_backend::InputHandler;

pub trait TuiApp {
    fn render(&mut self, frame: &mut TerminalFrame);
    #[allow(unused)]
    fn handle_mouse_events(&mut self, mouse_events: Vec<MouseEvent>) {}
    fn handle_key_events(&mut self, keys_events: Vec<KeyEvent>);
    fn before_frame(&mut self, #[allow(unused)] terminal: &TerminalBackend) {}
    fn after_frame(&mut self, #[allow(unused)] terminal: &TerminalBackend) {}
    fn should_quit(&self) -> bool;
    fn should_draw(&mut self) -> bool {
        true
    }
    fn quit_requested(&mut self) {}
}
pub use ratatui::{buffer::Buffer, layout::Rect};

// Widget trait that all renderable components must implement
pub trait TuiWidget: Send + Sync {
    fn preprocess(&mut self) {}
    fn draw(&mut self, area: Rect, buf: &mut Buffer);
    fn key_event(&mut self, event: KeyEvent) -> bool; // Return true if handled
    #[allow(unused)]
    fn mouse_event(&mut self, event: MouseEvent) -> bool {
        false
    }
    fn focus(&mut self);
    fn unfocus(&mut self);
    fn is_focused(&self) -> bool;
    fn need_draw(&self) -> bool {
        true
    }
    fn need_visibility(&self) -> Option<bool> {
        None
    }
}

pub type TerminalBackend = ratatui::DefaultTerminal;
pub type TerminalFrame<'a> = ratatui::Frame<'a>;

const DEFAULT_FRAME_TIME: Duration = Duration::from_millis(100);
pub struct Tui {
    key_handler: Option<InputHandler>,
    frame_sync: bool,
    frame_length: Duration,
}

impl Tui {
    pub fn new() -> Result<Self> {
        Ok(Tui {
            key_handler: Some(InputHandler::new()),
            frame_sync: true,
            frame_length: DEFAULT_FRAME_TIME,
        })
    }

    pub fn without_key_capture(mut self) -> Self {
        self.key_handler = None;
        self
    }

    pub fn without_frame_sync(mut self) -> Self {
        self.frame_sync = false;
        self
    }

    pub fn with_frame_length(mut self, frame_time: Duration) -> Self {
        self.frame_length = frame_time;
        self
    }

    pub fn run<A: TuiApp>(mut self, mut app: A) -> Result<A> {
        // Set up the terminal
        enable_raw_mode()?;
        execute!(
            stdout(),
            EnterAlternateScreen,
            EnableMouseCapture // Enable mouse events
        )?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        // Start the key handler if we have one
        if let Some(handler) = &mut self.key_handler {
            handler.start()?;
        }
        let mut last_width = 0u16;
        let mut last_height = 0u16;
        // Main event loop
        loop {
            let frame_start = Instant::now();

            // Check if we should quit
            if app.should_quit() {
                break;
            }

            // Pre-frame processing
            app.before_frame(&terminal);

            // Process key events from handler if any
            if let Some(handler) = &mut self.key_handler {
                // Poll for new keys if needed (non-threaded handlers)

                // Process any available keys
                if let Some((key_events, mouse_events)) = handler.flush_events() {
                    if let Some(events) = key_events {
                        app.handle_key_events(events);
                    }
                    if let Some(events) = mouse_events {
                        app.handle_mouse_events(events);
                    }
                }
            }
            let frame_size = terminal
                .size()
                .unwrap_or_else(|_| ratatui::layout::Size::new(last_width, last_height));
            let frame_changed = last_width != frame_size.width || last_height != frame_size.height;

            if app.should_draw() || frame_changed {
                last_width = frame_size.width;
                last_height = frame_size.height;

                // Render the UI
                terminal.draw(|frame| app.render(frame))?;
            }

            // Post-frame processing
            app.after_frame(&terminal);

            if self.frame_sync {
                // If we processed the frame too quickly, sleep for the remainder of the frame time
                let frame_elapsed = frame_start.elapsed();
                if frame_elapsed < self.frame_length {
                    std::thread::sleep(self.frame_length - frame_elapsed);
                }
            }
        }

        // Stop the key handler if we have one
        if let Some(handler) = &mut self.key_handler {
            handler.stop();
        }

        // Clean up the terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture // Disable mouse capture when done
        )?;

        Ok(app)
    }
}
