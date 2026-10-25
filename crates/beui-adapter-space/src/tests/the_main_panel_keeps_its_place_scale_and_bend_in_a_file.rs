use super::*;

fn started(path: &std::path::Path, now: Instant) -> Space {
    let mut space = Space::keeping_main(path.to_path_buf());
    space.input(&mut Vec::new(), &view(now));
    space
}

#[test]
fn the_main_panel_keeps_its_place_scale_and_bend_in_a_file() {
    let directory = std::env::temp_dir().join(format!(
        "beui-adapter-space-kept-main-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    let path = directory.join("app").join("space-main-panel.txt");
    let now = Instant::now();

    let mut space = started(&path, now);
    let straight = *main(&space);
    assert_eq!(
        straight,
        *main(&super::space().0),
        "with nothing saved the main panel opens straight ahead"
    );
    assert!(
        !path.exists(),
        "opening where it always does writes nothing"
    );

    let corner = on_part(&space, WindowId::MAIN, Part::Scale(Corner::BottomRight));
    let to = corner + vec2(60.0, 40.0);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(corner),
            button(corner, true),
            Input::PointerMoved(to),
        ],
    );
    assert!(!path.exists(), "nothing is written while the gesture lasts");
    send(&mut space, now, [button(to, false)]);
    let scaled = *main(&space);
    assert_ne!(scaled.metres_per_point, straight.metres_per_point);
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(scaled.placement().0),
        "letting go writes the main panel's placement"
    );

    let mut shaped = scaled;
    shaped.bend = 0.9;
    shaped.pose.rotation = glam::Quat::from_rotation_y(0.4) * glam::Quat::from_rotation_z(0.1);
    shaped.pose.position += Vec3::new(0.3, 0.1, -0.2);
    std::fs::write(&path, shaped.placement().0).expect("the placement is written");

    let reopened = *main(&started(&path, now));
    assert_eq!(
        reopened, shaped,
        "the next start puts the main panel back where it was, at its scale and bend"
    );

    std::fs::write(&path, "space nonsense").expect("the file is written");
    assert_eq!(
        *main(&started(&path, now)),
        straight,
        "a file it cannot read opens the main panel straight ahead"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
