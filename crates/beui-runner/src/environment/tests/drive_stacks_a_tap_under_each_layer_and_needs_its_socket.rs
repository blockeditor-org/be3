use super::*;

#[test]
fn drive_stacks_a_tap_under_each_layer_and_needs_its_socket() {
    let socket = |layers| {
        choose(
            &["winit"],
            with(&[(LAYERS, layers), (DRIVE_SOCKET, "/tmp/d.sock")]),
        )
    };
    assert_eq!(
        socket("drive,space,drive"),
        Ok(Choice {
            layers: vec![Stacked::Drive, Stacked::Space, Stacked::Drive],
            ..NATIVE
        })
    );
    assert_eq!(
        drive_taps(&[Stacked::Drive, Stacked::Space, Stacked::Drive]),
        vec![Some(Stacked::Space), None],
        "each tap is named by the layer just above it, and the top one is the app's"
    );
    assert!(
        choose(&["winit"], with(&[(LAYERS, "drive")]))
            .is_err_and(|error| error.contains(DRIVE_SOCKET))
    );
    assert!(
        choose(&["winit"], with(&[(DRIVE_SOCKET, "/tmp/d.sock")]))
            .is_err_and(|error| error.contains("add drive"))
    );
    assert!(
        socket("drive,drive").is_err_and(|error| error.contains("twice in a row")),
        "two taps with nothing between them reach the same place"
    );
    assert!(
        socket("drive,space,drive,space").is_err_and(|error| error.contains("--into")),
        "two taps under space could not be told apart"
    );
}
