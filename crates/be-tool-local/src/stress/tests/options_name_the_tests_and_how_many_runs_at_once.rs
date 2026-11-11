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
        })
    );
    let options = options(
        arguments(&["a_test", "--repeat", "200", "--parallel", "14", "--limit", "60"]),
        8,
    )
    .expect("the flags are known");
    assert_eq!(
        (options.repeat, options.parallel, options.limit),
        (200, 14, Duration::from_secs(60))
    );
    assert!(super::options(arguments(&[]), 8).is_err(), "a name is needed");
    assert!(super::options(arguments(&["a", "b"]), 8).is_err(), "and only one");
    assert!(super::options(arguments(&["a", "--repeat", "0"]), 8).is_err());
    assert!(super::options(arguments(&["a", "--repeat"]), 8).is_err());
    assert!(super::options(arguments(&["a", "--forever"]), 8).is_err());
}
