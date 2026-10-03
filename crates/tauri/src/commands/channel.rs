use tauri::ipc::Channel;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub(crate) fn channel_send<T>(channel: &Channel<T>, value: T) -> Result<(), tauri::Error>
where
    T: serde::Serialize + Clone,
{
    channel
        .send(value)
        .map_err(|e| tauri::Error::Anyhow(e.into()))
}

/// Bridges a queue to a Tauri channel from a background task, so blocking git
/// work can keep producing from its own threads.
///
/// The sender goes to the producer and the handle to the caller: awaiting the
/// handle after the producer resolves guarantees the last value has landed,
/// because the forwarding loop only ends once every sender is gone. That
/// ordering is what keeps a stream's events from arriving after the result
/// they precede.
pub(crate) fn forward_channel<T>(
    channel: Channel<T>,
) -> (UnboundedSender<T>, tauri::async_runtime::JoinHandle<()>)
where
    T: serde::Serialize + Clone + Send + 'static,
{
    let (sender, mut receiver) = unbounded_channel();
    let forward = tauri::async_runtime::spawn(async move {
        while let Some(value) = receiver.recv().await {
            if channel_send(&channel, value).is_err() {
                // The frontend is gone. The producer keeps running; its events
                // just have nowhere left to go.
                break;
            }
        }
    });
    (sender, forward)
}
