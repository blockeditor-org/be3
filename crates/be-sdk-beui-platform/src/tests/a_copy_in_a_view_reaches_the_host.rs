use super::*;

#[test]
fn a_copy_in_a_view_reaches_the_host() {
    let host = EditorHost::default();
    let mut copy = Some("copied".to_owned());
    let mut surface = launched(
        &host,
        Windowed::empty().hooked(move |context| {
            if let Some(text) = copy.take() {
                context.copy(beui_core::clipboard::Clip::text(text));
            }
        }),
    );
    one(&mut surface, &whole(40.0, 30.0));
    let copies: Vec<_> = host
        .take_told()
        .into_iter()
        .filter_map(|message| match message {
            be_sdk_api::shell_types::ShellMessage::Copy(copy) => Some((copy.serial, copy.items)),
            _ => None,
        })
        .collect();
    let [(_, items)] = &copies[..] else {
        panic!("one copy, not {copies:?}");
    };
    assert_eq!(
        items,
        &vec![be_sdk_api::OfferedClipItem {
            format: "text/plain".to_owned(),
            body: Some(b"copied".to_vec().into()),
        }]
    );
}
