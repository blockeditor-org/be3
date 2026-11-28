use std::sync::Arc;

use be_lib_graphing_calculator::Expression;
use be_sdk_beui::be_block_content::ObjectId;
use be_sdk_beui::be_block_content::presence::{PresenceColor, PresenceKind, pick_free_color};
use be_sdk_beui::Editor;
use be_sdk_text::live_text::{FieldOf, LiveText, Synced};
use be_sdk_text::{SharedCaret, remote_carets};
use be_type_graphing_calculator::{Equation, Graph};
use beui::icons::{ICON_GRIP_VERTICAL, ICON_PLUS, ICON_TRASH};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, CursorIcon, Direction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::GraphProjection;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct EquationCaret {
    equation: ObjectId,
    caret: SharedCaret,
    color: PresenceColor,
}

impl PresenceKind for EquationCaret {
    const ID: Uuid = Uuid::from_u128(0x7e9b_e3a2_659c_48cb_a536_429c_eb32_5cb3);
}

#[component]
pub(super) fn Equations(editor: Editor, content: GraphProjection) -> NodeId {
    let ids = content.ids(ObjectId::ROOT, Graph::EQUATIONS);
    let text = move |id| {
        content.read(|graph| {
            graph.root().equations.get(id).map(|row| row.expression.to_str_lossy().into_owned())
        }).ok().flatten().filter(|text| !text.is_empty()).unwrap_or_else(|| "Equation".into())
    };
    let moved = move |moved: ReorderMove<ObjectId, ObjectId>| {
        if editor.editable().get_untracked() && ids.with_untracked(|ids| {
            ids.contains(&moved.payload)
                && moved.after.is_none_or(|after| after != moved.payload && ids.contains(&after))
        }) {
            content.break_step();
            content.operate(Graph::move_after(moved.payload, moved.after));
            content.break_step();
        }
    };
    let active = create_signal(None::<ObjectId>);
    let caret = create_signal(None::<EquationCaret>);
    let visible = editor.presence_visible();
    let peers = editor.peers::<EquationCaret>();
    let color = pick_free_color(peers.get_untracked().into_iter().map(|(_, peer)| peer.color));
    create_effect(move || {
        let outgoing = caret.get().filter(|caret| visible.get() && active.get() == Some(caret.equation));
        editor.show(outgoing.as_ref());
    });
    on_cleanup(move || editor.show::<EquationCaret>(None));

    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll>
                <Frame padding=Space::Md>
                    <List spacing=Space::Sm>
                        <Button
                            glyph=ICON_PLUS
                            label="Add equation"
                            disabled={editor.read_only()}
                            on_click={move || change(editor, content, Graph::add(""))}
                            @test_id={"graphing-calculator.add"}
                        />
                        <Reorder value={ids} payload={Func::new(Some)} disabled={editor.read_only()}
                            lifts_on_long_press=true text={text} on_move={moved}
                            preview={move |id| view! { <Frame fill=ColorRole::Control padding=Space::Sm><Text>{text(id)}</Text></Frame> }}
                        >|reorder: ReorderHandle<ObjectId, ObjectId>|
                            <List spacing=Space::Xs>
                                <ForEach value={reorder.order} handle={reorder.rows}>|id: ObjectId|
                                    <EquationRow editor content reorder id active caret peers color />
                                </ForEach>
                            </List>
                        </Reorder>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

fn change(editor: Editor, content: GraphProjection, edit: be_sdk_beui::be_block_content::Edit) {
    if editor.editable().get_untracked() {
        content.break_step();
        content.operate(edit);
        content.break_step();
    }
}

#[component]
fn EquationRow(
    editor: Editor,
    content: GraphProjection,
    reorder: ReorderHandle<ObjectId, ObjectId>,
    id: ObjectId,
    active: Signal<Option<ObjectId>>,
    caret: Signal<Option<EquationCaret>>,
    peers: ReadSignal<Vec<(u64, EquationCaret)>>,
    color: PresenceColor,
) -> NodeId {
    let document = Arc::new(LiveText::new(
        FieldOf::<Graph>::plain(id, Equation::EXPRESSION),
        editor.host().waker(),
    ));
    let area = TextAreaState::new(document.clone() as Arc<dyn beui_text_editor_core::Document>);
    let expression = content.field(id, Equation::EXPRESSION);
    let failure = create_memo(move || expression.with(|expression| {
        let text = expression.to_str_lossy();
        if text.trim().is_empty() {
            None
        } else {
            Expression::parse(&text).err()
        }
    }));

    create_effect(move || {
        area.content().track();
        content.track();
        untrack(|| {
            // LiveContent's projected operation log has one reader. Each equation instead
            // adopts its own text sequence, which retains the IDs used by peer carets.
            if document.sync(&content.live(), &editor.host(), editor.block_id(), true) != Synced::Waiting {
                area.sync();
            }
        });
    });

    let visible = editor.presence_visible();
    let remote = create_memo(move || {
        area.content().track();
        if !visible.get() {
            return Vec::new();
        }
        remote_carets(area, peers.get().into_iter().filter_map(|(_, peer)| {
            (peer.equation == id).then_some((peer.caret, peer.color))
        }))
    });
    create_effect(move || {
        if active.get() == Some(id) {
            area.cursors().track();
            caret.set(SharedCaret::of(area).map(|caret| EquationCaret { equation: id, caret, color }));
        }
    });
    let focused = move |focused: bool| {
        if active.is_alive() {
            if focused {
                active.set(Some(id));
            } else if active.get_untracked() == Some(id) {
                active.set(None);
            }
        }
    };
    on_cleanup(move || focused(false));

    view! {
        <ReorderItem reorder item=id cursor=CursorIcon::Grab>|_: ReorderItemHandle|
            <Frame fill=ColorRole::Control radius=Radius::Sm padding=Space::Xs>
                <List spacing=Space::Xs>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text color=TextColor::Muted><Icon glyph=ICON_GRIP_VERTICAL /></Text>
                        <ListItem grow=1.0>
                            <Frame height={Size::Custom(40.0)}>
                                <TextArea state=area line_numbers=false find=false placeholder="y = f(x)"
                                    read_only={editor.read_only()} remote_cursors={remote}
                                    on_focus_change={focused}
                                    @test_id={format!("graphing-calculator.equation.{id}")}
                                />
                            </Frame>
                        </ListItem>
                        <Button icon_only=true glyph=ICON_TRASH label="Remove equation"
                            size=ButtonSize::Small variant=ButtonVariant::Ghost
                            capture_presses=true disabled={editor.read_only()}
                            on_click={move || change(editor, content, Graph::remove(id))}
                        />
                    </List>
                    <Show value={move || failure.get().map(str::to_owned)}>|message: ReadSignal<String>|
                        <Text size=TextSize::Sm color=TextColor::Danger>{message}</Text>
                    </Show>
                </List>
            </Frame>
        </ReorderItem>
    }
}
