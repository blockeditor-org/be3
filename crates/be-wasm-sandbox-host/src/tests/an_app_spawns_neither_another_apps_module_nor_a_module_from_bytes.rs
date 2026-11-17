use be_sdk_worker::Worker;

use super::*;

#[test]
fn an_app_spawns_neither_another_apps_module_nor_a_module_from_bytes() {
    let mut pump = Pump::echo_app();
    let (other, unstaged, compiled) = pump.run(async {
        let other = guest::sandbox::Instance::new(
            Source::App(StagedModule {
                app: "be3.notes".to_owned(),
                module: "notes.wasm".to_owned(),
            }),
            app_config(),
        );
        let mut unstaged = Worker::spawn("../echo.wasm");
        (
            other.next().await.unwrap(),
            unstaged.receive().await,
            guest::sandbox::compile(ECHO.to_vec()).await.unwrap(),
        )
    });
    assert_eq!(
        other,
        Err("be3.echo may spawn only its own modules, not be3.notes's".to_owned())
    );
    assert_eq!(
        unstaged,
        Err("be3.echo stages no module named \"../echo.wasm\"".to_owned()),
        "a module is named, never found by a path"
    );
    assert_eq!(compiled, Err(APPS_COMPILE_NOTHING.to_owned()));
}
