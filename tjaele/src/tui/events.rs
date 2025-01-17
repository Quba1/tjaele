use std::time::Duration;

use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::{
    self,
    event::{Event as CrosstermEvent, KeyEvent},
};
use tokio::sync::mpsc;

/// Terminal events.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// Terminal tick.
    Tick,
    /// Key press.
    Key(KeyEvent),
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: f64) -> Self {
        let tick_rate = Duration::from_secs_f64(tick_rate);

        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);

            loop {
                let tick_delay = tick.tick();
                let crossterm_event = reader.next().fuse();

                tokio::select! {
                  _ = sender.closed() => {
                    break;
                  }
                  _ = tick_delay => {
                    sender.send(Event::Tick).unwrap();
                  }
                  Some(Ok(evt)) = crossterm_event => {
                    if let CrosstermEvent::Key(key) = evt {
                      if key.kind == crossterm::event::KeyEventKind::Press {
                        sender.send(Event::Key(key)).unwrap();
                      }
                    }
                  }
                };
            }
        });

        Self { receiver }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub async fn next(&mut self) -> Result<Event> {
        self.receiver.recv().await.context("Channel has been closed")
    }
}
