use super::*;

/// A server stopped while a compile is under way answers that compile with the stop's reason, so
/// its caller is not left waiting.
#[test]
fn a_stop_answers_the_compiles_under_way() {
    let mut server = idle_server();
    let heard = Rc::new(RefCell::new(None));
    let answered = Rc::clone(&heard);
    let compiling = be_wasm_host::sandbox::compile(ECHO.to_vec(), Arc::new(|| {}));
    server.compiling.push((
        compiling,
        Box::new(move |id: &Result<ModuleId, String>| {
            *answered.borrow_mut() = Some(id.clone().map(|_| ()));
            (1, Outcome::Ok(Vec::new()))
        }),
    ));

    server.stop("the app stopped", &mut Connection::new(Side::Host));

    assert_eq!(
        heard.borrow_mut().take(),
        Some(Err("the app stopped".to_owned())),
        "the compile hears why the server stopped"
    );
}
