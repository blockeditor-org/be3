use super::*;

#[test]
fn a_point_lands_on_the_topmost_layer_that_takes_it_there() {
    let mut top = layer(2, pos2(50.0, 50.0), (100, 100));
    top.clip = rect(0.0, 0.0, 50.0, 100.0);
    let layers = window(vec![layer(1, pos2(0.0, 0.0), (400, 400)), top]);
    assert_eq!(layers.hit(pos2(60.0, 60.0)), Some(LayerId(2)));
    assert_eq!(
        layers.hit(pos2(90.0, 60.0)),
        Some(LayerId(1)),
        "the part of a surface its clip hides takes nothing"
    );
    assert!(close(
        layers.local(LayerId(2), pos2(60.0, 60.0)).unwrap(),
        pos2(20.0, 20.0)
    ));
    let mut hidden = layers.clone();
    hidden.layers[1].shown = false;
    assert_eq!(hidden.hit(pos2(60.0, 60.0)), Some(LayerId(1)));
    let mut through = layers.clone();
    through.layers[1].input.hit = Area::Nowhere;
    assert_eq!(through.hit(pos2(60.0, 60.0)), Some(LayerId(1)));
}
