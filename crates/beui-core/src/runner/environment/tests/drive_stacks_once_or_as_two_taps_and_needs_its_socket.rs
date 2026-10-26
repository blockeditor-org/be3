use super::*;

#[test]
fn drive_stacks_once_or_as_two_taps_and_needs_its_socket() {
    assert_eq!(
        choose(
            "winit",
            with(&[(LAYERS, "drive,space,drive"), (DRIVE_SOCKET, "/tmp/d.sock")])
        ),
        Ok(Choice {
            layers: vec![Stacked::Drive, Stacked::Space, Stacked::Drive],
            ..NATIVE
        })
    );
    assert!(
        choose("winit", with(&[(LAYERS, "drive")]))
            .is_err_and(|error| error.contains(DRIVE_SOCKET))
    );
    assert!(
        choose("winit", with(&[(DRIVE_SOCKET, "/tmp/d.sock")]))
            .is_err_and(|error| error.contains("add drive"))
    );
    assert!(
        choose(
            "winit",
            with(&[(LAYERS, "drive,drive,drive"), (DRIVE_SOCKET, "/tmp/d.sock")])
        )
        .is_err()
    );
}
