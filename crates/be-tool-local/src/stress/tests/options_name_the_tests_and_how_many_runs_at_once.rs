use super::*;

fn arguments(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn options_name_the_tests_and_how_many_runs_at_once() {
    assert_eq!(
        options(arguments(&["a_test"]), 8),
        Ok(Options {
            name: "a_test".to_owned(),
            repeat: REPEAT,
            parallel: 8,
            limit: LIMIT,
            apps: None,
            latency: None,
        })
    );
    let options = options(
        arguments(&[
            "a_test",
            "--repeat",
            "200",
            "--parallel",
            "14",
            "--limit",
            "60",
        ]),
        8,
    )
    .expect("the flags are known");
    assert_eq!(
        (options.repeat, options.parallel, options.limit),
        (200, 14, Duration::from_secs(60))
    );
    let staged = super::options(arguments(&["a", "--apps", "text,file-tree,chat"]), 8)
        .expect("--apps is known");
    assert_eq!(
        staged.apps.as_deref().map(host_with),
        Some("//crates/be-host-tests:host-with-chat+text".to_owned()),
        "the base apps are left out and the rest sorted, as apps.bzl names a host"
    );
    let base = super::options(arguments(&["a", "--apps", ""]), 8).expect("no apps is a host too");
    assert_eq!(
        base.apps.as_deref().map(host_with),
        Some("//crates/be-host-tests:host-with-base".to_owned())
    );
    assert!(super::options(arguments(&["a", "--apps"]), 8).is_err());
    assert_eq!(
        super::options(arguments(&["a", "--latency", "100"]), 8)
            .expect("--latency is known")
            .latency,
        Some(Duration::from_millis(100))
    );
    assert!(
        super::options(arguments(&[]), 8).is_err(),
        "a name is needed"
    );
    assert!(
        super::options(arguments(&["a", "b"]), 8).is_err(),
        "and only one"
    );
    assert!(super::options(arguments(&["a", "--repeat", "0"]), 8).is_err());
    assert!(super::options(arguments(&["a", "--repeat"]), 8).is_err());
    assert!(super::options(arguments(&["a", "--forever"]), 8).is_err());
}
