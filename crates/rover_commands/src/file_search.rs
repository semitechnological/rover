use std::path::PathBuf;

pub struct FileResult {
    pub name: String,
    pub path: PathBuf,
}

/// Search for files matching a query using Spotlight (`mdfind`), falling back
/// to a shallow directory scan of common user folders if mdfind is unavailable.
pub fn search_files(query: &str, max_results: usize) -> Vec<FileResult> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    search_spotlight(query, max_results).unwrap_or_else(|| search_dirs(query, max_results))
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
