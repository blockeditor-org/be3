use std::rc::Rc;

use be_sdk_beui::Editor;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_gem_hunt::{CELLS, HuntContent};
use beui::{Align, Direction};

use super::field::{Field, Look};

const PADDING: Space = Space::Xl2;

/// The world as one who may read the block sees it: where every gem lies, dug or not. A
/// player sees only what the host tells it (`players::Players`).
#[component]
pub fn World(editor: Editor) -> NodeId {
    let hunt = editor.block_content::<HuntContent>();
    let world = hunt.project(|content| content.root());
    let looks = create_memo(move || {
        world.with(|world| {
            (0..CELLS)
                .map(
                    |cell| match (world.dug.contains_key(&cell), world.has_gem(cell)) {
                        (true, true) => Look::Gem,
                        (true, false) => Look::Near(world.near(cell)),
                        (false, true) => Look::Buried,
                        (false, false) => Look::Hidden { diggable: false },
                    },
                )
                .collect::<Vec<_>>()
        })
    });
    let summary = create_memo(move || {
        world.with(|world| match (world.over(), world.gems.len()) {
            (true, _) => "Every gem is found.".to_owned(),
            (false, gems) => format!("{} of {gems} gems found.", world.found()),
        })
    });
    let editable = editor.editable();
    let new_world = move || {
        if let Ok(edit) = hunt.read(|content| content.root().new_world(super::seed())) {
            hunt.operate(edit);
        }
    };
    view! {
        <Frame padding=PADDING>
            <List spacing=Space::Xl>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::HEADING>"Gem hunt"</Text>
                    <Text @test_id={"hunt.summary"}>{summary}</Text>
                    <Text style=TextStyle::CAPTION>
                        "You see where every gem lies, as the hunt's hosts do. Players see only the cells that are dug."
                    </Text>
                </List>
                <Field looks dig={Rc::new(|_| {}) as Rc<dyn Fn(u16)>} />
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Button
                        label="New world"
                        variant=ButtonVariant::Secondary
                        disabled={move || !editable.get()}
                        @test_id={"hunt.new-world"}
                        on_click={new_world}
                    />
                </List>
            </List>
        </Frame>
    }
}
