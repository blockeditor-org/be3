//! `buck-tools unzstd IN OUT`: decompresses a bare zstd stream into an
//! executable, as buck2's release of starlark_fmt is (buck/tools/BUCK), which
//! http_archive does not unpack.

use std::process::ExitCode;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [input, out] = arguments else {
        return Err("usage: unzstd IN OUT".into());
    };
    let compressed =
        std::fs::read(input).map_err(|error| format!("cannot read {input}: {error}"))?;
    let decompressed = zstd::decode_all(compressed.as_slice())
        .map_err(|error| format!("cannot decompress {input}: {error}"))?;
    std::fs::write(out, decompressed).map_err(|error| format!("cannot write {out}: {error}"))?;
    executable(out)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(unix)]
fn executable(path: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("cannot make {path} executable: {error}"))
}

#[cfg(not(unix))]
fn executable(_: &str) -> Result<(), String> {
    Ok(())
}
