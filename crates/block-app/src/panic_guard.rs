use std::sync::Mutex;

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

#[cfg(not(target_arch = "wasm32"))]
static REPORT_FILE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub(crate) fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = format!("{info}\n\n{}", backtrace());
        #[cfg(target_arch = "wasm32")]
        web_sys::console::error_1(&format!("Block panicked: {report}").into());
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &*lock(&REPORT_FILE) {
            let _ = std::fs::write(path, &report);
        }
        *lock(&LAST_PANIC) = Some(report);
        previous(info);
    }));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn report_in(data_dir: &std::path::Path) {
    let path = data_dir.join("last-panic.txt");
    if let Ok(report) = std::fs::read_to_string(&path) {
        lock(&LAST_PANIC).get_or_insert(format!(
            "Block closed after this error the last time it ran.\n\n{report}"
        ));
    }
    *lock(&REPORT_FILE) = Some(path);
}

pub(crate) fn take() -> Option<String> {
    let report = lock(&LAST_PANIC).take()?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &*lock(&REPORT_FILE) {
        let _ = std::fs::remove_file(path);
    }
    Some(report)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
