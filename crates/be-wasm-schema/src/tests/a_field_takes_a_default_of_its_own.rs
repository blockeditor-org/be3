use super::*;

#[test]
fn a_field_takes_a_default_of_its_own() {
    let defaults = Config {
        enabled: true,
        retries: 7,
    };
    assert_eq!(bytes(&defaults), Vec::<u8>::new());
    assert_eq!(decode::<Config>(&[]), Ok(defaults));
    let changed = Config {
        enabled: false,
        retries: 0,
    };
    assert_eq!(decode::<Config>(&bytes(&changed)), Ok(changed));
}
