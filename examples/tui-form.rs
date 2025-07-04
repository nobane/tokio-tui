// tokio-tui/examples/tui-form.rs
use anyhow::Result;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tracing::info;

use tokio_tui::{
    FormData, FormWidget, TracerWidget, Tui, TuiApp, TuiEdit, TuiForm, TuiList, TuiWidget, layout,
    vertical,
};

#[derive(Debug, Default, Clone, PartialEq, Serialize, TuiEdit)]
pub enum PriorityLevel {
    #[default]
    LOW,
    MEDIUM,
    HIGH,
    CRITICAL,
}

#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct AddressForm {
    pub street: String,
    pub city: String,
    pub state: String,
    pub zip: String,
}

#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct UserProfileForm {
    pub name: String,
    pub username: String,
    pub emails: Vec<String>,
    pub address: TuiForm<AddressForm>,
    pub other_addresses: TuiList<AddressForm>,
    pub contacts: TuiList<ContactForm>,
}

#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct ContactForm {
    pub contact_type: String,
    pub value: String,
    pub priority: PriorityLevel,
    pub data: TuiList<ContactMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct ContactMetadata {
    pub data_type: String,
    pub data_value: String,
}

struct NestedFormDemoApp {
    form_widget: FormWidget,
    tracer_widget: TracerWidget,
    active_widget: ActiveWidget,
    run_token: CancellationToken,
}

enum ActiveWidget {
    Form,
    Tracer,
}

impl NestedFormDemoApp {
    fn new(run_token: CancellationToken, tracer: tokio_tracer::Tracer) -> Result<Self> {
        // Create data using struct initialization
        let user_profile = UserProfileForm {
            name: "John Doe".to_string(),
            username: "johndoe".to_string(),
            emails: vec![
                "johndoe@example.com".to_string(),
                "jdoe@threeletteragency.gov".to_string(),
                "john.doe@coldmail.com".to_string(),
                "johhny@yeehaw.com".to_string(),
            ],
            address: TuiForm(AddressForm {
                street: "123 Main St".to_string(),
                city: "Anytown".to_string(),
                state: "CA".to_string(),
                zip: "12345".to_string(),
            }),
            other_addresses: TuiList(vec![AddressForm {
                street: "456 Oak Ave".to_string(),
                city: "Other City".to_string(),
                state: "NY".to_string(),
                zip: "67890".to_string(),
            }]),
            contacts: TuiList(vec![ContactForm {
                contact_type: "Email".to_string(),
                value: "john.doe@example.com".to_string(),
                priority: PriorityLevel::HIGH,
                data: TuiList::empty(),
            }]),
        };

        // Create form for editing user profile
        let run_token2 = run_token.clone();
        let mut form_widget = FormWidget::new("User Profile Form")
            .with_data(&user_profile)
            .with_submit(move |_| {
                info!("Form submit");
                run_token2.cancel();
            });

        // Create tracer widget
        let tracer_widget = TracerWidget::new(tracer)?;

        // Focus the form by default
        form_widget.focus();

        Ok(NestedFormDemoApp {
            form_widget,
            tracer_widget,
            active_widget: ActiveWidget::Form,
            run_token,
        })
    }

    fn focus_widget(&mut self, widget: ActiveWidget) {
        // Unfocus all widgets
        self.form_widget.unfocus();
        self.tracer_widget.unfocus();

        // Focus the selected widget
        match widget {
            ActiveWidget::Form => self.form_widget.focus(),
            ActiveWidget::Tracer => self.tracer_widget.focus(),
        }

        self.active_widget = widget;
    }

    fn widget_refs(&mut self, area: Option<Rect>) -> [(&mut dyn TuiWidget, Rect); 2] {
        // Initialize with zero-sized rects
        let zero_rect = Rect::new(0, 0, 0, 0);
        let mut form_area = zero_rect;
        let mut tracer_area = zero_rect;

        // Calculate areas if an area was provided
        if let Some(area) = area {
            // Vertical split: form (top) and tracer (bottom)
            if let [form_section, tracer_section] = layout![
                () => vertical![Percentage(70), Percentage(30)]
            ]
            .split(&(), area)[..]
            {
                form_area = form_section;
                tracer_area = tracer_section;
            }
        }

        [
            (&mut self.form_widget, form_area),
            (&mut self.tracer_widget, tracer_area),
        ]
    }

    // Get the form data for after submission
    pub fn get_form_data(&self) -> UserProfileForm {
        UserProfileForm::from_fields(self.form_widget.get_fields())
    }
}

impl TuiApp for NestedFormDemoApp {
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
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.run_token.cancel();
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.focus_widget(ActiveWidget::Form);
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.focus_widget(ActiveWidget::Tracer);
                }
                KeyCode::Tab => {
                    // Toggle focus between widgets
                    match self.active_widget {
                        ActiveWidget::Form => self.focus_widget(ActiveWidget::Tracer),
                        ActiveWidget::Tracer => self.focus_widget(ActiveWidget::Form),
                    }
                }
                _ => {
                    // Pass key to the focused widget
                    match self.active_widget {
                        ActiveWidget::Form => self.form_widget.key_event(key),
                        ActiveWidget::Tracer => self.tracer_widget.key_event(key),
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

    let tracer = tokio_tracer::Tracer::init_default()?;
    // Create app instance
    let app = NestedFormDemoApp::new(run_token_clone, tracer)?;

    // Run the TUI application
    let app = Tui::new()?.run(app)?;

    // Get form data after submission
    let form_data = app.get_form_data();

    // Print as JSON
    match serde_json::to_string_pretty(&form_data) {
        Ok(json) => {
            println!("\nForm submitted with data:\n{json}");
        }
        Err(e) => {
            println!("\nError serializing form data: {e}");
        }
    }

    Ok(())
}
