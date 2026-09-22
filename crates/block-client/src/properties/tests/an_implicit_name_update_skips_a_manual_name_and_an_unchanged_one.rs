use super::*;

#[test]
fn an_implicit_name_update_skips_a_manual_name_and_an_unchanged_one() {
    let mut properties = BTreeMap::new();
    properties.insert(
        NAME,
        encode_name(&BlockName {
            manual: false,
            value: "Example Domain".to_owned(),
        }),
    );
    assert_eq!(
        implicit_name_update(&properties, Some("Example Domain".to_owned())),
        None
    );

    properties.insert(
        NAME,
        encode_name(&BlockName {
            manual: true,
            value: "Renamed by hand".to_owned(),
        }),
    );
    assert_eq!(
        implicit_name_update(&properties, Some("Something else".to_owned())),
        None
    );
}
