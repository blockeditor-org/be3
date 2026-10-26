use super::*;

#[test]
fn a_crashed_plugin_offers_a_reload() {
    let pressed = Pressed::default();
    let reload = pressing(&pressed, "reload");
    let mut stepped = stepped(move || {
        view! {
            <CrashNotice
                title="Counter stopped"
                detail="the count overflowed\n(at src/counter.rs:12:9)"
                on_reload={reload}
            />
        }
    });
    snapshot(&mut stepped, "a_crashed_plugin");
    click(&mut stepped, "crash-notice.reload");
    assert_eq!(*pressed.borrow(), ["reload"]);
}
