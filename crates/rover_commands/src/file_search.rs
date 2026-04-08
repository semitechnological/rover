use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct FileResult {
    pub name: String,
    pub path: PathBuf,
}

struct SearchCache {
    query: String,
    at: Instant,
    results: Vec<FileResult>,
}

/// Search for files matching a query using Spotlight (`mdfind`), falling back
/// to a shallow directory scan of common user folders if mdfind is unavailable.
pub fn search_files(query: &str, max_results: usize) -> Vec<FileResult> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    if query.len() < 2 {
        return Vec::new();
    }

    static CACHE: OnceLock<Mutex<Option<SearchCache>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(entry) = guard.as_ref() {
            if entry.query == query && entry.at.elapsed() < Duration::from_millis(250) {
                return entry.results.iter().take(max_results).cloned().collect();
            }
        }
    }

    let results = search_spotlight(query, max_results).unwrap_or_else(|| search_dirs(query, max_results));
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(SearchCache {
            query: query.to_string(),
            at: Instant::now(),
            results: results.clone(),
        });
    }
    results
}

/// Build a lightweight local file index from common user folders.
/// Used as a fast fallback corpus for semantic file search.
pub fn build_local_index(max_results: usize) -> Vec<FileResult> {
    let mut results = Vec::new();
    let dirs = [
        dirs::home_dir().unwrap_or_default(),
        dirs::desktop_dir().unwrap_or_default(),
        dirs::document_dir().unwrap_or_default(),
        dirs::download_dir().unwrap_or_default(),
    ];
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if results.len() >= max_results {
                    break;
                }
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if !name.is_empty() {
                    results.push(FileResult { name, path });
                }
            }
        }
        if results.len() >= max_results {
            break;
        }
    }
    results
}

fn search_spotlight(query: &str, max_results: usize) -> Option<Vec<FileResult>> {
    let output = std::process::Command::new("mdfind")
        .arg("-name")
        .arg(query)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let query_lower = query.to_lowercase();

    // Gather more than needed so we can sort and trim cleanly.
    let mut results: Vec<FileResult> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .take(max_results * 4)
        .map(|line| {
            let path = PathBuf::from(line);
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            FileResult { name, path }
        })
        .collect();

    // Exact match first, then prefix, then alphabetical.
    results.sort_by(|a, b| {
        let al = a.name.to_lowercase();
        let bl = b.name.to_lowercase();
        let a_exact = al == query_lower;
        let b_exact = bl == query_lower;
        let a_prefix = al.starts_with(&query_lower);
        let b_prefix = bl.starts_with(&query_lower);
        b_exact
            .cmp(&a_exact)
            .then(b_prefix.cmp(&a_prefix))
            .then(al.cmp(&bl))
    });
    results.truncate(max_results);
    Some(results)
}

fn search_dirs(query: &str, max_results: usize) -> Vec<FileResult> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    let dirs = [
        dirs::home_dir().unwrap_or_default(),
        dirs::desktop_dir().unwrap_or_default(),
        dirs::document_dir().unwrap_or_default(),
        dirs::download_dir().unwrap_or_default(),
    ];

    for dir in &dirs {
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
                    results.push(FileResult { name, path });
                }
            }
        }
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    results.truncate(max_results);
    results
}
