use super::*;

fn press(layer: LayerId) -> Pressed {
    Pressed {
        layer,
        escape: false,
    }
}

fn escape(layer: LayerId) -> Pressed {
    Pressed {
        layer,
        escape: true,
    }
}

#[test]
fn a_press_outside_an_active_child_or_escape_in_its_placer_takes_it_back() {
    let mut children = placing(vec![up(1, ChildMode::Active), up(2, ChildMode::Live)], 0);
    let active = |children: &Children| {
        children
            .shown
            .iter()
            .map(|child| child.takes_input(&children.revoked))
            .collect::<Vec<_>>()
    };
    assert_eq!(active(&children), [true, true]);

    // A press in the active child, or Escape there, leaves it active.
    children.revoke(&[press(LayerId(201)), escape(LayerId(201))]);
    assert_eq!(active(&children), [true, true]);

    // A press in its placer's view takes it back; a live child is never taken back.
    children.revoke(&[press(PLACER)]);
    assert_eq!(active(&children), [false, true]);

    // Placed other than active, it is forgotten, and placed active again it takes input.
    children.shown[0].placement.mode = ChildMode::Passive;
    children.forget_revoked();
    children.shown[0].placement.mode = ChildMode::Active;
    assert_eq!(active(&children), [true, true]);

    // A press outside its placer's view, on the root's own surface, takes it back too.
    children.revoke(&[press(LayerId(1))]);
    assert_eq!(active(&children), [false, true]);
    children.revoked.clear();

    // Escape takes it back only in its placer's view.
    children.revoke(&[escape(LayerId(1))]);
    assert_eq!(active(&children), [true, true]);
    children.revoke(&[escape(PLACER)]);
    assert_eq!(active(&children), [false, true]);
}
