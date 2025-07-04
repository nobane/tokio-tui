// tokio-tui/examples/tui-console.rs
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio_tui::{
    CommandSet, CommandSetBuilder, ConsoleCommand, ConsoleWidget, Tui, TuiApp, TuiWidget,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

// Main application struct
struct ConsoleDemo {
    console_widget: ConsoleWidget,
    run_token: CancellationToken,
    append_during_render: bool,
    entry_counter: usize,
}

impl ConsoleDemo {
    fn new(
        run_token: CancellationToken,
        tracer: tokio_tracer::Tracer,
        append_during_render: bool,
    ) -> Result<Self> {
        // Create command set
        let command_set = Self::create_command_set();

        // Create console widget
        let console_widget = ConsoleWidget::new(tracer, command_set)?;

        // Create app
        let mut app = Self {
            console_widget,
            run_token,
            append_during_render,
            entry_counter: 0,
        };

        // Add initial lines if requested
        app.console_widget
            .tracer_mut()
            .logs_mut()
            .add_tab("Main".to_string(), "Main");

        app.console_widget.focus();

        Ok(app)
    }

    fn add_styled_entry(&mut self) {
        self.entry_counter += 1;

        // Convert the styled text to a string for the console widget
        let entry_text = format!("Generated message #{}", self.entry_counter);

        // Send to console
        if let Err(e) = self
            .console_widget
            .command_sender()
            .send(ConsoleCommand::Lines(vec![entry_text]))
        {
            eprintln!("Failed to send message: {e}");
        }

        // Also send to tracer
        trace!("System trace message #{}", self.entry_counter);
        debug!("System debug message #{}", self.entry_counter);
        info!("System info message #{}", self.entry_counter);

        if self.entry_counter % 5 == 0 {
            warn!("System warning message #{}", self.entry_counter);
        }

        if self.entry_counter % 10 == 0 {
            error!("System error message #{}", self.entry_counter);
        }
    }

    fn create_command_set() -> CommandSet {
        CommandSetBuilder::<()>::new()
            .add_simple("trace", "Log a trace level message", |ctx| async move {
                let message = ctx.args.join(" ");
                trace!("TRACE: {message}");

                Ok(Some(format!("Logged trace message: {message}")))
            })
            .add_simple("debug", "Log a debug level message", |ctx| async move {
                let message = ctx.args.join(" ");
                debug!("DEBUG: {message}");

                Ok(Some(format!("Logged debug message: {message}")))
            })
            .add_simple("info", "Log an info level message", |ctx| async move {
                let message = ctx.args.join(" ");
                info!("INFO: {message}");

                Ok(Some(format!("Logged info message: {message}")))
            })
            .add_simple("warn", "Log a warning level message", |ctx| async move {
                let message = ctx.args.join(" ");
                warn!("WARN: {message}");

                Ok(Some(format!("Logged warning message: {message}")))
            })
            .add_simple("error", "Log an error level message", |ctx| async move {
                let message = ctx.args.join(" ");
                error!("ERROR: {message}");

                Ok(Some(format!("Logged error message: {message}")))
            })
            .add_simple("repeat", "Repeat a message N times", |ctx| async move {
                if ctx.args.is_empty() {
                    return Ok(Some("Usage: repeat <count> [message]".to_string()));
                }

                let count = ctx.args[0].parse::<usize>().map_err(|_| {
                    anyhow::anyhow!("Invalid count: {}, must be a number", ctx.args[0])
                })?;

                let message = if ctx.args.len() > 1 {
                    ctx.args[1..].join(" ")
                } else {
                    "Repeated message".to_string()
                };

                let mut messages = Vec::with_capacity(count);
                for i in 0..count {
                    messages.push(format!("{message} #{i}"));
                }
                let messages = messages.join("\n");
                Ok(Some(format!("Generated {count} messages\n{messages}")))
            })
            .build(())
    }
}

impl TuiApp for ConsoleDemo {
    fn should_draw(&mut self) -> bool {
        self.console_widget.need_draw()
    }

    fn before_frame(&mut self, #[allow(unused)] terminal: &tokio_tui::TerminalBackend) {
        self.console_widget.preprocess();
    }

    fn render(&mut self, frame: &mut tokio_tui::TerminalFrame) {
        // Add a new entry if append_during_render is enabled
        if self.append_during_render {
            self.add_styled_entry();
        }

        // Render the console widget using the full frame
        self.console_widget.draw(frame.area(), frame.buffer_mut());
    }

    fn should_quit(&self) -> bool {
        self.run_token.is_cancelled()
    }

    fn handle_mouse_events(&mut self, mouse_events: Vec<crossterm::event::MouseEvent>) {
        for event in mouse_events {
            self.console_widget.mouse_event(event);
        }
    }

    fn handle_key_events(&mut self, keys: Vec<KeyEvent>) {
        for key in keys {
            // Skip key release events
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                // Toggle auto-append on Ctrl+T
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.append_during_render = !self.append_during_render;
                    let status = if self.append_during_render {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    if let Err(e) =
                        self.console_widget
                            .command_sender()
                            .send(ConsoleCommand::Lines(vec![format!(
                                "Auto-append is now {}",
                                status
                            )]))
                    {
                        eprintln!("Failed to send toggle status: {e}");
                    }
                }

                // Manually add an entry on Ctrl+A
                KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.add_styled_entry();
                }

                // Quit application on Ctrl+Q
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.run_token.cancel();
                }

                // Handle key in console widget
                _ => {
                    self.console_widget.key_event(key);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();

    // Parse flags
    let append_during_render = args
        .iter()
        .any(|arg| arg == "--append-render" || arg == "-ar");

    // Set up cancellation token for graceful shutdown
    let run_token = CancellationToken::new();

    // Initialize tracer
    let tracer = tokio_tracer::Tracer::init_default()?;

    info!("Starting console demo with append_during_render={append_during_render}");

    if let Some(lines) = args
        .iter()
        .find(|arg| arg.starts_with("--lines="))
        .and_then(|l| l.trim_start_matches("--lines=").parse::<usize>().ok())
    {
        // Initialize the styled demo with the specified number of lines
        for _ in 0..lines {
            info!("line");
        }
    }
    // Create and run the application
    let app = ConsoleDemo::new(run_token.clone(), tracer, append_during_render)?;

    // Run the application
    Tui::new()?.run(app)?;

    Ok(())
}
