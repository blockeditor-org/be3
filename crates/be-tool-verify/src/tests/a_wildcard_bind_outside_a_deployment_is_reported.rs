use super::super::binds::wildcard_binds;

#[test]
fn a_wildcard_bind_outside_a_deployment_is_reported() {
    let loopback = "{\n\tdefault_bind {$SITE_BIND:127.0.0.1}\n}\nhttp://:{$SITE_PORT} {\n}\n";
    assert!(wildcard_binds("crates/a/web/Caddyfile", loopback).is_empty());
    assert!(
        wildcard_binds(
            "crates/a/web/site.Caddyfile",
            "{\n\tdefault_bind [::1]\n}\n"
        )
        .is_empty()
    );

    let every_interface = "http://127.0.0.1:8080 {\n}\n";
    assert_eq!(
        wildcard_binds("crates/a/web/Caddyfile", every_interface).len(),
        1
    );
    let fallback_everywhere = "{\n\tdefault_bind {$SITE_BIND:0.0.0.0}\n}\n";
    assert_eq!(
        wildcard_binds("buck/program/site.Caddyfile", fallback_everywhere).len(),
        2
    );
    assert!(wildcard_binds("scripts/internal/nativelink/Caddyfile", every_interface).is_empty());

    let harness = "server.listen(0, \"0.0.0.0\", ready);\n// not \"0.0.0.0\"\n";
    let found = wildcard_binds("buck/program/test.mjs", harness);
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("buck/program/test.mjs:1:"));
    let rust = "let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0));\n";
    assert_eq!(wildcard_binds("crates/a/src/tests.rs", rust).len(), 1);
    assert!(wildcard_binds("crates/be-host/web/serve.sh", "bind=\"[::]\"\n").is_empty());
    assert!(wildcard_binds("guides/hosting.md", "`--addr 0.0.0.0:8080`\n").is_empty());

    let domain = "    bind=\"[::]\"\n    address=0.0.0.0:8080\n";
    let found = wildcard_binds("crates/be-host/web/serve.sh", domain);
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("crates/be-host/web/serve.sh:2:"));
    assert_eq!(wildcard_binds("buck/program/serve.sh", "bind=[::]\n").len(), 1);

    let node = "server.listen(0, \"127.0.0.1\", ready);\nserver.listen(8080);\nprobe.listen(0, () => {});\n";
    let found = wildcard_binds("buck/program/harness.mjs", node);
    assert_eq!(found.len(), 2);
    assert!(found[0].starts_with("buck/program/harness.mjs:2:"));
    assert!(found[1].starts_with("buck/program/harness.mjs:3:"));
    assert!(wildcard_binds("crates/a/src/lib.rs", "link.listen(callback);\n").is_empty());
}
