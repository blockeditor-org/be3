use std::sync::Mutex;

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);
static PREVIOUS_PANIC: Mutex<Option<String>> = Mutex::new(None);

#[cfg(not(target_arch = "wasm32"))]
static REPORT_FILE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// Puts be-host's hook in place: a panic outside `be_wasm_panic::contain` is the whole host's
/// crash, kept for `take` and written to last-panic.txt; one inside it is logged and left to
/// whoever contained it.
pub fn install() {
    be_wasm_panic::set_hook(Box::new(hook));
}

fn hook(info: &std::panic::PanicHookInfo<'_>) {
    let report = format!("{info}\n\n{}", backtrace());
    let contained = be_wasm_panic::containing();
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unnamed");
    let what = match contained {
        true => "panicked, and the panic was contained",
        false => "panicked",
    };
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&format!("be-host's thread '{name}' {what}: {report}").into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("be-host: thread '{name}' {what}: {report}");
    if contained {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &*lock(&REPORT_FILE) {
        let _ = std::fs::write(path, &report);
    }
    *lock(&LAST_PANIC) = Some(report);
}

pub fn summary(report: &str) -> &str {
    report.split("\n\n").next().unwrap_or(report)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn report_in(data_dir: &std::path::Path) {
    let path = data_dir.join("last-panic.txt");
    if let Ok(report) = std::fs::read_to_string(&path) {
        lock(&PREVIOUS_PANIC).get_or_insert(report);
    }
    *lock(&REPORT_FILE) = Some(path);
}

pub fn take() -> Option<String> {
    lock(&LAST_PANIC).take()
}

pub fn take_previous() -> Option<String> {
    let report = lock(&PREVIOUS_PANIC).take()?;
    forget();
    Some(report)
}

#[cfg_attr(
    any(not(target_os = "linux"), target_arch = "wasm32"),
    allow(dead_code, unused_variables)
)]
pub fn persist(report: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &*lock(&REPORT_FILE) {
        let _ = std::fs::write(path, report);
    }
}

pub fn forget() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &*lock(&REPORT_FILE) {
        let _ = std::fs::remove_file(path);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(target_arch = "wasm32")]
fn backtrace() -> String {
    if let Ok(error) = js_sys::Reflect::get(&js_sys::global(), &"Error".into()) {
        let _ = js_sys::Reflect::set(&error, &"stackTraceLimit".into(), &64.into());
    }
    js_sys::Reflect::get(&js_sys::Error::new(""), &"stack".into())
        .ok()
        .and_then(|stack| stack.as_string())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn backtrace() -> String {
    let trace = std::backtrace::Backtrace::force_capture().to_string();
    #[cfg(target_os = "android")]
    let trace = trace
        .lines()
        .map(|line| match library_offset(line) {
            Some(offset) => format!("{line} {offset}"),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    trace
}

// unsafe(ffi)
#[cfg(target_os = "android")]
fn library_offset(line: &str) -> Option<String> {
    let hex = line
        .split_whitespace()
        .find_map(|word| word.strip_prefix("0x"))?;
    let address = usize::from_str_radix(hex, 16).ok()?;
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    if unsafe { libc::dladdr(address as *const libc::c_void, &mut info) } == 0
        || info.dli_fname.is_null()
    {
        return None;
    }
    let file = unsafe { std::ffi::CStr::from_ptr(info.dli_fname) }.to_string_lossy();
    let name = file.rsplit('/').next().unwrap_or(&file);
    Some(format!("({name}+{:#x})", address - info.dli_fbase as usize))
}

#[cfg(test)]
mod tests;
