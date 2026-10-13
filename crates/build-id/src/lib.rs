use std::{path::Path, sync::OnceLock};

pub const FILE: &str = "build-id";

static CURRENT: OnceLock<Result<String, String>> = OnceLock::new();

pub fn current() -> Result<&'static str, String> {
    CURRENT
        .get_or_init(beside_executable)
        .as_deref()
        .map_err(Clone::clone)
}

pub fn install(id: &str) {
    let _ =
        CURRENT.set(normalized(id).ok_or_else(|| "the build id is empty or unknown".to_owned()));
}

pub fn install_from(path: &Path) -> Result<(), String> {
    let id = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read the build id in {}: {error}", path.display()))?;
    let id = normalized(&id).ok_or_else(|| format!("{} names no build", path.display()))?;
    let _ = CURRENT.set(Ok(id));
    Ok(())
}

pub fn compatible(ours: &str, theirs: &str) -> bool {
    ours == theirs
}

const UNKNOWN: &str = "unknown";

fn normalized(id: &str) -> Option<String> {
    Some(id.trim())
        .filter(|id| !id.is_empty() && *id != UNKNOWN)
        .map(str::to_owned)
}

fn beside_executable() -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot find this program's own path: {error}"))?;
    let path = executable.with_file_name(FILE);
    let id = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "cannot read {}: {error}; run this program from the bundle buck builds for it, which puts its build id there",
            path.display()
        )
    })?;
    normalized(&id).ok_or_else(|| format!("{} names no build", path.display()))
}

#[cfg(test)]
mod tests;
