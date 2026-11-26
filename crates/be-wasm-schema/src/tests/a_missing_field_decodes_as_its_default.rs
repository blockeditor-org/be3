use super::*;

#[test]
fn a_missing_field_decodes_as_its_default() {
    let old = encode(&PersonV1 {
        name: "Ada".into(),
        age: 36,
    });
    assert_eq!(
        decode::<PersonV2>(&old),
        Ok(PersonV2 {
            name: "Ada".into(),
            age: 36,
            email: None,
            tags: Vec::new()
        })
    );
}
