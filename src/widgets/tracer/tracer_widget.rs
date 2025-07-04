// tokio-tui/src/widgets/tracer/tracer_widget.rs
use std::sync::Arc;

use anyhow::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::Borders,
};
use tokio::sync::mpsc;
use tracing::{Level, error};

use tokio_tracer::{TraceData, TraceEvent, Tracer};

use crate::{StyledText, TabbedScrollbox, TuiWidget, tui_theme};

enum TraceUIMessage {
    Normal(TraceEvent, Vec<String>),
    ClearTab(String),
    External(TraceEvent, Vec<String>, String),
}

pub type TraceEventSender = Arc<dyn Fn(TraceEvent, Vec<String>) + Send + Sync>;

pub struct TracerWidget {
    logs: TabbedScrollbox<String>,
    form_visible: bool,
    tracer: Tracer,
    form_active: bool,
    is_focused: bool,
    // Single channel for all messages
    rx: mpsc::UnboundedReceiver<TraceUIMessage>,
    // External sources configuration - now just store StyledText directly
    source_prefixes: std::collections::HashMap<String, StyledText>,
    default_prefix: Option<StyledText>,
    borders: Borders,
    tx: mpsc::UnboundedSender<TraceUIMessage>,
}

impl TracerWidget {
    pub fn new(tracer: Tracer) -> Result<Self> {
        // Create channel for messages
        let (tx, rx) = mpsc::unbounded_channel();

        // Create tabbed scrollbox for logs
        let mut logs = TabbedScrollbox::new("Tracer Console")
            .with_borders(Borders::TOP)
            .with_wrap_indent(13)
            .with_wrap_lines(false);

        logs.focus();
        {
            let tx = tx.clone();
            tracer.set_callback(move |event, subscribers| {
                // Convert subscribers to Vec<String>
                let subscriber_names: Vec<String> =
                    subscribers.iter().map(|s| s.to_string()).collect();

                // Send message to our channel
                let _ = tx.send(TraceUIMessage::Normal(event, subscriber_names));
            })?;
        }
        // Create instance
        Ok(TracerWidget {
            logs,
            form_visible: false,
            tracer,
            form_active: false,
            is_focused: false,
            tx,
            rx,
            source_prefixes: std::collections::HashMap::new(),
            default_prefix: None,
            borders: Borders::all(),
        })
    }
    pub fn set_borders(&mut self, borders: Borders) {
        self.borders = borders;
        self.logs_mut().set_borders(borders);
    }

    pub fn with_borders(mut self, borders: Borders) -> Self {
        self.set_borders(borders);
        self
    }

    // Set default prefix directly with a StyledText
    pub fn set_default_prefix(&mut self, prefix: impl AsRef<str>) {
        self.set_default_prefix_with_style(prefix, Style::default());
    }

    pub fn set_default_prefix_with_style(&mut self, prefix: impl AsRef<str>, style: Style) {
        self.default_prefix = Some(StyledText::from_styled(prefix, style));
        self.update_wrap_indent();
    }

    fn update_wrap_indent(&mut self) {
        let mut max_indent = self.default_prefix.as_ref().map(|p| p.len()).unwrap_or(0);
        for prefix in self.source_prefixes.values() {
            let len = prefix.len();
            if len > max_indent {
                max_indent = len
            }
        }
        let wrap_indent = 13 + max_indent;
        self.logs.set_wrap_indent(wrap_indent);
    }

    // Register a new source with its styled prefix directly
    pub fn register_source(
        &mut self,
        source_id: impl Into<String>,
        prefix: impl AsRef<str>,
    ) -> TraceEventSender {
        self.register_source_with_style(source_id, prefix, Style::default())
    }
    pub fn register_source_with_style(
        &mut self,
        source_id: impl Into<String>,
        prefix: impl AsRef<str>,
        style: Style,
    ) -> TraceEventSender {
        let source_id = source_id.into();

        // Store the styled prefix directly
        self.source_prefixes
            .insert(source_id.clone(), StyledText::from_styled(prefix, style));

        self.update_wrap_indent();

        // Create a sender for this source
        let tx = self.tx.clone();
        let source_id_clone = source_id.clone();

        Arc::new(move |event, tabs| {
            let _ = tx.send(TraceUIMessage::External(
                event,
                tabs,
                source_id_clone.clone(),
            ));
        })
    }

    pub fn clear(&self, tab: String) {
        let _ = self.tx.send(TraceUIMessage::ClearTab(tab));
    }

    // Get prefix for a source ID
    fn get_prefix(&self, source_id: &str) -> StyledText {
        // Try to get a specific prefix for this source
        self.source_prefixes
            .get(source_id)
            .cloned()
            .unwrap_or_default()
    }

    // Get default prefix
    fn get_default_prefix(&self) -> StyledText {
        // Return the default prefix if set, otherwise empty
        self.default_prefix.clone().unwrap_or_default()
    }

    // Process all pending log messages
    pub fn process_messages(&mut self) {
        // Process up to a reasonable number of messages per frame
        for _ in 0..100 {
            match self.rx.try_recv() {
                Ok(TraceUIMessage::Normal(trace_event, tab_names)) => {
                    let entries = self.styled_log_message(self.get_default_prefix(), &trace_event);

                    // Optimization: If there's only one subscriber, we can avoid cloning
                    if tab_names.len() == 1 {
                        let tab = &tab_names[0];
                        // Make sure the tab exists
                        if !self.logs.tab_exists(tab) {
                            self.logs.add_tab(tab, tab);
                        }
                        // Add to the tab
                        self.logs.add_styled_to_tab(tab, entries);
                    } else {
                        // Prepare all the copies we need upfront
                        let mut copied_entries = Vec::with_capacity(tab_names.len());
                        for _ in 0..tab_names.len() - 1 {
                            copied_entries.push(entries.clone());
                        }
                        copied_entries.push(entries);

                        // Now we can add entries to each tab without more cloning
                        for tab_name in tab_names.iter() {
                            // Make sure the tab exists
                            if !self.logs.tab_exists(tab_name) {
                                self.logs.add_tab(tab_name, tab_name);
                            }

                            self.logs
                                .add_styled_to_tab(tab_name, copied_entries.remove(0));
                        }
                    }
                }

                Ok(TraceUIMessage::External(message, tab_names, source_id)) => {
                    let entries = self.styled_log_message(self.get_prefix(&source_id), &message);

                    // Optimization: If there's only one tab, we can avoid cloning
                    if tab_names.len() == 1 {
                        let tab = &tab_names[0];
                        // Make sure the tab exists
                        if !self.logs.tab_exists(tab) {
                            self.logs.add_tab(tab, tab);
                        }
                        // Add to the tab
                        self.logs.add_styled_to_tab(tab, entries);
                    } else {
                        // Prepare all the copies we need upfront
                        let mut copied_entries = Vec::with_capacity(tab_names.len());
                        for _ in 0..tab_names.len() - 1 {
                            copied_entries.push(entries.clone());
                        }
                        copied_entries.push(entries);

                        // Process multiple tabs
                        for tab_name in tab_names.iter() {
                            // Make sure the tab exists
                            if !self.logs.tab_exists(tab_name) {
                                self.logs.add_tab(tab_name, tab_name);
                            }

                            // Add to the tab (using remove to transfer ownership)
                            self.logs
                                .add_styled_to_tab(tab_name, copied_entries.remove(0));
                        }
                    }
                }
                Ok(TraceUIMessage::ClearTab(tab_name)) => {
                    if let Some(tab) = self.logs.get_tab_mut(&tab_name) {
                        tab.clear();
                    }
                }
                Err(_) => break, // No more messages
            }
        }
    }

    fn styled_log_message(
        &self,
        mut prefix: StyledText,
        trace_event: &TraceData,
    ) -> Vec<StyledText> {
        let mut result = Vec::new();

        // Split the full message by newlines
        let message_parts: Vec<&str> = trace_event.message.split('\n').collect();

        // Create the common timestamp and level prefix
        let header_prefix = prefix
            .append(
                trace_event.timestamp.format("%H").to_string(),
                Style::default().fg(tui_theme::HOUR_FG),
            )
            .append(
                trace_event.timestamp.format("%M").to_string(),
                Style::default().fg(tui_theme::MINUTE_FG),
            )
            .append(
                trace_event.timestamp.format("%S").to_string(),
                Style::default().fg(tui_theme::SEC_FG),
            )
            .append_space()
            .append(
                format!(
                    "{}{}",
                    match trace_event.level.0 {
                        Level::WARN | Level::INFO => " ",
                        _ => "",
                    },
                    trace_event.level,
                ),
                Style::default().fg(match trace_event.level.0 {
                    Level::INFO => Color::Green,
                    Level::DEBUG => Color::Cyan,
                    Level::WARN => Color::Yellow,
                    Level::ERROR => Color::Red,
                    Level::TRACE => Color::Gray,
                }),
            )
            .append_space();

        // Generate file/line info once if available
        let file_line_info = trace_event.file.as_ref().and_then(|file| {
            trace_event
                .line
                .as_ref()
                .map(|line| format!("  ({file}:{line})"))
        });
        let file_style = Style::default().fg(tui_theme::GRAY1_FG);

        let message_style = Style::default().fg(Color::White);

        // Handle first line
        let first_line = header_prefix.append(message_parts[0], message_style);

        if message_parts.len() == 1 {
            // Single line message - add file/line info to the only line
            result.push(
                first_line
                    .append_option(file_line_info, file_style)
                    .to_owned(),
            );
        } else {
            // Multiline message - add first line without file/line info

            const INDENT_SIZE: usize = 13;

            result.push(first_line.to_owned());

            // Add middle lines
            for &line in message_parts.iter().skip(1).take(message_parts.len() - 2) {
                result.push(
                    StyledText::default()
                        .append_spaces(INDENT_SIZE)
                        .append(line, message_style)
                        .to_owned(),
                );
            }

            // Add the last line with file/line info
            if let Some(last_part) = message_parts.last().filter(|_| message_parts.len() > 1) {
                result.push(
                    StyledText::default()
                        .append_spaces(INDENT_SIZE)
                        .append(*last_part, message_style)
                        .append_option(file_line_info, file_style)
                        .to_owned(),
                );
            }
        }

        result
    }
    pub fn logs_mut(&mut self) -> &mut crate::TabbedScrollbox<String> {
        &mut self.logs
    }

    // pub fn form_mut(&mut self) -> std::cell::RefMut<'_, FormWidget> {
    //     self.form.as_mut()
    // }

    pub fn logs_ref(&self) -> &crate::TabbedScrollbox<String> {
        &self.logs
    }

    // pub fn form_ref(&mut self) -> std::cell::Ref<'_, FormWidget> {
    //     self.form.as_ref()
    // }

    pub fn clear_current_tab(&mut self) -> bool {
        self.logs.clear_current_tab()
    }

    // Start editing the selected tab's configuration
    pub fn start_editing(&mut self) {
        // if self.form_visible {
        //     return;
        // }

        // // Get the name of the currently selected tab
        // let Some(tab_name) = self.logs_ref().current_tab_name().cloned() else {
        //     return;
        // };

        // // Don't allow editing of special tabs
        // if tab_name == "Silenced" || tab_name == "Dropped" {
        //     return;
        // }

        // // Find the subscriber config for this tab
        // let subscriber_index = self
        //     .config
        //     .subscribers
        //     .iter()
        //     .position(|s| s.name == tab_name);

        // if let Some(index) = subscriber_index {
        //     // Save the tab name we're editing
        //     self.editing_tab = Some(tab_name.clone());

        //     // Get the subscriber config
        //     let subscriber = self.config.subscribers[index].clone();

        //     // Convert to our form struct
        //     let subscriber_form = SubscriberConfigForm::from(subscriber);

        //     self.form_mut().set_data(&subscriber_form);

        //     // Show the form
        //     self.form_visible = true;

        //     // Focus the form
        //     self.focus_form();
        // }
    }

    // Check if form was submitted and apply changes
    pub fn check_form_status(&mut self) {
        // Check if form was submitted
        // if self.form.as_mut().reset_submit() {
        //     if let Err(e) = self.save_edited_config() {
        //         error!("Failed to save config: {}", e);
        //     }
        // }

        // // Check if form was closed
        // if self.form.as_mut().reset_closed() {
        //     if let Err(e) = self.cancel_editing() {
        //         error!("Failed to close config: {}", e);
        //     }
        // }
    }

    // Save the edited configuration and update the UI
    // fn save_edited_config(&mut self) -> Result<()> {
    // if let Some(tab_name) = &self.editing_tab {
    //     // Get form data and convert from form to trace manager type
    //     let form_data = SubscriberConfigForm::from_fields(self.form.as_ref().get_fields());
    //     let edited_config: tokio_tracer::SubscriberConfig = form_data.into();

    //     // Find the subscriber config for this tab
    //     let subscriber_index = self
    //         .config
    //         .subscribers
    //         .iter()
    //         .position(|s| s.name == *tab_name);

    //     if let Some(index) = subscriber_index {
    //         // Update the config
    //         self.config.subscribers[index] = edited_config.clone();

    //         // Remove old subscriber from tracer
    //         if let Err(e) = self.tracer.remove_subscriber(tab_name.to_string()) {
    //             error!("Failed to remove subscriber {}: {}", tab_name, e);
    //         }

    //         // Add updated subscriber to tracer
    //         if let Err(e) = self
    //             .tracer
    //             .add_subscriber(edited_config.name.clone(), edited_config.filter_set.clone())
    //         {
    //             error!(
    //                 "Failed to add updated subscriber {}: {}",
    //                 edited_config.name, e
    //             );
    //         }

    //         // If the name changed, update the tab
    //         if *tab_name != edited_config.name {
    //             // Rename the tab
    //
    //                 .add_tab(&edited_config.name, &edited_config.name);

    //             // Add confirmation message
    //             self.logs.string_add_entry_to_tab(
    //                 &edited_config.name,
    //                 format!(
    //                     "Renamed subscriber from {} to {}",
    //                     tab_name, edited_config.name
    //                 ),
    //             );

    //             // Remove old tab
    //             self.logs.remove_tab(tab_name);
    //         } else {
    //             // Add a confirmation message
    //             self.logs.string_add_entry_to_tab(
    //                 tab_name,
    //                 "Updated subscriber configuration".to_string(),
    //             );
    //         }
    //     }

    //     // Reset form and hide it
    //     self.cancel_editing()?;
    // }

    // Ok(())
    // }

    // Add a new subscriber tab
    pub fn add_subscriber(&mut self) {
        // // Create a default subscriber config with unique name
        // let new_subscriber = tokio_tracer::SubscriberConfig {
        //     name: format!("Subscriber_{}", self.config.subscribers.len() + 1),
        //     ..Default::default()
        // };

        // // Add to config
        // self.config.subscribers.push(new_subscriber.clone());

        // // Add tab
        //     .as_mut()
        //     .add_tab(&new_subscriber.name, &new_subscriber.name);

        // // Add the subscriber to the tracer
        // if let Err(e) = self.tracer.add_subscriber(
        //     new_subscriber.name.clone(),
        //     new_subscriber.filter_set.clone(),
        // ) {
        //     error!(
        //         "Failed to add new subscriber {}: {}",
        //         new_subscriber.name, e
        //     );
        // }

        // // Select the new tab
        // self.logs.select_string_tab(&new_subscriber.name);
    }

    // Delete the current subscriber tab
    pub fn delete_current_subscriber(&mut self) -> Result<()> {
        // // Get the current tab
        // if let Some(tab_name) = self.logs.current_tab_name().cloned() {
        //     // Don't delete if we're editing
        //     if self.editing_tab.is_some() {
        //         return Ok(());
        //     }

        //     // Don't delete special tabs
        //     if tab_name == "Silenced" || tab_name == "Dropped" {
        //         return Ok(());
        //     }

        //     // Find the subscriber
        //     let subscriber_index = self
        //         .config
        //         .subscribers
        //         .iter()
        //         .position(|s| s.name == tab_name);

        //     if let Some(index) = subscriber_index {
        //         // Remove from config
        //         self.config.subscribers.remove(index);

        //         // Remove from tracer
        //         if let Err(e) = self.tracer.remove_subscriber(tab_name.to_string()) {
        //             error!("Failed to remove subscriber {}: {}", tab_name, e);
        //         }

        //         // Remove tab
        //         self.logs.remove_tab(&tab_name);

        //         return Ok(());
        //     }
        // }

        anyhow::bail!("Could not find subscriber to delete")
    }

    fn focus_form(&mut self) {
        self.form_active = true;
        self.logs_mut().unfocus();
        // self.form_mut().focus();
    }

    fn focus_logs(&mut self) {
        self.form_active = false;
        // self.form_mut().unfocus();
        self.logs_mut().focus();
    }

    // Get statistics about messages
    pub fn get_stats(&self) -> (u64, u64, u64) {
        (
            self.tracer.get_captured_count(),
            self.tracer.get_silenced_count(),
            self.tracer.get_dropped_count(),
        )
    }

    // Clear all statistics
    pub fn clear_stats(&mut self) {
        if let Err(e) = self.tracer.clear_stats() {
            error!("Failed to clear stats: {}", e);
        }
    }
}

impl TuiWidget for TracerWidget {
    fn need_draw(&self) -> bool {
        self.logs.need_draw()
    }
    fn preprocess(&mut self) {
        // Process any pending messages
        self.process_messages();
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        // Check form status
        self.check_form_status();
        // Split the screen depending on whether form is visible
        if self.form_visible {
            // Create a horizontal split
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            // Render logs on the left
            self.logs.draw(chunks[0], buf);

            // Render form on the right
            // self.form.as_mut().render(chunks[1], buf);
        } else {
            // Render just the logs panel using the full area
            self.logs.draw(area, buf);
        }
    }

    fn mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        self.logs_mut().mouse_event(mouse)
    }

    fn key_event(&mut self, key: KeyEvent) -> bool {
        let mut handled = true;

        match key.code {
            // Edit current tab configuration
            KeyCode::Char('e')
                if !self.form_visible && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.start_editing();
            }

            // Add new subscriber
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.add_subscriber();
            }

            // Delete current subscriber
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = self.delete_current_subscriber();
            }

            // Toggle focus between panels
            KeyCode::Tab if self.form_visible => {
                self.form_active = !self.form_active;
                if self.form_active {
                    self.focus_form();
                } else {
                    self.focus_logs();
                }
            }

            // Handle other key events based on active panel
            _ => {
                // if self.form_active {
                // handled = self.form_mut().handle_key_event(key);
                // } else {
                handled = self.logs_mut().key_event(key);
                // }
            }
        }

        handled
    }

    fn focus(&mut self) {
        self.is_focused = true;
        if self.form_active {
            self.focus_form();
        } else {
            self.focus_logs();
        }
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
        self.logs.unfocus();
        // self.form.as_mut().unfocus();
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}
