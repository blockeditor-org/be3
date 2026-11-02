use super::*;

#[test]
fn a_contained_panic_is_not_a_crash_but_a_loose_one_is() {
    install();

    let contained = contain(|| -> u32 { panic!("a contained editor bug") });

    assert_eq!(contained, Err("a contained editor bug".to_owned()));
    assert!(
        take().is_none_or(|report| !report.contains("a contained editor bug")),
        "a contained panic does not crash the app"
    );
    assert_eq!(contain(|| 7), Ok(7));

    let loose = std::thread::spawn(|| -> u32 { panic!("a loose background bug") }).join();

    assert!(loose.is_err());
    let report = take().expect("a panic nothing contained crashes the app");
    assert!(report.contains("a loose background bug"), "{report}");
    assert_eq!(
        summary(&report).lines().last(),
        Some("a loose background bug"),
        "the summary leaves the backtrace out"
    );
}
