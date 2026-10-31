use super::*;

#[test]
fn an_offset_into_another_modules_thread_local_is_refused() {
    let object = reaching_object("ELSEWHERE", 8);
    let foreign = HashSet::from(["ELSEWHERE".to_owned()]);
    let refusal = tls::through_got(&object, &foreign).unwrap_err();
    assert!(
        refusal.contains("8 bytes into the thread-local ELSEWHERE"),
        "{refusal}"
    );
}
