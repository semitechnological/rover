use crate::ExtensionManifest;
use anyhow::{Context as _, Result};
use rover_theme::{ThemeFamilyContent, ThemeRegistry};
use std::path::{Path, PathBuf};

/// A loaded extension with its manifest and root directory.
#[derive(Debug)]
pub struct LoadedExtension {
    pub manifest: ExtensionManifest,
    pub root: PathBuf,
}

/// Scan the extensions directory and load all extensions.
pub fn load_extensions() -> Result<Vec<LoadedExtension>> {
    let ext_dir = extensions_dir();
    if !ext_dir.exists() {
        return Ok(Vec::new());
    }

    let mut loaded = Vec::new();
    for entry in std::fs::read_dir(&ext_dir)
        .with_context(|| format!("reading extensions dir {:?}", ext_dir))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        match load_extension(&path) {
            Ok(ext) => loaded.push(ext),
            Err(e) => log::warn!("Failed to load extension at {:?}: {}", path, e),
        }
    }

    Ok(loaded)
}

fn load_extension(root: &Path) -> Result<LoadedExtension> {
    let manifest_path = root.join("extension.toml");
    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {:?}", manifest_path))?;
    let manifest: ExtensionManifest =
        toml::from_str(&content).with_context(|| format!("parsing {:?}", manifest_path))?;

    Ok(LoadedExtension {
        manifest,
        root: root.to_path_buf(),
    })
}

/// Register themes from loaded extensions into the theme registry.
pub fn register_extension_themes(extensions: &[LoadedExtension], registry: &mut ThemeRegistry) {
    for ext in extensions {
        for theme_path in &ext.manifest.themes {
            let full_path = ext.root.join(theme_path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    if let Ok(family) = serde_json::from_str::<ThemeFamilyContent>(&content) {
                        registry.load_family(&family);
                        log::info!(
                            "Loaded theme family from extension '{}': {:?}",
                            ext.manifest.name,
                            theme_path
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Failed to read theme {:?} from extension '{}': {}",
                        theme_path,
                        ext.manifest.name,
                        e
                    );
                }
            }
        }
    }
}

pub fn extensions_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rover")
        .join("extensions")
}
