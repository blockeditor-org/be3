use super::*;

fn type_password(test: &mut DocumentTest, password: &str) {
    for letter in password.chars() {
        test.frame(vec![Input::Text(letter.to_string())]);
    }
    press(test, Key::Enter);
}

#[test]
fn it_stays_locked_until_its_password_is_typed() {
    let _root = RootScope::new();
    let locked = create_signal(true);
    let error = create_signal(None::<String>);
    let document = build(move || {
        view! {
            <Root>
                <LockScreen
                    open={locked}
                    time="09:41"
                    date="Thursday, 8 October"
                    user="Ada Lovelace"
                    error={error}
                    actions={vec![suspend()]}
                    on_submit={move |typed: String| match typed == "beui" {
                        true => locked.set(false),
                        false => error.set(Some("That password is not right.".to_owned())),
                    }}
                />
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, WIDE);
    assert!(test.shows("lock.0"), "the lock screen covers the window");

    type_password(&mut test, "hunter2");
    assert!(test.shows("lock.0.error"), "a wrong password says so");

    press(&mut test, Key::Escape);
    test.click(pos2(4.0, 4.0));
    test.frame(Vec::new());
    assert!(
        test.shows("lock.0"),
        "neither Escape nor a click outside the card unlocks"
    );

    type_password(&mut test, "beui");
    assert!(!test.shows("lock.0"), "the right password unlocks");
}
