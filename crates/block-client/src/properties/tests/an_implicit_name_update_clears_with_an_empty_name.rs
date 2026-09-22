use super::*;

#[test]
fn an_implicit_name_update_clears_with_an_empty_name() {
    let mut properties = BTreeMap::new();
    let set = implicit_name_update(&properties, Some("Example Domain".to_owned()))
        .expect("a new name is written");
    properties.insert(NAME, set);
    assert_eq!(
        read_name(&properties).map(|name| name.value),
        Some("Example Domain".to_owned())
    );

    let cleared =
        implicit_name_update(&properties, None).expect("losing the name writes an empty one");
    properties.insert(NAME, cleared);

    assert_eq!(read_name(&properties), None);
    assert_eq!(implicit_name_update(&properties, None), None);
}
