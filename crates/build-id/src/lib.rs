use std::{path::Path, sync::OnceLock};

pub const FILE: &str = "build-info.json";

#[derive(Debug, serde::Deserialize)]
pub struct Metadata {
    pub id: String,
    pub app_id: String,
}

impl Metadata {
    pub fn parse(json: &str) -> Result<Self, String> {
        let info: Self = serde_json::from_str(json)
            .map_err(|error| format!("invalid build metadata: {error}"))?;
        if info.id.trim().is_empty() || info.id == "unknown" || info.app_id.trim().is_empty() {
            return Err("the build metadata has no build or app identity".to_owned());
        }
        Ok(info)
    }
}

static CURRENT: OnceLock<Result<Metadata, String>> = OnceLock::new();

pub fn metadata() -> Result<&'static Metadata, String> {
    CURRENT
        .get_or_init(beside_executable)
        .as_ref()
        .map_err(Clone::clone)
}

pub fn current() -> Result<&'static str, String> {
    metadata().map(|info| info.id.as_str())
}

pub fn install(id: &str) {
    let _ = CURRENT.set(Ok(Metadata {
        id: id.to_owned(),
        app_id: "BlockDev".to_owned(),
    }));
}

pub fn install_metadata(json: &str) -> Result<(), String> {
    let info = Metadata::parse(json)?;
    let _ = CURRENT.set(Ok(info));
    Ok(())
}

pub fn install_from(path: &Path) -> Result<(), String> {
    let json = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    install_metadata(&json)
}

pub fn compatible(ours: &str, theirs: &str) -> bool {
    ours == theirs
}

fn beside_executable() -> Result<Metadata, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot find this program's own path: {error}"))?;
    let path = executable.with_file_name(FILE);
    let json = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "cannot read {}: {error}; run this program from its bundle",
            path.display()
        )
    })?;
    Metadata::parse(&json)
}

#[cfg(test)]
mod tests;
