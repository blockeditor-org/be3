use super::*;

#[test]
fn a_region_with_damage_has_changed_and_keeps_its_surface_while_shown() {
    let mut driver = driven(Plain {
        damage: Some(damaged()),
        ..Plain::default()
    });
    let damaged = shown(1);
    let clean = shown(2);
    let frames = driver.frame(&[damaged.clone(), clean.clone()]);
    assert!(
        frames[0].changed,
        "the region that damaged its surface has changed"
    );
    assert!(!frames[1].changed, "a region with no damage has not");
    assert_eq!(
        driver
            .surfaces()
            .get(damaged.id())
            .map(|surface| surface.size),
        Some((40, 30))
    );
    driver.frame(std::slice::from_ref(&clean));
    assert!(
        driver.surfaces().get(damaged.id()).is_none(),
        "a region the host stops showing loses its surface"
    );
    assert!(driver.surfaces().get(clean.id()).is_some());
}
