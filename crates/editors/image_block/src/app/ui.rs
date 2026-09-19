use std::rc::Rc;

use block_client::blocks::image::{Image as ImageBlock, ImageOperation};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Picture, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::beui::{ImageFit, NodeId};
use block_editor_plugin::{Creation, Editor, Sidebar};

use super::chooser::Chooser;
use super::picture::watch;

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;

#[component]
pub fn ImageEditor(editor: Editor) -> NodeId {
    let block = editor.block::<ImageBlock>();
    let shown = watch(&editor, &block);
    let image = create_memo(clone!(shown -> move || shown.get().image));
    let failed = create_memo(clone!(shown -> move || shown.get().error.is_some()));
    let reason = create_memo(clone!(shown -> move || shown.get().error.unwrap_or_default()));

    let chooser = Chooser::new();
    let polled = Rc::clone(&chooser);
    let host = editor.host().clone();
    let operating = Rc::clone(&block);
    editor.each_frame(move || {
        polled.poll(&host);
        if let Some(image) = polled.take() {
            operating.operate(ImageOperation::Replace { image });
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(clone!(read_only busy -> move || read_only.get() || busy.get()));
    let failure = chooser.error();
    let refused = create_memo(clone!(failure -> move || failure.get().is_some()));
    let refusal = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let opening = editor.host().clone();
    let choose = clone!(chooser -> move || chooser.open(&opening));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    let danger = theme.danger.clone();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <List align=Align::Center spacing=0.0>
                        <Picture
                            @sizing=ItemSize::Percent(100.0)
                            image={image}
                            fit=ImageFit::Contain
                            @test_id={"image.picture"}
                        />
                        <Show condition={failed}>
                            <Caption content={reason} color={danger} />
                        </Show>
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <Heading content="Image" />
                    <Button
                        label="Replace image…"
                        variant=ButtonVariant::Secondary
                        disabled={blocked}
                        @test_id={"image.replace"}
                        on_click={choose}
                    />
                    <Show condition={refused}>
                        <Caption content={refusal} color={theme.danger.clone()} />
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
pub fn ImagePreview(editor: Editor) -> NodeId {
    let block = editor.block::<ImageBlock>();
    let shown = watch(&editor, &block);
    let image = create_memo(move || shown.get().image);
    view! {
        <Picture image={image} fit=ImageFit::Contain />
    }
}

#[component]
pub fn ImageCreation(creation: Creation) -> NodeId {
    let chooser = Chooser::new();
    let polled = Rc::clone(&chooser);
    let host = creation.host().clone();
    creation.each_frame(move || {
        polled.poll(&host);
        host.set_creation_ready(polled.peek());
    });
    let client = creation.client().clone();
    let made = Rc::clone(&chooser);
    creation.on_create(move || {
        let image = made.take().ok_or("no file was chosen")?;
        Ok(client.create_block(image).id())
    });

    let busy = chooser.busy();
    let chosen = chooser.name();
    let name = create_memo(clone!(chosen -> move || {
        chosen.get().unwrap_or_else(|| "No file chosen".to_owned())
    }));
    let failure = chooser.error();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let opening = creation.host().clone();
    let choose = clone!(chooser -> move || chooser.open(&opening));
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        label="Choose file…"
                        variant=ButtonVariant::Primary
                        disabled={busy}
                        @test_id={"image.choose"}
                        on_click={choose}
                    />
                    <Caption content={name} />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <Show condition={failed}>
                    <Caption content={reason} color={theme.danger.clone()} />
                </Show>
            </List>
        </Frame>
    }
}
