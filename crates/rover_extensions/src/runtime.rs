use anyhow::{Context as _, Result};
use rover_extension_api::{CommandOutput, ExtensionCommandInfo};
use std::path::Path;
use std::sync::{Arc, Mutex};
use wasmtime::*;

/// Host state passed to WASM instances via wasmtime's Store.
struct HostState {
    /// Memory exported by the WASM module.
    memory: Option<Memory>,
    /// Allocator exported by the WASM module.
    alloc_fn: Option<TypedFunc<i32, i32>>,
}

/// A running WASM extension instance.
pub struct ExtensionRuntime {
    store: Mutex<Store<HostState>>,
    instance: Instance,
}

impl ExtensionRuntime {
    /// Load and instantiate a WASM extension from disk.
    pub fn load(wasm_path: &Path) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)
            .with_context(|| format!("loading WASM module from {:?}", wasm_path))?;

        let mut linker = Linker::new(&engine);

        // Host import: host_log(level: i32, msg_ptr: i32, msg_len: i32)
        linker.func_wrap("env", "host_log", |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| {
            if let Some(memory) = caller.data().memory {
                let data = memory.data(&caller);
                if let Some(slice) = data.get(ptr as usize..(ptr as usize + len as usize)) {
                    let msg = String::from_utf8_lossy(slice);
                    match level {
                        0 => log::error!("[ext] {msg}"),
                        1 => log::warn!("[ext] {msg}"),
                        2 => log::info!("[ext] {msg}"),
                        _ => log::debug!("[ext] {msg}"),
                    }
                }
            }
        })?;

        // Host import: host_clipboard_read() -> i32 (ptr to JSON string in WASM memory)
        linker.func_wrap("env", "host_clipboard_read", |_caller: Caller<'_, HostState>| -> i32 {
            // Returns 0 (null) — clipboard access requires main thread coordination
            // Extensions that need clipboard should use the CommandOutput flow instead.
            0_i32
        })?;

        // Host import: host_clipboard_write(ptr: i32, len: i32)
        linker.func_wrap("env", "host_clipboard_write", |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            if let Some(memory) = caller.data().memory {
                let data = memory.data(&caller);
                if let Some(slice) = data.get(ptr as usize..(ptr as usize + len as usize)) {
                    let text = String::from_utf8_lossy(slice).to_string();
                    // Best-effort clipboard write via pbcopy
                    let _ = std::process::Command::new("pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(stdin) = child.stdin.as_mut() {
                                stdin.write_all(text.as_bytes())?;
                            }
                            child.wait()
                        });
                }
            }
        })?;

        let mut store = Store::new(&engine, HostState {
            memory: None,
            alloc_fn: None,
        });

        let instance = linker
            .instantiate(&mut store, &module)
            .context("instantiating WASM module")?;

        // Capture memory export
        if let Some(memory) = instance.get_memory(&mut store, "memory") {
            store.data_mut().memory = Some(memory);
        }

        // Capture alloc function if available
        if let Ok(alloc) = instance.get_typed_func::<i32, i32>(&mut store, "rover_alloc") {
            store.data_mut().alloc_fn = Some(alloc);
        }

        Ok(Self {
            store: Mutex::new(store),
            instance,
        })
    }

    /// Call `rover_init` to get metadata about the extension's commands.
    /// Returns a list of commands the extension provides.
    pub fn init(&self) -> Result<Vec<ExtensionCommandInfo>> {
        let mut store = self.store.lock().unwrap();
        let init_fn = self
            .instance
            .get_typed_func::<(), i32>(&mut *store, "rover_init")
            .context("WASM module missing `rover_init` export")?;

        let ptr = init_fn.call(&mut *store, ()).context("calling rover_init")?;
        let json = self.read_string(&mut store, ptr)?;
        let commands: Vec<ExtensionCommandInfo> =
            serde_json::from_str(&json).context("parsing rover_init JSON")?;
        Ok(commands)
    }

    /// Call `rover_run_command` with a command name and args, returning the output.
    pub fn run_command(&self, command: &str, args: &str) -> Result<CommandOutput> {
        let mut store = self.store.lock().unwrap();

        let cmd_ptr = self.write_string(&mut store, command)?;
        let args_ptr = self.write_string(&mut store, args)?;

        let run_fn = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut *store, "rover_run_command")
            .context("WASM module missing `rover_run_command` export")?;

        let result_ptr = run_fn
            .call(&mut *store, (cmd_ptr, args_ptr))
            .context("calling rover_run_command")?;

        let json = self.read_string(&mut store, result_ptr)?;
        let output: CommandOutput =
            serde_json::from_str(&json).context("parsing rover_run_command JSON")?;
        Ok(output)
    }

    /// Read a NUL-terminated string from WASM memory at the given pointer.
    fn read_string(&self, store: &mut Store<HostState>, ptr: i32) -> Result<String> {
        let memory = store
            .data()
            .memory
            .context("WASM module has no memory export")?;
        let data = memory.data(&*store);
        let start = ptr as usize;
        let mut end = start;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }
        Ok(String::from_utf8_lossy(&data[start..end]).to_string())
    }

    /// Write a string into WASM memory, returning the pointer.
    /// Uses the `rover_alloc` export if available, otherwise errors.
    fn write_string(&self, store: &mut Store<HostState>, s: &str) -> Result<i32> {
        let bytes = s.as_bytes();
        let len = bytes.len() as i32 + 1; // +1 for NUL terminator

        let alloc = store
            .data()
            .alloc_fn
            .clone()
            .context("WASM module has no `rover_alloc` export")?;

        let ptr = alloc.call(&mut *store, len).context("calling rover_alloc")?;

        let memory = store
            .data()
            .memory
            .context("WASM module has no memory export")?;
        let data = memory.data_mut(&mut *store);
        let dest = &mut data[ptr as usize..ptr as usize + len as usize];
        dest[..bytes.len()].copy_from_slice(bytes);
        dest[bytes.len()] = 0; // NUL terminate

        Ok(ptr)
    }
}

/// Thread-safe handle to an extension runtime.
pub type ExtensionRuntimeRef = Arc<ExtensionRuntime>;

/// Load all extensions that have WASM libraries and return their runtimes
/// along with the commands they provide.
pub fn load_extension_runtimes(
    extensions: &[crate::LoadedExtension],
) -> Vec<(String, ExtensionRuntimeRef, Vec<ExtensionCommandInfo>)> {
    let mut results = Vec::new();

    for ext in extensions {
        let lib_entry = match &ext.manifest.lib {
            Some(lib) => lib,
            None => continue,
        };

        let wasm_path = match &lib_entry.path {
            Some(p) => ext.root.join(p),
            None => ext.root.join("target/wasm32-wasip1/release")
                .join(format!("{}.wasm", ext.manifest.id)),
        };

        if !wasm_path.exists() {
            log::warn!(
                "Extension '{}': WASM file not found at {:?}",
                ext.manifest.name,
                wasm_path
            );
            continue;
        }

        match ExtensionRuntime::load(&wasm_path) {
            Ok(runtime) => {
                match runtime.init() {
                    Ok(commands) => {
                        log::info!(
                            "Loaded extension '{}' with {} commands",
                            ext.manifest.name,
                            commands.len()
                        );
                        results.push((
                            ext.manifest.id.clone(),
                            Arc::new(runtime),
                            commands,
                        ));
                    }
                    Err(e) => {
                        log::warn!(
                            "Extension '{}' init failed: {}",
                            ext.manifest.name,
                            e
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to load WASM for extension '{}': {}",
                    ext.manifest.name,
                    e
                );
            }
        }
    }

    results
}
