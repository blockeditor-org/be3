use std::{path::Path, sync::OnceLock};

pub const UNKNOWN: &str = "unknown";

pub const FILE: &str = "build-id";

static CURRENT: OnceLock<String> = OnceLock::new();

pub fn current() -> &'static str {
    CURRENT.get_or_init(beside_executable)
}

pub fn install(id: &str) {
    let _ = CURRENT.set(normalized(id));
}

pub fn install_from(path: &Path) -> Result<(), String> {
    let id = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read the build id in {}: {error}", path.display()))?;
    install(&id);
    Ok(())
}

pub fn commit(id: &str) -> &str {
    id.split_once('+').map_or(id, |(commit, _)| commit)
}

pub fn compatible(ours: &str, theirs: &str) -> bool {
    ours == theirs || ours == UNKNOWN || theirs == UNKNOWN
}

fn normalized(id: &str) -> String {
    match id.trim() {
        "" => UNKNOWN.to_owned(),
        id => id.to_owned(),
    }
}

fn beside_executable() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|executable| std::fs::read_to_string(executable.with_file_name(FILE)).ok())
        .map_or_else(|| UNKNOWN.to_owned(), |id| normalized(&id))
}

#[cfg(test)]
mod tests;
