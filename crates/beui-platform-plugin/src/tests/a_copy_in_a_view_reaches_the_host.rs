use super::*;

#[test]
fn a_copy_in_a_view_reaches_the_host() {
    let host = EditorHost::default();
    let mut copy = Some("copied".to_owned());
    let mut surface = launched(
        &host,
        Windowed::empty().hooked(move |context| {
            if let Some(text) = copy.take() {
                context.copy_text(text);
            }
        }),
    );
    one(&mut surface, &whole(40.0, 30.0));
    assert_eq!(host.take_copied_text(), vec!["copied".to_owned()]);
}
