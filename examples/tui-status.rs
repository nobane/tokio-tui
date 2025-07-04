// tokio-tui/examples/tui-status.rs
use anyhow::Result;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::time::{Duration, Instant};
use tokio_tui::{
    ETAStatus, FileSizeStatus, IconMode, IconStatus, ProgressStatus, StatusLine, StatusWidget,
    TextAlignment, TextStatus, TimerStatus, Tui, TuiApp, TuiWidget, status_line,
};
use tokio_util::sync::CancellationToken;

// Define status lines using the macro
status_line! {
   struct DownloadLine {
       icon: IconStatus,
       progress: ProgressStatus,
       eta: ETAStatus,
       size: FileSizeStatus,
   }
}

status_line! {
   struct TimerLine {
       timer_icon: IconStatus,
       timer: TimerStatus,
   }
}

status_line! {
   struct SystemLine {
       system_icon: IconStatus,
       system_status: TextStatus,
       cpu_usage: TextStatus,
       memory_usage: TextStatus,
   }
}

status_line! {
   struct NetworkLine {
       network_icon: IconStatus,
       network_status: TextStatus,
       bandwidth: TextStatus,
   }
}

status_line! {
   struct UploadLine {
       upload_icon: IconStatus,
       upload_label: TextStatus,
       upload_progress: ProgressStatus,
   }
}

struct StatusDemoApp {
    status_widget: StatusWidget,
    run_token: CancellationToken,

    // Status line references
    download_line: DownloadLine,
    timer_line: TimerLine,
    system_line: SystemLine,
    network_line: NetworkLine,
    upload_line: UploadLine,

    // Simulation state
    download_current: u64,
    download_total: u64,
    last_update: Instant,
    download_speed: u64, // bytes per update
    system_counter: u32,
    network_messages: Vec<&'static str>,
    network_msg_index: usize,
}

impl StatusDemoApp {
    fn new(run_token: CancellationToken) -> Result<Self> {
        let mut status_widget = StatusWidget::new();

        // Create status lines using the macro-generated structs
        let download_line = DownloadLine::with_components(
            &mut status_widget,
            IconStatus::from(IconMode::Download),
            ProgressStatus::from((1024 * 1024 * 100, true)), // 100MB file with ETA
            ETAStatus::default(),
            FileSizeStatus::default(),
        );

        let timer_line = TimerLine::with_components(
            &mut status_widget,
            IconStatus::from(IconMode::Wait),
            TimerStatus::default(),
        );

        let system_line = SystemLine::with_components(
            &mut status_widget,
            IconStatus::from(IconMode::Spinner),
            TextStatus::from("System: Initializing..."),
            TextStatus::from(("CPU: 45%", TextAlignment::Right)),
            TextStatus::from(("RAM: 8.2GB", TextAlignment::Right)),
        );

        let network_line = NetworkLine::with_components(
            &mut status_widget,
            IconStatus::from(IconMode::Pulsate),
            TextStatus::from("Network: Connected"),
            TextStatus::from(("↑ 1.2MB/s ↓ 5.4MB/s", TextAlignment::Right)),
        );

        let upload_line = UploadLine::with_components(
            &mut status_widget,
            IconStatus::from(IconMode::Check),
            TextStatus::from("Upload backup.tar.gz"),
            ProgressStatus::from((1024 * 1024 * 50, 1024 * 1024 * 50, false)), // 50MB file, no ETA, completed
        );

        // Show all lines using the improved API
        status_widget.process_updates(vec![
            download_line.show(),
            timer_line.show(),
            system_line.show(),
            network_line.show(),
            upload_line.show(),
        ]);

        Ok(Self {
            status_widget,
            run_token,
            download_line,
            timer_line,
            system_line,
            network_line,
            upload_line,

            // Initialize simulation state
            download_current: 0,
            download_total: 1024 * 1024 * 100, // 100MB
            last_update: Instant::now(),
            download_speed: 1024 * 1024, // 1MB per update
            system_counter: 0,
            network_messages: vec![
                "Connected to server",
                "Receiving data...",
                "Processing packets",
                "Optimizing connection",
                "Syncing with remote",
                "Checking bandwidth",
                "Network stable",
            ],
            network_msg_index: 0,
        })
    }

    fn update_simulation(&mut self) {
        if self.last_update.elapsed() < Duration::from_millis(500) {
            return;
        }

        let mut updates = Vec::new();

        // Update download progress if not complete
        if self.download_current < self.download_total {
            self.download_current =
                (self.download_current + self.download_speed).min(self.download_total);

            updates.push(
                self.download_line
                    .progress
                    .set_progress(self.download_current, self.download_total),
            );

            updates.push(
                self.download_line
                    .eta
                    .update_progress(self.download_current, self.download_total),
            );

            updates.push(
                self.download_line
                    .size
                    .set_size(self.download_current, self.download_total),
            );

            // Change icon when complete
            if self.download_current >= self.download_total {
                updates.push(self.download_line.icon.set(IconMode::Check));
            }
        }

        // Update system status
        self.system_counter += 1;
        let system_messages = [
            "System: Running normally",
            "System: Processing tasks",
            "System: Optimizing performance",
            "System: Checking health",
            "System: Updating components",
        ];
        let system_msg = system_messages[self.system_counter as usize % system_messages.len()];

        updates.push(
            self.system_line
                .system_status
                .set_text(system_msg, Style::default().fg(Color::White)),
        );

        // Update network status
        let network_msg = self.network_messages[self.network_msg_index];
        self.network_msg_index = (self.network_msg_index + 1) % self.network_messages.len();

        updates.push(self.network_line.network_status.set_text(
            format!("Network: {network_msg}"),
            Style::default().fg(Color::Green),
        ));

        // Process all updates
        self.status_widget.process_updates(updates);
        self.last_update = Instant::now();
    }

    fn reset_download(&mut self) {
        self.download_current = 0;

        let updates = vec![
            self.download_line.icon.set(IconMode::Download),
            self.download_line
                .progress
                .set_progress(0, self.download_total),
            self.download_line
                .eta
                .update_progress(0, self.download_total),
            self.download_line.size.set_size(0, self.download_total),
        ];

        self.status_widget.process_updates(updates);
    }

    fn cycle_icons(&mut self) {
        let modes = [
            IconMode::Spinner,
            IconMode::Download,
            IconMode::Pulsate,
            IconMode::Check,
            IconMode::Cross,
            IconMode::Alert,
            IconMode::Question,
            IconMode::Pause,
            IconMode::Wait,
            IconMode::Cancel,
        ];

        static mut ICON_INDEX: usize = 0;
        unsafe {
            ICON_INDEX = (ICON_INDEX + 1) % modes.len();
            let mode = modes[ICON_INDEX];

            let updates = vec![
                self.download_line.icon.set(mode),
                self.upload_line.upload_icon.set(mode),
            ];

            self.status_widget.process_updates(updates);
        }
    }

    fn reset_timer(&mut self) {
        let update = self.timer_line.timer.reset();
        self.status_widget.process_cell_update(update);
    }

    fn widget_refs(&mut self, area: Option<Rect>) -> [(&mut dyn TuiWidget, Rect); 1] {
        let zero_rect = Rect::new(0, 0, 0, 0);
        let mut status_area = zero_rect;

        if let Some(area) = area {
            status_area = area;
        }

        [(&mut self.status_widget, status_area)]
    }
}

impl TuiApp for StatusDemoApp {
    fn should_draw(&mut self) -> bool {
        for (widget, _) in self.widget_refs(None) {
            if widget.need_draw() {
                return true;
            }
        }
        false
    }

    fn before_frame(&mut self, _terminal: &tokio_tui::TerminalBackend) {
        // Update simulation state
        self.update_simulation();

        // Preprocess all widgets
        for (widget, _) in self.widget_refs(None) {
            widget.preprocess();
        }
    }

    fn render(&mut self, frame: &mut tokio_tui::TerminalFrame) {
        let area = frame.area();

        // Render status widget at the bottom of the screen
        let status_area = Rect {
            x: area.x,
            y: area.y, // Reserve 7 lines for status
            width: area.width,
            height: 7,
        };

        // Render help text in the remaining area
        let help_area = Rect {
            x: area.x,
            y: area.y + 7,
            width: area.width,
            height: area.height.saturating_sub(7),
        };

        // Render help text
        let help_text = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "New Status Widget Demo (Type-Safe Closures)",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from("This demo showcases the new closure-based status cells:"),
            Line::from("• No hard-coded StatusUpdate enum"),
            Line::from("• Type-safe updates using closures"),
            Line::from("• Ergonomic status_line! macro"),
            Line::from("• Each cell type provides its own update methods"),
            Line::from(""),
            Line::from("Status Lines Generated by Macro:"),
            Line::from("• DownloadLine { icon, progress, eta, size }"),
            Line::from("• TimerLine { timer_icon, timer }"),
            Line::from("• SystemLine { system_icon, system_status, cpu_usage, memory_usage }"),
            Line::from("• NetworkLine { network_icon, network_status, bandwidth }"),
            Line::from("• UploadLine { upload_icon, upload_label, upload_progress }"),
            Line::from(""),
            Line::from("Controls:"),
            Line::from("• Ctrl+R - Reset download simulation"),
            Line::from("• Ctrl+I - Cycle through different icon modes"),
            Line::from("• Ctrl+T - Reset timer"),
            Line::from("• Ctrl+Q - Quit"),
            Line::from(""),
            Line::from("Watch the status bars below update in real-time!"),
        ]);

        let help_block = Block::default()
            .title("New Status Widget Demo")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        frame.render_widget(
            Paragraph::new(help_text)
                .block(help_block)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Left),
            help_area,
        );

        // Render status widget
        let buf = frame.buffer_mut();
        for (widget, widget_area) in self.widget_refs(Some(status_area)) {
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
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.run_token.cancel();
                }
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.reset_download();
                }
                KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.cycle_icons();
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.reset_timer();
                }
                _ => {
                    // Pass other keys to status widget if it needs them
                    for (widget, _) in self.widget_refs(None) {
                        widget.key_event(key);
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create cancellation token for graceful shutdown
    let run_token = CancellationToken::new();

    // Create and run the application
    let app = StatusDemoApp::new(run_token)?;
    Tui::new()?.run(app)?;

    Ok(())
}
