// tokio-tui/examples/tui-tracer-multi.rs
use anyhow::Result;
use chrono::Local;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Style},
};
use std::time::Duration;
use tokio_tracer::{TraceData, TraceLevel, Tracer};
use tokio_tui::{TraceEventSender, TracerWidget, Tui, TuiApp, TuiWidget};
use tokio_util::sync::CancellationToken;
use tracing::{Level, debug, error, info, trace, warn};

struct MultiSourceTracerDemo {
    tracer_widget: TracerWidget,
    run_token: CancellationToken,
}

impl MultiSourceTracerDemo {
    fn new(run_token: CancellationToken, tracer: Tracer) -> Result<Self> {
        // Create the tracer widget
        let mut tracer_widget = TracerWidget::new(tracer)?;

        // Set a default prefix with direct StyledText (no callback)
        tracer_widget.set_default_prefix_with_style("M", Style::default().fg(Color::Gray));

        // Register different sources with custom emoji prefixes as direct StyledText
        let database_sender = tracer_widget.register_source_with_style(
            "database",
            "D",
            Style::default().fg(Color::Magenta),
        );

        let network_sender =
            tracer_widget.register_source_with_style("network", "N", Style::new().fg(Color::Green));

        let auth_sender =
            tracer_widget.register_source_with_style("auth", "A", Style::new().fg(Color::Red));

        let file_sender = tracer_widget.register_source("filesystem", "F");

        // Spawn tasks to generate logs from each source
        spawn_database_logs(database_sender, run_token.clone());
        spawn_network_logs(network_sender, run_token.clone());
        spawn_auth_logs(auth_sender, run_token.clone());
        spawn_filesystem_logs(file_sender, run_token.clone());

        // Focus the widget
        tracer_widget.focus();

        Ok(Self {
            tracer_widget,
            run_token,
        })
    }

    fn widget_refs(&mut self, area: Option<Rect>) -> [(&mut dyn TuiWidget, Rect); 1] {
        // Initialize with zero-sized rect
        let zero_rect = Rect::new(0, 0, 0, 0);
        let mut tracer_area = zero_rect;

        // If area is provided, use the entire area for the tracer
        if let Some(area) = area {
            tracer_area = area;
        }

        [(&mut self.tracer_widget, tracer_area)]
    }
}

impl TuiApp for MultiSourceTracerDemo {
    fn should_draw(&mut self) -> bool {
        // Check if any widget needs drawing
        for (widget, _) in self.widget_refs(None) {
            if widget.need_draw() {
                return true;
            }
        }
        false
    }

    fn before_frame(&mut self, _terminal: &tokio_tui::TerminalBackend) {
        // Preprocess all widgets
        for (widget, _) in self.widget_refs(None) {
            widget.preprocess();
        }
    }

    fn render(&mut self, frame: &mut tokio_tui::TerminalFrame) {
        let area = frame.area();
        let buf = frame.buffer_mut();

        // Get widgets with calculated areas
        for (widget, widget_area) in self.widget_refs(Some(area)) {
            if widget_area.width > 0 && widget_area.height > 0 {
                widget.draw(widget_area, buf);
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.run_token.is_cancelled()
    }

    fn handle_key_events(&mut self, keys: Vec<KeyEvent>) {
        for key in keys {
            match key.code {
                // Quit application on Ctrl+Q
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.run_token.cancel();
                }
                // Pass other key events to the tracer widget
                _ => {
                    self.tracer_widget.key_event(key);
                }
            }
        }
    }
}

// Helper function to create trace events for external sources
fn create_trace_event(id: u64, level: Level, message: String) -> tokio_tracer::TraceEvent {
    let event = TraceData {
        id,
        timestamp: Local::now(),
        level: TraceLevel(level),
        target: "external".to_string(),
        name: "external_event".to_string(),
        module_path: Some("external_module".to_string()),
        file: Some("external.rs".to_string()),
        line: Some(42),
        message,
        fields: std::collections::HashMap::new(),
        span_name: None,
        span_hierarchy: None,
    };

    std::sync::Arc::new(event)
}

// Spawn tasks for each type of log source
fn spawn_database_logs(sender: TraceEventSender, token: CancellationToken) {
    tokio::spawn(async move {
        let mut counter = 0;
        let operations = ["SELECT", "INSERT", "UPDATE", "DELETE", "JOIN", "INDEX"];
        let tables = ["users", "products", "orders", "payments", "logs"];

        loop {
            counter += 1;
            let op = operations[counter % operations.len()];
            let table = tables[counter % tables.len()];

            // Mix of different log levels
            let (level, message) = match counter % 10 {
                0 => (
                    Level::ERROR,
                    format!("Database query failed: {op} on {table} table"),
                ),
                1 => (
                    Level::WARN,
                    format!("Slow query: {op} took 1.2s on {table}"),
                ),
                2..=3 => (Level::INFO, format!("Query executed: {op} on {table}")),
                _ => (Level::DEBUG, format!("Preparing query: {op} {table}")),
            };

            let event = create_trace_event(counter as u64, level, message);
            sender(event, vec!["Main".to_string()]);

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(800)) => {}
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

fn spawn_network_logs(sender: TraceEventSender, token: CancellationToken) {
    tokio::spawn(async move {
        let mut counter = 0;
        let endpoints = [
            "api/users",
            "api/products",
            "api/auth",
            "api/health",
            "api/stats",
        ];
        let methods = ["GET", "POST", "PUT", "DELETE", "PATCH"];

        loop {
            counter += 1;
            let endpoint = endpoints[counter % endpoints.len()];
            let method = methods[counter % methods.len()];

            // Mix of different log levels
            let (level, mut message) = match counter % 8 {
                0 => (
                    Level::ERROR,
                    format!("Connection refused: {method} {endpoint}"),
                ),
                1 => (
                    Level::WARN,
                    format!("Slow response: {method} {endpoint} (350ms)"),
                ),
                2..=4 => (
                    Level::INFO,
                    format!("Request: {method} {endpoint} (200 OK)"),
                ),
                _ => (
                    Level::DEBUG,
                    format!("Processing request : {method} {endpoint}"),
                ),
            };
            for _i in 0..20 {
                message.push_str("looooooong ");
            }

            let event = create_trace_event(counter as u64 + 1000, level, message);
            sender(event, vec!["Main".to_string()]);

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(650)) => {}
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

fn spawn_auth_logs(sender: TraceEventSender, token: CancellationToken) {
    tokio::spawn(async move {
        let mut counter = 0;
        let actions = [
            "login",
            "logout",
            "password_reset",
            "token_refresh",
            "permission_check",
        ];
        let users = ["admin", "user123", "guest", "system", "api_user"];

        loop {
            counter += 1;
            let action = actions[counter % actions.len()];
            let user = users[counter % users.len()];

            // Mix of different log levels
            let (level, message) = match counter % 15 {
                0 => (
                    Level::ERROR,
                    format!("Authentication failed: {action} for user {user}"),
                ),
                1..=2 => (
                    Level::WARN,
                    format!("Multiple {action} attempts for {user}"),
                ),
                3..=6 => (Level::INFO, format!("Successful {action}: user {user}")),
                _ => (Level::DEBUG, format!("Auth request: {action} for {user}")),
            };

            let event = create_trace_event(counter as u64 + 2000, level, message);
            sender(event, vec!["Main".to_string()]);

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(1200)) => {}
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

fn spawn_filesystem_logs(sender: TraceEventSender, token: CancellationToken) {
    tokio::spawn(async move {
        let mut counter = 0;
        let operations = ["read", "write", "delete", "create", "move", "copy"];
        let paths = [
            "/etc/config.json",
            "/var/log/app.log",
            "/usr/bin/app",
            "/tmp/cache",
            "/home/user/data",
        ];

        loop {
            counter += 1;
            let op = operations[counter % operations.len()];
            let path = paths[counter % paths.len()];

            // Mix of different log levels
            let (level, message) = match counter % 12 {
                0 => (Level::ERROR, format!("IO Error: Failed to {op} {path}")),
                1..=2 => (
                    Level::WARN,
                    format!("Slow disk operation: {op} {path} (150ms)"),
                ),
                3..=5 => (Level::INFO, format!("File {op}: {path}")),
                _ => (Level::DEBUG, format!("Processing {op}: {path}")),
            };

            let event = create_trace_event(counter as u64 + 3000, level, message);
            sender(event, vec!["Main".to_string()]);

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(950)) => {}
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up cancellation token for graceful shutdown
    let run_token = CancellationToken::new();
    let run_token_clone = run_token.clone();

    // Start a background task for system logs (via the normal tracer)
    tokio::spawn(async move {
        let mut counter = 0;
        loop {
            counter += 1;
            trace!("System trace message #{}", counter);
            debug!("System debug message #{}", counter);
            info!("System info message #{}", counter);
            info!(
                "System info verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong verrrrry loooooong message #{}",
                counter
            );

            if counter % 5 == 0 {
                warn!("System warning message #{}", counter);
            }
            if counter % 10 == 0 {
                error!("System error message #{}", counter);
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(700)) => {}
                _ = run_token_clone.cancelled() => {
                    info!("Background task exiting");
                    break;
                }
            }
        }
    });

    // Initialize the main tracer
    let tracer = Tracer::init_default()?;

    // Create and run the application
    let app = MultiSourceTracerDemo::new(run_token, tracer)?;
    Tui::new()?.run(app)?;

    Ok(())
}
