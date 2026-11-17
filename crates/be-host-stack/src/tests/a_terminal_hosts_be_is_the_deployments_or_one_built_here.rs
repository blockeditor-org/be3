use be_lib_build_id::Metadata;
use be_sdk_api::TerminalHostSource;

use crate::terminal_host::answer;

#[test]
fn a_terminal_hosts_be_is_the_deployments_or_one_built_here() {
    let deployed = Metadata::parse(
        r#"{"id":"tree","program_id":"be-host","terminal_hosts":[{"arch":"x86_64","url":"https://be3.example/data/tree/be-x86_64-unknown-linux-gnu/be","sha256":"ab12"}]}"#,
    )
    .unwrap();
    let answered = answer(&deployed, "x86_64", None).unwrap();
    match answered.source {
        TerminalHostSource::Download(download) => {
            assert_eq!(
                download.url,
                "https://be3.example/data/tree/be-x86_64-unknown-linux-gnu/be"
            );
            assert_eq!(download.sha256, "ab12");
        }
        TerminalHostSource::Files(_) => panic!("a deployed build downloads be"),
    }
    let beside = Metadata::parse(&answered.build_info).unwrap();
    assert_eq!(beside.id, "tree");
    assert!(answer(&deployed, "riscv64", None).is_err());

    let not_deployed_for_arm =
        answer(&deployed, "arm64", Some(std::path::Path::new("/nowhere"))).unwrap_err();
    assert!(
        !not_deployed_for_arm.contains("./scripts/buck"),
        "a deployed build never asks a person to build be: {not_deployed_for_arm}"
    );

    let development = Metadata::parse(r#"{"id":"work","program_id":"be-host-dev"}"#).unwrap();
    let built = std::env::temp_dir().join(format!("be3-terminal-host-{}", uuid::Uuid::new_v4()));
    let missing = answer(&development, "arm64", Some(&built)).unwrap_err();
    assert!(
        missing.contains(
            "./scripts/buck build //crates/be-tool-cli:terminal-host --target-platforms root//buck/platforms:linux_arm64 --out"
        ),
        "the missing build names what to build: {missing}"
    );

    let arm = built.join("arm64");
    std::fs::create_dir_all(&arm).unwrap();
    std::fs::write(arm.join("be"), b"\x7fELF be").unwrap();
    std::fs::write(
        arm.join(be_lib_build_id::FILE),
        r#"{"id":"older","program_id":"be-host-dev"}"#,
    )
    .unwrap();
    let stale = answer(&development, "arm64", Some(&built)).unwrap_err();
    assert!(stale.contains("another build"), "{stale}");

    std::fs::write(
        arm.join(be_lib_build_id::FILE),
        r#"{"id":"work","program_id":"be-host-dev"}"#,
    )
    .unwrap();
    let only_be = answer(&development, "arm64", Some(&built)).unwrap_err();
    assert!(
        only_be.contains("no apps beside it") && only_be.contains(":terminal-host"),
        "a be built alone reads no block: {only_be}"
    );

    // As //crates/be-tool-cli:terminal-host lays it out: the apps' manifests and type
    // modules beside be, the terminal's among them.
    std::fs::write(arm.join("apps.json"), r#"["be3.terminal.app.json"]"#).unwrap();
    std::fs::write(arm.join("be3.terminal.app.json"), "{}").unwrap();
    std::fs::write(arm.join("terminal_types.wasm"), b"\0asm terminal").unwrap();
    std::fs::write(arm.join("terminal_types.sha256"), "ab12\n").unwrap();
    std::fs::write(arm.join("program-files.txt"), "be\n").unwrap();
    let answered = answer(&development, "arm64", Some(&built)).unwrap();
    let TerminalHostSource::Files(files) = answered.source else {
        panic!("a development build uploads its own files");
    };
    let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "apps.json",
            "be",
            "be3.terminal.app.json",
            "terminal_types.sha256",
            "terminal_types.wasm"
        ],
        "be with the terminal's type module beside it, and its build-info.json as the answer's own"
    );
    let types = files
        .iter()
        .find(|file| file.name == "terminal_types.wasm")
        .unwrap();
    assert_eq!(types.bytes, b"\0asm terminal");
    let _ = std::fs::remove_dir_all(&built);
}
