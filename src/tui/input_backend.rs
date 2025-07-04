// tokio-tui/src/tui/input_backend.rs
use std::time::Duration;

use anyhow::{Result, anyhow};
use crossterm::event::{
    Event as CrosstermEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub type InputEvents = (Option<Vec<KeyEvent>>, Option<Vec<MouseEvent>>);
pub enum InputEvent {
    Mouse(MouseEvent),
    Key(KeyEvent),
}

#[derive(Clone, Copy, Debug)]
pub struct InputBackendOpts {
    key_buffer: usize,
    mouse_buffer: usize,
    tick_rate: Duration,
    flush_cap: usize,
}
impl Default for InputBackendOpts {
    fn default() -> Self {
        Self {
            key_buffer: 5,
            mouse_buffer: 8,
            tick_rate: Duration::from_millis(75),
            flush_cap: 512,
        }
    }
}

// Threaded key handler (captures keys in a separate tokio thread)
pub struct InputHandler {
    key_rx: UnboundedReceiver<InputEvents>,
    task_handle: Option<JoinHandle<JoinHandle<()>>>,
    cancel: CancellationToken,
    backend: Option<InputBackend>,

    opts: InputBackendOpts,
}

impl InputHandler {
    pub fn new() -> Self {
        Self::with_opts(InputBackendOpts::default())
    }
    pub fn with_opts(opts: InputBackendOpts) -> Self {
        let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();

        let cancel = CancellationToken::new();
        Self {
            key_rx,
            task_handle: None,
            backend: Some(InputBackend::new(opts, key_tx, cancel.clone())),
            opts,
            cancel,
        }
    }
    pub fn is_running(&self) -> bool {
        !self.cancel.is_cancelled()
    }

    pub fn start(&mut self) -> Result<()> {
        let backend = self
            .backend
            .take()
            .ok_or(anyhow!("Key handler already started"))?;

        self.task_handle = Some(tokio::task::spawn_blocking(move || {
            tokio::spawn(backend.run())
        }));

        Ok(())
    }

    pub fn stop(&mut self) {
        self.cancel.cancel();

        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }

    pub fn flush_events(&mut self) -> Option<InputEvents> {
        if !self.is_running() {
            return None;
        }

        let mut key_events: Vec<KeyEvent> = Vec::new();
        let mut mouse_events: Vec<MouseEvent> = Vec::new();

        // pull **everything** that is ready right now
        while let Ok((k, m)) = self.key_rx.try_recv() {
            if let Some(k) = k {
                key_events.extend(k);
            }
            if let Some(m) = m {
                mouse_events.extend(m);
            }
            // optional hard cap so we never stall a frame forever
            if key_events.len() + mouse_events.len() > self.opts.flush_cap {
                break;
            }
        }
        match (key_events.len(), mouse_events.len()) {
            (0, 0) => None,
            (_, 0) => Some((Some(key_events), None)),
            (0, _) => Some((None, Some(mouse_events))),
            (_, _) => Some((Some(key_events), Some(mouse_events))),
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

struct InputBackend {
    tx: UnboundedSender<InputEvents>,
    key_buffer: Vec<KeyEvent>,
    mouse_buffer: Vec<MouseEvent>,
    cancel: CancellationToken,
    event_reader: EventStream,
    interval: tokio::time::Interval,
    scroll_delta: i32,
    backspace_cnt: usize,
    opts: InputBackendOpts,
}

impl InputBackend {
    pub fn new(
        opts: InputBackendOpts,
        tx: UnboundedSender<InputEvents>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            tx,
            key_buffer: Vec::with_capacity(opts.key_buffer),
            mouse_buffer: Vec::with_capacity(opts.mouse_buffer),
            cancel,
            event_reader: EventStream::new(),
            interval: tokio::time::interval(opts.tick_rate),
            scroll_delta: 0,  // +N down, -N up
            backspace_cnt: 0, // pending back-spaces
            opts,
        }
    }

    /// Push the current buffers through the channel in one packet.
    fn flush(&mut self) {
        if self.key_buffer.is_empty() && self.mouse_buffer.is_empty() {
            return;
        }

        let keys = if self.key_buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.key_buffer))
        };
        let mouses = if self.mouse_buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.mouse_buffer))
        };

        let _ = self.tx.send((keys, mouses));
    }

    /// Main loop – runs in a spawned async task
    pub async fn run(mut self) {
        loop {
            if self.cancel.is_cancelled() {
                break;
            }

            tokio::select! {
                maybe_event = self.event_reader.next().fuse() => {
                    if let Some(Ok(evt)) = maybe_event {
                        match evt {
                            /* ---------- Mouse ---------- */
                            CrosstermEvent::Mouse(mev) => {
                                match mev.kind {
                                    MouseEventKind::ScrollUp   => self.scroll_delta -= 1,
                                    MouseEventKind::ScrollDown => self.scroll_delta += 1,
                                    MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
                                        // horizontal wheel straight through
                                        self.mouse_buffer.push(mev);
                                    }
                                    _ => {
                                        // clicks / moves
                                        self.mouse_buffer.push(mev);
                                    }
                                }

                                if self.mouse_buffer.len() >= self.opts.mouse_buffer{
                                    self.flush();
                                }
                            }

                            /* ---------- Keys ---------- */
                            CrosstermEvent::Key(kev) if kev.kind == KeyEventKind::Press => {
                                match kev.code {
                                    KeyCode::Backspace => {
                                        self.backspace_cnt += 1;      // coalesce
                                    }
                                    _ => {
                                        // materialise pending back-spaces first
                                        if self.backspace_cnt > 0 {
                                            self.push_backspaces();
                                        }

                                        self.key_buffer.push(kev);
                                        if self.key_buffer.len() >= self.opts.key_buffer{
                                            self.flush();
                                        }
                                    }
                                }
                            }
                            _ => {} // ignore key releases etc.
                        }
                    }
                }

                /* ---------- Tick ---------- */
                _ = self.interval.tick() => {
                    /*          materialise wheel delta          */
                    if self.scroll_delta != 0 {
                        use MouseEventKind::{ScrollDown, ScrollUp};
                        let dir  = if self.scroll_delta > 0 { ScrollDown } else { ScrollUp };
                        let reps = self.scroll_delta.unsigned_abs();

                        for _ in 0..reps {
                            self.mouse_buffer.push(MouseEvent {
                                kind:       dir,
                                column:     0,
                                row:        0,
                                modifiers:  KeyModifiers::NONE,
                            });
                            if self.mouse_buffer.len() >= self.opts.key_buffer {
                                self.flush();
                            }
                        }
                        self.scroll_delta = 0;
                    }

                    /*          materialise back-spaces          */
                    if self.backspace_cnt > 0 {
                        self.push_backspaces();
                    }

                    /*          ship it          */
                    self.flush();
                }
            }
        }
    }

    /// Turn the pending back-space count into individual events.
    fn push_backspaces(&mut self) {
        while self.backspace_cnt > 0 {
            self.key_buffer
                .push(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            self.backspace_cnt -= 1;

            if self.key_buffer.len() >= self.opts.key_buffer {
                break; // leave the rest for the next flush
            }
        }
    }
}
