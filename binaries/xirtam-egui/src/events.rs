use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use notify_rust::Notification;
use rodio::{Decoder, OutputStream, Sink};
use tokio::sync::mpsc::Sender;
use xirtam_client::internal::{DmDirection, Event, InternalClient};

use crate::promises::flatten_rpc;

pub async fn event_loop(rpc: InternalClient, event_tx: Sender<Event>, focused: Arc<AtomicBool>) {
    let focused_task = focused.clone();
    let mut max_notified = 0;
    loop {
        match rpc.next_event().await {
            Ok(event) => {
                if let Event::DmUpdated { peer } = &event
                    && !focused_task.load(Ordering::Relaxed)
                {
                    match flatten_rpc(rpc.dm_history(peer.clone(), None, None, 1).await) {
                        Ok(messages) => {
                            if let Some(message) = messages.last()
                                && matches!(message.direction, DmDirection::Incoming)
                                && message.received_at.unwrap_or_default().0 > max_notified
                            {
                                max_notified = message.received_at.unwrap_or_default().0;
                                let body = String::from_utf8_lossy(&message.body).to_string();
                                if let Err(err) = Notification::new()
                                    .summary(&format!("Message from {}", message.sender))
                                    .body(&body)
                                    .show()
                                {
                                    tracing::warn!(error = %err, "notification error");
                                }
                                play_notification_sound();
                            }
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "failed to fetch latest message");
                        }
                    }
                }
                if event_tx.send(event).await.is_err() {
                    break;
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "event loop error");
            }
        }
    }
}

fn play_notification_sound() {
    let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
        tracing::warn!("failed to open audio output stream");
        return;
    };
    let bytes = include_bytes!("sounds/notification.mp3");
    let cursor = std::io::Cursor::new(bytes);
    let Ok(source) = Decoder::new(cursor) else {
        tracing::warn!("failed to decode notification sound");
        return;
    };
    let Ok(sink) = Sink::try_new(&stream_handle) else {
        tracing::warn!("failed to create audio sink");
        return;
    };
    sink.append(source);
    sink.detach();
}
