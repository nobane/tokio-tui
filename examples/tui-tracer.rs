// tokio-tui/examples/tui-tracer.rs
use anyhow::Result;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
};
use tokio_tui::{TracerWidget, Tui, TuiApp, TuiWidget};
use tokio_util::sync::CancellationToken;

struct TracerTuiDemo {
    tracer_widget: TracerWidget,
    run_token: CancellationToken,
}

impl TracerTuiDemo {
    fn new(run_token: CancellationToken, tracer: tokio_tracer::Tracer) -> Result<Self> {
        // Create the tracer widget
        let mut tracer_widget = TracerWidget::new(tracer)?;
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

impl TuiApp for TracerTuiDemo {
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

#[tokio::main]
async fn main() -> Result<()> {
    // Set up cancellation token for graceful shutdown
    let run_token = CancellationToken::new();
    let run_token_clone = run_token.clone();

    // Start a background task to generate logs
    tokio::spawn(async move {
        use std::time::Duration;
        use tracing::{debug, error, info, trace, warn};

        let mut counter = 0;
        loop {
            counter += 1;

            trace!("System trace message #{}", counter);
            debug!("System debug message #{}", counter);
            info!("System info message #{}", counter);

            if counter % 5 == 0 {
                warn!("System warning message #{}", counter);
            }

            if counter % 10 == 0 {
                error!("System error message #{}", counter);
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                _ = run_token_clone.cancelled() => {
                    info!("Background task exiting");
                    break;
                }
            }
        }
    });

    // Initialize the tracer
    let tracer = tokio_tracer::Tracer::init_default()?;

    // Create and run the application
    let app = TracerTuiDemo::new(run_token, tracer)?;
    Tui::new()?.run(app)?;

    Ok(())
}
