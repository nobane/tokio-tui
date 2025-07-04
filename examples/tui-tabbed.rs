// tokio-tui/examples/tui-tabbed.rs
use anyhow::Result;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Style},
};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use tokio_tui::{
    InputWidget, TabbedScrollbox, Tui, TuiApp, TuiWidget, horizontal, layout, vertical,
};

// Define an enum for tab types
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum LogTab {
    System,
    Network,
    Application,
    Debug,
}

// Implement Display for the enum
impl std::fmt::Display for LogTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            LogTab::System => "System",
            LogTab::Network => "Network",
            LogTab::Application => "Application",
            LogTab::Debug => "Debug",
        };
        write!(f, "{name}")
    }
}

struct TabbedDemo {
    enum_tabs: TabbedScrollbox<LogTab>,
    string_tabs: TabbedScrollbox<String>,
    input_box: InputWidget,
    last_update: Instant,
    counter: u32,
    dynamic_tab_counter: u32,
    run_token: CancellationToken,
    active_widget: ActiveWidget,
}

enum ActiveWidget {
    EnumTabs,
    StringTabs,
    InputBox,
}

impl TabbedDemo {
    fn new(run_token: CancellationToken) -> Result<Self> {
        // Create a tabbed scrollbox with enum-based tabs
        let mut enum_tabs =
            TabbedScrollbox::<LogTab>::new("Enum Tabs").style(Style::default().fg(Color::Green));

        // Add tabs to enum-based widget
        enum_tabs.add_tab(LogTab::System, "System Log");
        enum_tabs.add_tab(LogTab::Network, "Network Log");
        enum_tabs.add_tab(LogTab::Application, "App Log");
        enum_tabs.add_tab(LogTab::Debug, "Debug Info");

        // Add initial content to enum tabs
        for i in 1..10 {
            enum_tabs.add_ansi_to_tab(&LogTab::System, format!("Enum System entry {i}"));
        }
        enum_tabs.add_ansi_to_tab(&LogTab::Network, "Enum Network traffic".to_string());
        enum_tabs.add_ansi_to_tab(&LogTab::Application, "Enum App started".to_string());
        enum_tabs.add_ansi_to_tab(&LogTab::Debug, "Enum Debug info".to_string());

        // Create a tabbed scrollbox with string-based tabs
        let mut string_tabs =
            TabbedScrollbox::<String>::new("String Tabs").style(Style::default().fg(Color::Cyan));

        // Add tabs to string-based widget
        string_tabs.add_tab("system".to_string(), "System Log");
        string_tabs.add_tab("network".to_string(), "Network Log");
        string_tabs.add_tab("application".to_string(), "App Log");
        string_tabs.add_tab("debug".to_string(), "Debug Info");

        // Add initial content to string tabs
        for i in 1..10 {
            string_tabs.add_ansi_to_tab(&"system".to_string(), format!("String System entry {i}"));
        }
        string_tabs.add_ansi_to_tab(&"network".to_string(), "String Network traffic".to_string());
        string_tabs.add_ansi_to_tab(&"application".to_string(), "String App started".to_string());
        string_tabs.add_ansi_to_tab(&"debug".to_string(), "String Debug info".to_string());

        // Create a simple input box
        let input_box = InputWidget::new();

        // Focus the enum tabbed log by default
        enum_tabs.focus();

        Ok(TabbedDemo {
            enum_tabs,
            string_tabs,
            input_box,
            last_update: Instant::now(),
            counter: 0,
            dynamic_tab_counter: 0,
            run_token,
            active_widget: ActiveWidget::EnumTabs,
        })
    }

    fn periodic_update(&mut self) {
        if self.last_update.elapsed() > Duration::from_secs(2) {
            // Add entries to different tabs
            self.enum_tabs.add_ansi_to_tab(
                &LogTab::System,
                format!("Enum system update {}", self.counter),
            );

            self.string_tabs.add_ansi_to_tab(
                &"system".to_string(),
                format!("String system update {}", self.counter),
            );

            if self.counter % 3 == 0 {
                self.enum_tabs.add_ansi_to_tab(
                    &LogTab::Network,
                    format!("Enum network packet {}", self.counter),
                );

                self.string_tabs.add_ansi_to_tab(
                    &"network".to_string(),
                    format!("String network packet {}", self.counter),
                );
            }

            self.counter += 1;
            self.last_update = Instant::now();
        }
    }

    fn focus_widget(&mut self, widget: ActiveWidget) {
        // Unfocus all widgets
        self.enum_tabs.unfocus();
        self.string_tabs.unfocus();
        self.input_box.unfocus();

        // Focus the selected widget
        match widget {
            ActiveWidget::EnumTabs => self.enum_tabs.focus(),
            ActiveWidget::StringTabs => self.string_tabs.focus(),
            ActiveWidget::InputBox => self.input_box.focus(),
        }

        self.active_widget = widget;
    }

    fn widget_refs(&mut self, area: Option<Rect>) -> [(&mut dyn TuiWidget, Rect); 3] {
        // Initialize with zero-sized rects
        let zero_rect = Rect::new(0, 0, 0, 0);
        let mut enum_tabs_area = zero_rect;
        let mut string_tabs_area = zero_rect;
        let mut input_box_area = zero_rect;

        // Calculate areas if an area was provided
        if let Some(area) = area {
            // Vertical split: main area (top) and input box (bottom)
            if let [main_area, input_area] =
                layout![() => vertical![Fill(1), Length(3)]].split(&(), area)[..]
            {
                input_box_area = input_area;

                // Horizontal split in the main area for the two tab widgets
                if let [left_area, right_area] =
                    layout![() => horizontal![Percentage(50), Percentage(50)]].split(&(), main_area)
                        [..]
                {
                    enum_tabs_area = left_area;
                    string_tabs_area = right_area;
                }
            }
        }

        [
            (&mut self.enum_tabs, enum_tabs_area),
            (&mut self.string_tabs, string_tabs_area),
            (&mut self.input_box, input_box_area),
        ]
    }
}

impl TuiApp for TabbedDemo {
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
        // Update content periodically
        self.periodic_update();

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
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.run_token.cancel();
                }
                KeyCode::Char('1') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Select first tab in both widgets
                    self.enum_tabs.select_tab(&LogTab::System);
                    self.string_tabs.select_tab(&"system".to_string());
                }
                KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Select second tab in both widgets
                    self.enum_tabs.select_tab(&LogTab::Network);
                    self.string_tabs.select_tab(&"network".to_string());
                }
                KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Select third tab in both widgets
                    self.enum_tabs.select_tab(&LogTab::Application);
                    self.string_tabs.select_tab(&"application".to_string());
                }
                KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Select fourth tab in both widgets
                    self.enum_tabs.select_tab(&LogTab::Debug);
                    self.string_tabs.select_tab(&"debug".to_string());
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Next tab in both widgets
                    self.enum_tabs.next_tab();
                    self.string_tabs.next_tab();
                }
                KeyCode::Char('T') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Previous tab in both widgets
                    self.enum_tabs.prev_tab();
                    self.string_tabs.prev_tab();
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Focus enum tabs
                    self.focus_widget(ActiveWidget::EnumTabs);
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Focus string tabs
                    self.focus_widget(ActiveWidget::StringTabs);
                }
                KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Focus input box
                    self.focus_widget(ActiveWidget::InputBox);
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Create a new dynamic tab (only in string tabs)
                    self.dynamic_tab_counter += 1;
                    let new_tab_name = format!("dynamic_{}", self.dynamic_tab_counter);
                    self.string_tabs.add_tab(
                        new_tab_name.clone(),
                        format!("Dynamic {}", self.dynamic_tab_counter),
                    );
                    self.string_tabs.add_ansi_to_tab(
                        &new_tab_name,
                        format!(
                            "This is dynamic tab {} created at runtime",
                            self.dynamic_tab_counter
                        ),
                    );
                    self.string_tabs.select_tab(&new_tab_name);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Clear current tab in both widgets
                    self.enum_tabs.clear_current_tab();
                    self.string_tabs.clear_current_tab();
                }
                _ => {
                    // Pass key to the focused widget
                    match self.active_widget {
                        ActiveWidget::EnumTabs => self.enum_tabs.key_event(key),
                        ActiveWidget::StringTabs => self.string_tabs.key_event(key),
                        ActiveWidget::InputBox => self.input_box.key_event(key),
                    };
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create cancellation token for graceful shutdown
    let run_token = CancellationToken::new();
    let run_token_clone = run_token.clone();

    let app = TabbedDemo::new(run_token_clone.clone())?;

    Tui::new()?.run(app)?;

    Ok(())
}
