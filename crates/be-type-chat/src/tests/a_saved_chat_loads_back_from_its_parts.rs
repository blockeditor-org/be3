use be_block::{Part, Stored};

use super::*;

#[test]
fn a_saved_chat_loads_back_from_its_parts() {
    let chat = sent(&ChatContent::default(), ADA, &["one", "two"]);
    let chat = edited(&chat, [ChatContent::react(first(&chat), BO, "eye", 1)]);

    let Part::Node { parts, .. } = chat.parts() else {
        panic!("a chat saves as a node of parts");
    };
    assert_eq!(parts.len(), 1, "a chat saves only its header so far");
    assert_eq!(chat.parts().flatten(), Some(chat.encode()));

    let loaded = ChatContent::from_stored(&Stored::unsaved(chat.encode()))
        .expect("a chat loads its stored form");
    assert_eq!(loaded, chat);
    assert_eq!(round_trip(&chat), chat);
}
