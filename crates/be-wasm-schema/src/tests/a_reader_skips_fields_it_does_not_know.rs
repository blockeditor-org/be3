use super::*;

#[test]
fn a_reader_skips_fields_it_does_not_know() {
    let bytes = bytes(&person_v2());
    assert_eq!(
        decode::<PersonV1>(&bytes),
        Ok(PersonV1 {
            name: "Ada".into(),
            age: 36
        })
    );
}
