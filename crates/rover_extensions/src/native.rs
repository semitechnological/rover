use anyhow::{Context as _, Result};
use rover_extension_api::{CommandOutput, ExtensionCommandInfo};
use std::path::Path;
use std::sync::Arc;

/// C ABI function signatures for native extensions.
///
/// Native extensions must export:
/// - `rover_init() -> *const c_char` — returns JSON array of ExtensionCommandInfo
/// - `rover_run_command(cmd: *const c_char, args: *const c_char) -> *const c_char` — returns JSON CommandOutput
type InitFn = unsafe extern "C" fn() -> *const std::ffi::c_char;
type RunCommandFn =
    unsafe extern "C" fn(*const std::ffi::c_char, *const std::ffi::c_char) -> *const std::ffi::c_char;

/// A loaded native extension (shared library / dylib).
pub struct NativeExtensionRuntime {
    _lib: libloading::Library,
    init_fn: InitFn,
    run_fn: RunCommandFn,
}

// SAFETY: The native extension ABI is designed to be thread-safe.
// Extension authors must ensure their exported functions are reentrant.
unsafe impl Send for NativeExtensionRuntime {}
unsafe impl Sync for NativeExtensionRuntime {}

impl NativeExtensionRuntime {
    /// Load a native extension from a dylib path.
    ///
    /// # Safety
    /// Loading arbitrary shared libraries is inherently unsafe. Only load
    /// extensions from trusted sources.
    pub unsafe fn load(dylib_path: &Path) -> Result<Self> {
        let lib = unsafe {
            libloading::Library::new(dylib_path)
                .with_context(|| format!("loading native extension from {:?}", dylib_path))?
        };

        let init_fn: InitFn = unsafe {
            *lib.get::<InitFn>(b"rover_init\0")
                .context("native extension missing `rover_init` symbol")?
        };

        let run_fn: RunCommandFn = unsafe {
            *lib.get::<RunCommandFn>(b"rover_run_command\0")
                .context("native extension missing `rover_run_command` symbol")?
        };

        Ok(Self {
            _lib: lib,
            init_fn,
            run_fn,
        })
    }

    /// Call `rover_init` to get metadata about the extension's commands.
    pub fn init(&self) -> Result<Vec<ExtensionCommandInfo>> {
        let ptr = unsafe { (self.init_fn)() };
        if ptr.is_null() {
            return Ok(Vec::new());
        }
        let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_str()
            .context("rover_init returned invalid UTF-8")?;
        let commands: Vec<ExtensionCommandInfo> =
            serde_json::from_str(json).context("parsing rover_init JSON")?;
        Ok(commands)
    }

    /// Call `rover_run_command` with a command name and args.
    pub fn run_command(&self, command: &str, args: &str) -> Result<CommandOutput> {
        let cmd_c = std::ffi::CString::new(command).context("invalid command name")?;
        let args_c = std::ffi::CString::new(args).context("invalid args")?;
        let ptr = unsafe { (self.run_fn)(cmd_c.as_ptr(), args_c.as_ptr()) };
        if ptr.is_null() {
            anyhow::bail!("rover_run_command returned null");
        }
        let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_str()
            .context("rover_run_command returned invalid UTF-8")?;
        let output: CommandOutput =
            serde_json::from_str(json).context("parsing rover_run_command JSON")?;
        Ok(output)
    }
}

pub type NativeExtensionRuntimeRef = Arc<NativeExtensionRuntime>;
