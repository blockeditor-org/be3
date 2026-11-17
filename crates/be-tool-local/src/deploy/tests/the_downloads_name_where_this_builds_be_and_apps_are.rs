use super::*;

use std::io::{Cursor, Write};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[test]
fn the_downloads_name_where_this_builds_be_and_apps_are() {
    let bundle = std::env::temp_dir().join(format!("be3-deploy-be-{}", std::process::id()));
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("be"), b"be for arm").unwrap();
    std::fs::write(bundle.join("app-hashes.json"), b"{}").unwrap();
    let apps = release_apps(&bundle, "tree", "https://be3.example").unwrap();
    let hosts = terminal_hosts(
        &[
            ("aarch64-unknown-linux-gnu".to_owned(), bundle.clone()),
            ("riscv64gc-unknown-linux-gnu".to_owned(), bundle.clone()),
        ],
        "tree",
        "https://be3.example",
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&bundle);
    assert_eq!(
        hosts,
        vec![be_lib_build_id::TerminalHostBinary {
            arch: "arm64".to_owned(),
            url: "https://be3.example/data/tree/be-aarch64-unknown-linux-gnu/be".to_owned(),
            sha256: hex(&Sha256::digest(b"be for arm")),
        }],
        "only the architectures a terminal host runs on"
    );
    assert_eq!(
        apps,
        be_lib_build_id::ReleaseApps {
            url: "https://be3.example/data/tree/web".to_owned(),
            sha256: hex(&Sha256::digest(b"{}")),
        },
        "the apps are the web bundle's, pinned by the hash of the list of their hashes"
    );
    let release = Release {
        terminal_hosts: hosts.clone(),
        apps: Some(apps.clone()),
    };

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let executable = SimpleFileOptions::default().unix_permissions(0o755);
    zip.start_file("be-host-linux-arm64/be-host", executable)
        .unwrap();
    zip.write_all(b"the host").unwrap();
    zip.start_file(
        "be-host-linux-arm64/build-info.json",
        SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(br#"{"id":"tree","program_id":"be-host","default_server":{"name":"be3","api_url":"a","auth_url":"b","gateway_required":true}}"#)
        .unwrap();
    let archive = zip.finish().unwrap().into_inner();

    let rewritten = archive_with_release(&archive, &release).unwrap();
    let mut read = ZipArchive::new(Cursor::new(rewritten)).unwrap();
    let mut host = read.by_name("be-host-linux-arm64/be-host").unwrap();
    assert_eq!(host.unix_mode().map(|mode| mode & 0o777), Some(0o755));
    let mut bytes = Vec::new();
    host.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"the host");
    drop(host);
    let mut json = String::new();
    read.by_name("be-host-linux-arm64/build-info.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let metadata = be_lib_build_id::Metadata::parse(&json).unwrap();
    assert_eq!(metadata.terminal_hosts, hosts);
    assert_eq!(metadata.apps, Some(apps.clone()));
    assert_eq!(
        be_lib_build_id::Metadata::parse(&metadata.for_be())
            .unwrap()
            .apps,
        Some(apps),
        "a terminal host's be finds the same apps"
    );
    assert!(
        metadata.default_server.is_some(),
        "the other fields are kept"
    );
}
