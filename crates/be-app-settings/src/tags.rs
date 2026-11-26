use be_sdk_beui::be_block::{SettingsContent, TAGS_TYPE, Tag, TagColor, TagContent, TagsContent};
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::icons::{ICON_PLUS, ICON_TRASH};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{BlockQuery, Editor, Loaded};
use be_sdk_database::tags::TagDot;
use beui::{Align, Direction};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;
const NAME_WIDTH: Size = Size::Custom(110.0);
const COLOR_WIDTH: Size = Size::Custom(100.0);

pub struct TagsRoot;

#[component]
impl be_sdk_beui::BeuiRoot for TagsRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <TagsView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

fn owner_name(editor: Editor) -> Option<String> {
    let blocks = editor.blocks();
    let parent = blocks.info(editor.block_id())?.parent?.block()?;
    let held = blocks.info(parent)?;
    if held.block_type == SettingsContent::CONTENT_TYPE {
        return None;
    }
    Some(held.label(editor.block_types().as_ref()).name)
}

#[component]
pub fn TagsView(editor: Editor) -> NodeId {
    let block = editor.block_id();
    let _watched = store((
        editor.blocks().watch(BlockQuery::Block(block)),
        editor.blocks().watch(BlockQuery::Parents(block)),
    ));
    let tags = editor.block_content::<TagsContent>();
    let ids = tags.project(|tags| tags.root().tags());
    let editable = editor.editable();
    let description = move || match owner_name(editor) {
        Some(owner) => format!(
            "Anyone who can edit {owner} puts these on the blocks in it, and anyone who can view a block there sees them."
        ),
        None => "Your own tags. Only you see them, on any block you can edit.".to_owned(),
    };
    let name = create_signal(String::new());
    let add = move || {
        let wanted = name.get_untracked();
        if wanted.trim().is_empty() {
            return;
        }
        let color = TagColor::ALL[ids.with_untracked(Vec::len) % TagColor::ALL.len()];
        editor.add_tag(editor.block_id(), &wanted, color);
        name.set(String::new());
    };
    view! {
        <SettingsPage>
            <SettingsSection title="Tags" description={description}>
                <ForEach
                    value={ids}
                    fallback={|| view! {
                        <SettingsRow label="No tags yet." compact=true />
                    }}
                >|tag: Uuid|
                    <TagRow editor={editor} tag={tag} />
                </ForEach>
                <SettingsRow label="New tag">
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <TextInput
                            value={Binding::new(name, move |text: String| name.set(text))}
                            placeholder="Name"
                            disabled={move || !editable.get()}
                            @test_id={"tags.new-name"}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_PLUS
                            label="Add tag"
                            disabled={move || !editable.get() || name.with(|name| name.trim().is_empty())}
                            @test_id={"tags.add"}
                            on_click={add}
                        />
                    </List>
                </SettingsRow>
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn TagRow(editor: Editor, tag: Uuid) -> NodeId {
    let content = editor.content_of::<TagContent>(tag);
    let name = content.project(|content| content.root().name.clone());
    let color = content.project(|content| content.root().color);
    let editable = editor.editable();
    let rename = move |text: String| content.operate(Tag::rename(&text));
    let selected = create_memo(move || Some(color.get()));
    let recolor = move |chosen: Option<TagColor>| {
        if let Some(color) = chosen {
            content.operate(Tag::recolor(color));
        }
    };
    let remove = move || editor.remove_tag(editor.block_id(), tag);
    let found = editor.watch_blocks(BlockQuery::Backrefs(tag));
    let tagged = create_memo(move || {
        found
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|info| info.block_type != TAGS_TYPE && !info.deleted)
            .map(|info| info.id)
            .collect::<Vec<_>>()
    });
    let count = move || match tagged.with(Vec::len) {
        0 => "On no blocks".to_owned(),
        1 => "On 1 block".to_owned(),
        count => format!("On {count} blocks"),
    };
    view! {
        <List spacing=Space::None>
            <SettingsRow label={name} description={count}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <TagDot color={color} size=TextSize::Base />
                    <Frame width=NAME_WIDTH>
                        <TextInput
                            value={Binding::new(name, rename)}
                            disabled={move || !editable.get()}
                            @test_id={format!("tags.tag.{tag}.name")}
                        />
                    </Frame>
                    <Frame width=COLOR_WIDTH>
                        <Select
                            label="Color"
                            options={view! {
                                <ForEach value={TagColor::ALL.to_vec()}>|color: TagColor|
                                    <ChoiceOption value=color label={color.label().to_owned()} />
                                </ForEach>
                            }}
                            selected={Binding::new(selected, recolor)}
                            disabled={move || !editable.get()}
                            @test_id={format!("tags.tag.{tag}.color")}
                        />
                    </Frame>
                    <Button
                        icon_only=true
                        glyph=ICON_TRASH
                        label="Remove tag"
                        disabled={move || !editable.get()}
                        @test_id={format!("tags.tag.{tag}.remove")}
                        on_click={remove}
                    />
                </List>
            </SettingsRow>
            <ForEach value={tagged}>|block: Uuid|
                <TaggedBlock editor={editor} block={block} />
            </ForEach>
        </List>
    }
}

#[component]
fn TaggedBlock(editor: Editor, block: Uuid) -> NodeId {
    let label = move || {
        let types = editor.block_types();
        editor.blocks().info(block).map_or_else(
            || "A block you can't open".to_owned(),
            |info| info.label(types.as_ref()).name,
        )
    };
    let open = move || {
        if let Some(info) = editor.blocks().info(block) {
            editor.host().open_block(block, info.block_type);
        }
    };
    view! {
        <SettingsRow label={label} compact=true>
            <Button
                label="Open"
                variant=ButtonVariant::Ghost
                @test_id={format!("tags.block.{block}")}
                on_click={open}
            />
        </SettingsRow>
    }
}
