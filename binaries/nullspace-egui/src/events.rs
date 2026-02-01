use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::sync::mpsc::{Receiver, Sender as StdSender};
use tokio::sync::mpsc::Sender as TokioSender;
use nullspace_client::internal::{Event, InternalClient};

use crate::notify::show_notification;

pub async fn event_loop(
    rpc: InternalClient,
    event_tx: TokioSender<Event>,
    focused: Arc<AtomicBool>,
    audio_tx: StdSender<Vec<u8>>,
) {
    let focused_task = focused.clone();
    let mut max_notified = 0;
    loop {
        match rpc.next_event().await {
            Ok(event) => {
                show_notification(&event, &rpc, &focused_task, &audio_tx, &mut max_notified)
                    .await;
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

pub fn spawn_audio_thread() -> StdSender<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || audio_thread(rx));
    tx
}

fn audio_thread(rx: Receiver<Vec<u8>>) {
    let Ok(stream) = OutputStreamBuilder::open_default_stream() else {
        tracing::warn!("failed to open audio output stream");
        return;
    };
    for bytes in rx {
        let cursor = std::io::Cursor::new(bytes);
        let Ok(source) = Decoder::new(cursor) else {
            tracing::warn!("failed to decode notification sound");
            continue;
        };
        let sink = Sink::connect_new(stream.mixer());
        sink.append(source);
        sink.detach();
    }
}
