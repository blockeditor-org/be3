use super::*;

#[test]
fn the_app_id_is_the_app_user_model_id_else_the_executable() {
    let desktop = FakeDesktop::new();
    desktop.add(window(1, "Store app"));
    desktop.add(window(2, "Classic"));
    desktop
        .app_ids
        .borrow_mut()
        .insert(Handle(1), "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".to_owned());
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let ids: Vec<String> = model.snapshot().entries.iter().map(|entry| entry.app_id.clone()).collect();
    assert_eq!(
        ids,
        ["Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "C:\\Program Files\\App102\\app.exe"]
    );
}
