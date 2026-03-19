use std::path::PathBuf;

pub struct FileResult {
    pub name: String,
    pub path: PathBuf,
}

/// Search for files matching a query in common directories
pub fn search_files(query: &str, max_results: usize) -> Vec<FileResult> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    let search_dirs = [
        dirs::home_dir().unwrap_or_default(),
        dirs::desktop_dir().unwrap_or_default(),
        dirs::document_dir().unwrap_or_default(),
        dirs::download_dir().unwrap_or_default(),
    ];

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.to_lowercase().contains(&query_lower) {
                    results.push(FileResult {
                        name: name.clone(),
                        path,
                    });
                    if results.len() >= max_results {
                        return results;
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    results.truncate(max_results);
    results
}
