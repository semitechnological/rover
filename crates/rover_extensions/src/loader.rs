use crate::ExtensionManifest;
use anyhow::{Context as _, Result};
use rover_theme::{ThemeFamilyContent, ThemeRegistry};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

pub fn extensions_inbox_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rover")
        .join("extensions-inbox")
}

/// Install a ZIP-packaged extension into Rover's extensions directory.
///
/// The ZIP must contain an `extension.toml` at its root.
pub fn install_extension_zip(zip_path: &Path) -> Result<LoadedExtension> {
    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("opening extension zip {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("reading zip archive {}", zip_path.display()))?;

    let staging_root = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rover")
        .join("extension-install");
    std::fs::create_dir_all(&staging_root)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let staging_dir = staging_root.join(format!("staging-{stamp}"));
    std::fs::create_dir_all(&staging_dir)?;

    for i in 0..archive.len() {
        let mut item = archive.by_index(i)?;
        let enclosed = match item.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let out_path = staging_dir.join(enclosed);
        if item.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut item, &mut out)?;
    }

    let manifest_path = staging_dir.join("extension.toml");
    if !manifest_path.exists() {
        anyhow::bail!(
            "ZIP does not contain extension.toml at root: {}",
            zip_path.display()
        );
    }
    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest: ExtensionManifest = toml::from_str(&content)
        .with_context(|| format!("parsing {}", manifest_path.display()))?;

    let dest_root = extensions_dir().join(&manifest.id);
    if dest_root.exists() {
        std::fs::remove_dir_all(&dest_root)
            .with_context(|| format!("removing existing extension {}", dest_root.display()))?;
    }
    if let Some(parent) = dest_root.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&staging_dir, &dest_root).with_context(|| {
        format!(
            "moving installed extension from {} to {}",
            staging_dir.display(),
            dest_root.display()
        )
    })?;

    Ok(LoadedExtension {
        manifest,
        root: dest_root,
    })
}
