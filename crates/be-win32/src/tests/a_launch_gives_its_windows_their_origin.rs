use super::*;

fn launch(process_id: Option<u32>, program: &str, target: &str, number: u64) -> Launch {
    Launch {
        process_id,
        program: Some(program.to_owned()),
        command: vec![target.to_owned()],
        launch: Some(number),
        target: Some(PathBuf::from(target)),
    }
}

#[test]
fn a_launch_gives_its_windows_their_origin() {
    let desktop = FakeDesktop::new();
    let mut model = Model::new();
    model.launched(launch(
        Some(101),
        "Accessories/Paint",
        "C:\\Windows\\mspaint.exe",
        7,
    ));
    desktop.add(window(1, "Paint"));
    model.update(&desktop, &[]);
    let origin = model.snapshot().entries[0].origin.clone();
    assert_eq!(origin.program.as_deref(), Some("Accessories/Paint"));
    assert_eq!(origin.launch, Some(7));

    model.launched(launch(
        None,
        "Browser",
        "C:\\Program Files\\App102\\APP.EXE",
        8,
    ));
    desktop.add(window(2, "Browser"));
    model.update(&desktop, &[]);
    let origin = model.snapshot().entries[1].origin.clone();
    assert_eq!(origin.launch, Some(8));

    desktop.add(window(3, "Unlaunched"));
    model.update(&desktop, &[]);
    let origin = model.snapshot().entries[2].origin.clone();
    assert_eq!(origin.program, None);
    assert_eq!(origin.command, ["C:\\Program Files\\App103\\app.exe"]);

    model.launched(launch(Some(103), "Late", "C:\\late.exe", 9));
    model.update(&desktop, &[]);
    assert_eq!(model.snapshot().entries[2].origin.launch, Some(9));
}
