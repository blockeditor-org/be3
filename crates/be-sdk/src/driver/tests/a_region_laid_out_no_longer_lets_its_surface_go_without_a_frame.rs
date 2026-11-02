use super::*;

#[test]
fn a_region_laid_out_no_longer_lets_its_surface_go_without_a_frame() {
    let mut driver = driven(Plain::default());
    let kept = shown(1);
    let gone = shown(2);
    driver.frame(&[kept.clone(), gone.clone()]);
    assert!(driver.surfaces().get(gone.id()).is_some());

    driver.laid_out(&[kept.id()]);
    assert!(
        driver.surfaces().get(gone.id()).is_none(),
        "a region the app no longer lays out loses its surface at once"
    );
    assert!(driver.surfaces().get(kept.id()).is_some());

    driver.laid_out(&[]);
    assert!(
        driver.surfaces().get(kept.id()).is_none(),
        "an app that shows nothing keeps no surfaces, though it runs no frame"
    );
}
