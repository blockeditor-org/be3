//! One file out of a tar archive, as Rust's dist tarballs are written (GNU
//! tar's format, whose long names come in an entry of their own, and POSIX's,
//! whose come in a pax header or the ustar prefix).

use std::io::Read;

use crate::Result;

/// The contents of the regular file at `wanted` in the archive, or None when it
/// has none.
pub fn file(mut archive: impl Read, wanted: &str) -> Result<Option<Vec<u8>>> {
    let mut long_name: Option<String> = None;
    loop {
        let mut header = [0u8; 512];
        if !read_block(&mut archive, &mut header)? || header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }
        let size = octal(&header[124..136])?;
        let kind = header[156];
        let mut data = vec![0; usize::try_from(size).map_err(|_| "a tar entry too large")?];
        archive
            .read_exact(&mut data)
            .map_err(|error| format!("the archive ends inside an entry: {error}"))?;
        let padding = (512 - size % 512) % 512;
        std::io::copy(&mut (&mut archive).take(padding), &mut std::io::sink())
            .map_err(|error| format!("could not read the archive: {error}"))?;
        match kind {
            b'L' => long_name = Some(text(&data)),
            b'x' => long_name = pax_path(&data).or(long_name),
            b'0' | 0 => {
                let name = long_name.take().unwrap_or_else(|| name(&header));
                if name.trim_start_matches("./") == wanted {
                    return Ok(Some(data));
                }
            }
            _ => long_name = None,
        }
    }
}

fn read_block(archive: &mut impl Read, block: &mut [u8; 512]) -> Result<bool> {
    let mut read = 0;
    while read < block.len() {
        match archive.read(&mut block[read..]) {
            Ok(0) if read == 0 => return Ok(false),
            Ok(0) => return Err("the archive ends inside a header".to_owned()),
            Ok(count) => read += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(format!("could not read the archive: {error}")),
        }
    }
    Ok(true)
}

fn text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn name(header: &[u8; 512]) -> String {
    let name = text(&header[..100]);
    // POSIX ustar keeps a long path's directories in a prefix; GNU tar's
    // "ustar  " uses those bytes for other things.
    if &header[257..263] == b"ustar\0" {
        let prefix = text(&header[345..500]);
        if !prefix.is_empty() {
            return format!("{prefix}/{name}");
        }
    }
    name
}

fn octal(field: &[u8]) -> Result<u64> {
    let digits = text(field);
    let digits = digits.trim_matches(|character: char| character == ' ' || character == '\0');
    if digits.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(digits, 8).map_err(|_| format!("a tar header's size is {digits:?}"))
}

/// The path a pax header gives, from its "LENGTH path=VALUE\n" records.
fn pax_path(data: &[u8]) -> Option<String> {
    String::from_utf8_lossy(data)
        .lines()
        .filter_map(|record| record.split_once(' ').map(|(_, record)| record))
        .find_map(|record| record.strip_prefix("path=").map(str::to_owned))
}
