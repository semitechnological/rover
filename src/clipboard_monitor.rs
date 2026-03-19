use gpui::*;
use rover_db::RoverDb;
use std::sync::Arc;
use std::time::Duration;

/// Polls the system clipboard every 500ms and stores new entries in the DB.
pub fn start(db: Arc<RoverDb>, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut last_content: Option<String> = None;
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;

            let content = cx
                .update(|cx: &mut App| {
                    cx.read_from_clipboard()
                        .and_then(|item: ClipboardItem| item.text())
                })
                .ok()
                .flatten();

            if let Some(text) = content {
                if !text.trim().is_empty() && last_content.as_deref() != Some(&text) {
                    last_content = Some(text.clone());
                    let _ = db.add_clipboard_entry(&text);
                }
            }
        }
    })
    .detach();
}
