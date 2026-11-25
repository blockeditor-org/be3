use super::*;

#[test]
fn a_fallible_call_answers_its_error_only_when_it_fails() {
    let mut caller = Caller::new();
    assert_eq!(embed::shapes::glue::check(&mut caller, 1, 64, 256), 0, "success is 0");

    let length = embed::shapes::glue::check(&mut caller, 0, 64, 256);
    assert!(length > 1, "an error answers its length plus one");
    let error: shapes::shapes::Broken =
        be_wasm_ops::decode(caller.bytes(64, length - 1)).expect("the error decodes");
    assert_eq!(
        error,
        shapes::shapes::Broken::Failed(shapes::shapes::Failed {
            message: "nothing to check".to_owned(),
        })
    );

    let length = embed::shapes::glue::check(&mut caller, 0, 64, 2);
    assert!(length > 3, "an error longer than its room waits to be taken");
    assert_eq!(caller.returned.len() as u32, length - 1);
}
