use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum NavEvent {
    LoadStarted { url: String },
    LoadCommitted { url: String },
    LoadFinished { url: String, title: String },
    LoadFailed { error: String },
    TitleChanged { title: String },
    NewTabRequested { url: String },
    DownloadStarted { suggested_filename: String, destination: String },
    FindResult { found: bool, match_count: u32 },
}

pub type NavEventQueue = Arc<Mutex<Vec<NavEvent>>>;

pub fn new_queue() -> NavEventQueue {
    Arc::new(Mutex::new(Vec::new()))
}
