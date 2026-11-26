use super::*;

#[test]
fn some_of_a_default_is_not_none() {
    for value in [
        Opt {
            count: Some(0),
            nested: Some(None),
        },
        Opt {
            count: None,
            nested: Some(Some(0)),
        },
        Opt {
            count: Some(7),
            nested: None,
        },
    ] {
        assert_eq!(decode::<Opt>(&encode(&value)), Ok(value));
    }
    assert_eq!(
        encode(&Opt {
            count: None,
            nested: None
        }),
        Vec::<u8>::new()
    );
}
