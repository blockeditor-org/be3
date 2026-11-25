use std::rc::Rc;

use be_sdk_beui::ChannelEvent;
use be_sdk_beui::Editor;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_gem_hunt::CELLS;

use super::field::{Field, Look};
use crate::wire::{Ask, Board, Dug, Found, decode, encode};

const PADDING: Space = Space::Xl2;

/// The hunt as a player sees it: holding no key of the block, it sees only what the host
/// tells it on the block's host channel, and digs by asking the host.
#[component]
pub fn Players(editor: Editor) -> NodeId {
    // Whether a host answered, and the world as it last told it.
    let open = create_signal(false);
    let board = create_signal(None::<Board>);
    editor.on_channel(move |event| match event {
        ChannelEvent::Open(_) => open.set(true),
        ChannelEvent::Closed => {
            open.set(false);
            board.set(None);
        }
        ChannelEvent::Said(payload) => {
            if let Some(told) = decode::<Board>(&payload) {
                board.set(Some(told));
            }
        }
    });
    let looks = create_memo(move || {
        board.with(|board| {
            let playing = open.get() && board.as_ref().is_some_and(|board| !board.over());
            (0..CELLS)
                .map(
                    |cell| match board.as_ref().and_then(|board| board.dug(cell)) {
                        Some(Dug {
                            found: Found::Gem, ..
                        }) => Look::Gem,
                        Some(Dug {
                            found: Found::Near(near),
                            ..
                        }) => Look::Near(near),
                        None => Look::Hidden { diggable: playing },
                    },
                )
                .collect::<Vec<_>>()
        })
    });
    let status = create_memo(move || {
        board.with(|board| match (open.get(), board) {
            (false, _) => "Waiting for someone to host the hunt.".to_owned(),
            (true, None) => "Joining the hunt.".to_owned(),
            (true, Some(board)) if board.over() => {
                format!("Every gem is found. You found {}.", board.yours)
            }
            (true, Some(board)) => format!(
                "{} of {} gems found. You found {}.",
                board.found, board.gems, board.yours
            ),
        })
    });
    let dig: Rc<dyn Fn(u16)> = Rc::new(move |cell| editor.tell_host(encode(&Ask::Dig(cell))));
    view! {
        <Frame padding=PADDING>
            <List spacing=Space::Xl>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::HEADING>"Gem hunt"</Text>
                    <Text @test_id={"hunt.status"}>{status}</Text>
                    <Text style=TextStyle::CAPTION>
                        "Dig a cell to find a gem, or how many gems lie around it."
                    </Text>
                </List>
                <Field looks dig />
            </List>
        </Frame>
    }
}
