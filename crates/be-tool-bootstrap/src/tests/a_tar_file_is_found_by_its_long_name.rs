use super::*;

fn header(name: &str, size: usize, kind: u8) -> Vec<u8> {
    let mut header = vec![0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[124..135].copy_from_slice(format!("{size:011o}").as_bytes());
    header[156] = kind;
    header[257..265].copy_from_slice(b"ustar  \0");
    header
}

fn entry(archive: &mut Vec<u8>, name: &str, data: &[u8], kind: u8) {
    archive.extend(header(name, data.len(), kind));
    archive.extend(data);
    archive.resize(archive.len().div_ceil(512) * 512, 0);
}

#[test]
fn a_tar_file_is_found_by_its_long_name() {
    let long = format!("{}/cargo/bin/cargo", "d".repeat(120));
    let mut archive = Vec::new();
    entry(&mut archive, "d/", b"", b'5');
    entry(&mut archive, "d/other", b"other", b'0');
    entry(
        &mut archive,
        "././@LongLink",
        format!("{long}\0").as_bytes(),
        b'L',
    );
    entry(&mut archive, &long[..100], b"the binary", b'0');
    archive.extend([0; 1024]);
    assert_eq!(
        tar_file(archive.as_slice(), &long).unwrap().as_deref(),
        Some(&b"the binary"[..])
    );
    assert_eq!(
        tar_file(archive.as_slice(), "d/other").unwrap().as_deref(),
        Some(&b"other"[..])
    );
    assert_eq!(tar_file(archive.as_slice(), "d/missing").unwrap(), None);
}
