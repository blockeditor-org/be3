use super::*;

use beui::reactive::{Frame, build, with_reactive_scope};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

thread_local! {
    static DECODED: Cell<u32> = const { Cell::new(0) };
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Counted(Vec<u8>);

impl Serialize for Counted {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Counted {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DECODED.with(|decoded| decoded.set(decoded.get() + 1));
        Vec::deserialize(deserializer).map(Self)
    }
}

fn decoded() -> u32 {
    DECODED.with(Cell::get)
}

struct Windows;

impl HostValue for Windows {
    const KEY: &'static str = "test.windows";
    type Value = Vec<u32>;
}

struct Icons;

impl HostValue for Icons {
    const KEY: &'static str = "test.icons";
    type Value = Counted;
}

#[test]
fn a_host_value_wakes_only_the_watchers_of_its_own_key() {
    let host = EditorHost::default();
    let editor = Editor::new(host.clone(), Uuid::new_v4());
    let mut document = build(|| Frame().build());
    let (windows, icons) = with_reactive_scope(&mut document, || {
        (editor.host_value::<Windows>(), editor.host_value::<Icons>())
    });
    host.set_host_value::<Icons>(&Counted(vec![7; 4096]));
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert_eq!(icons.get_untracked(), Counted(vec![7; 4096]));
    let before = decoded();

    host.set_host_value::<Windows>(&vec![1, 2]);
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert_eq!(windows.get_untracked(), vec![1, 2]);
    assert_eq!(
        decoded(),
        before,
        "a push of the windows does not decode the icons again"
    );

    host.set_host_value::<Icons>(&Counted(vec![7; 4096]));
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert_eq!(decoded(), before, "the same icons again decode nothing");

    host.set_host_value::<Icons>(&Counted(vec![9; 16]));
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert_eq!(icons.get_untracked(), Counted(vec![9; 16]));
    assert_eq!(decoded(), before + 1, "new icons decode once");
}
