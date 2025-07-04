// tokio-tui/src/widgets/tracer/console_widget.rs
use anyhow::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    widgets::Borders,
};
use tokio::sync::mpsc;
use tokio_tracer::Tracer;
use tokio_tui::{CommandSet, InputWidget, TuiWidget};
use tracing::error;

use super::TracerWidget;

// Command that can be sent to the console
#[derive(Debug, Clone)]
pub enum ConsoleCommand {
    Clear,
    Lines(Vec<String>),
}

/// A console widget that combines a tracer display with an input box
/// for entering commands.
pub struct ConsoleWidget {
    // UI components
    tracer_widget: TracerWidget,
    input_widget: InputWidget,

    // Command processing
    command_rx: mpsc::UnboundedReceiver<ConsoleCommand>,
    command_tx: mpsc::UnboundedSender<ConsoleCommand>,
    command_set: CommandSet,

    // UI state
    input_focused: bool,
    is_focused: bool,
}

impl ConsoleWidget {
    /// Create a new console widget
    pub fn new(tracer: Tracer, command_set: CommandSet) -> Result<Self> {
        // Create channel for commands
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Create tracer tabs
        let tracer_widget = TracerWidget::new(tracer)?.with_borders(Borders::BOTTOM);

        // Create input box
        let mut input_widget = InputWidget::new()
            .without_border()
            .with_prefix("] ")
            .with_prefix_style(Style::default().fg(Color::Green));

        // Default focus on the input box
        input_widget.focus();

        Ok(Self {
            tracer_widget,
            input_widget,
            command_rx,
            command_tx,
            command_set,
            input_focused: false,
            is_focused: false,
        })
    }

    /// Process input from the input box
    pub fn process_input(&mut self) {
        // Check if there's a submission in the input box
        if let Some(input) = self.input_widget.take_submission() {
            if input.is_empty() {
                return;
            }

            // Process the command using CommandSet
            let command_set = self.command_set.clone();
            let command_tx = self.command_tx.clone();

            // Spawn a task to process the command
            tokio::spawn(async move {
                let result = command_set.parse_line(&input).await;

                // If there's a result, send it to the log
                if let Some(lines) = result {
                    let _ = command_tx.send(ConsoleCommand::Lines(
                        lines.split('\n').map(Into::into).collect(),
                    ));
                }
            });
        }
    }

    /// Process commands from the command channel
    fn process_commands(&mut self) {
        // Process pending commands in the channel
        // We'll process up to 10 commands per frame to avoid blocking
        for _ in 0..10 {
            match self.command_rx.try_recv() {
                Ok(command) => match command {
                    ConsoleCommand::Clear => {
                        self.tracer_widget.clear_current_tab();
                    }
                    ConsoleCommand::Lines(messages) => {
                        self.tracer_widget.logs_mut().add_ansi_to_current(messages);
                    }
                },
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    error!("Command channel disconnected");
                    break;
                }
            }
        }
    }

    /// Get access to the tracer tabs
    pub fn tracer_mut(&mut self) -> &mut TracerWidget {
        &mut self.tracer_widget
    }

    /// Get access to the input box
    pub fn input_mut(&mut self) -> &mut InputWidget {
        &mut self.input_widget
    }

    /// Get access to the tracer tabs
    pub fn tracer_ref(&self) -> &TracerWidget {
        &self.tracer_widget
    }

    /// Get access to the input box
    pub fn input_ref(&self) -> &InputWidget {
        &self.input_widget
    }

    pub fn focus_input(&mut self) {
        self.input_focused = true;
        self.apply_focus();
    }

    pub fn focus_tracer(&mut self) {
        self.input_focused = false;
        self.apply_focus();
    }

    fn apply_focus(&mut self) {
        if self.is_focused {
            if self.input_focused {
                self.tracer_widget.unfocus();
                self.input_widget.focus();
            } else {
                self.tracer_widget.focus();
                self.input_widget.unfocus();
            }
        } else {
            self.tracer_widget.unfocus();
            self.input_widget.unfocus();
        }
    }

    /// Create a command sender that can be used to send commands to this console
    pub fn command_sender(&self) -> mpsc::UnboundedSender<ConsoleCommand> {
        self.command_tx.clone()
    }
}

impl TuiWidget for ConsoleWidget {
    fn need_draw(&self) -> bool {
        self.tracer_ref().need_draw() || self.input_ref().need_draw()
    }
    fn preprocess(&mut self) {
        // Process any pending commands
        self.process_commands();

        // Process input submissions
        self.process_input();

        self.tracer_widget.preprocess();
        self.input_widget.preprocess();
    }
    fn mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        self.tracer_widget.mouse_event(mouse)
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        // Split the area vertically: tracer on top, input box at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),   // Tracer takes most of the space
                Constraint::Length(1), // Gap
                Constraint::Length(1), // Input box is 1 row high (without border)
            ])
            .split(area);

        let [tracer_area, _, input_area] = [chunks[0], chunks[1], chunks[2]];
        let input_area = input_area.inner(Margin::new(1, 0));

        // Render the components
        self.tracer_widget.draw(tracer_area, buf);
        self.input_widget.draw(input_area, buf);
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        // Skip key release events
        if key.kind != KeyEventKind::Press {
            return false;
        }

        match key.code {
            // Toggle focus between panels on Tab
            KeyCode::Esc => {
                if self.input_focused {
                    self.focus_tracer();
                    true
                } else {
                    self.tracer_widget.key_event(key)
                }
            }
            KeyCode::Enter => {
                if self.input_focused {
                    self.input_widget.key_event(key)
                } else if !self.tracer_widget.key_event(key) {
                    self.focus_input();
                    true
                } else {
                    false
                }
            }
            _ => {
                // Pass to active component
                if !self.input_focused || key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.tracer_widget.key_event(key)
                } else {
                    self.input_widget.key_event(key)
                }
            }
        }
    }

    fn focus(&mut self) {
        self.is_focused = true;
        self.apply_focus();
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
        self.apply_focus();
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
