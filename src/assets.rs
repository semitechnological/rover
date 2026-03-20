pub fn assets_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    for ancestor in exe.parent().unwrap_or(std::path::Path::new(".")).ancestors() {
        let candidate = ancestor.join("assets");
        if candidate.exists() {
            return candidate;
        }
    }
    std::path::PathBuf::from("assets")
}
