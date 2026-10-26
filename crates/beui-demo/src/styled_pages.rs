use super::*;
use beui::KeyAt;
use beui::TextAlign;
use beui::icons::Glyph;
use beui::icons::{
    ICON_A_LARGE_SMALL, ICON_CIRCLE_CHECK, ICON_CLOCK, ICON_COPY_PLUS, ICON_FILE_UP, ICON_FOLDER,
    ICON_IMAGE, ICON_INFO, ICON_PAPERCLIP, ICON_PLUS, ICON_QUOTE, ICON_SAVE, ICON_TABLE,
    ICON_TEXT_ALIGN_START, ICON_TRIANGLE_ALERT, ICON_UNDO_2, ICON_WIFI, ICON_X,
};
use beui::keyboard::styled::SoftwareKeyboard;
use beui::unstyled::MenuThen;

const COLUMN_SPACING: Space = Space::Xl2;
const COLUMNS_BREAKPOINT: f32 = 460.0;
const NARROW_CHOICE_WIDTH: f32 = 180.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const TOOLBAR_RULE_LENGTH: Size = Size::Sm;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_MIDPOINT: f32 = 1.0;
const TREE_HEIGHT: f32 = 240.0;
const DEFAULT_VOLUME: f32 = 0.6;
const NESTED_SCROLL_HEIGHT: f32 = 160.0;
const NESTED_SCROLL_ROWS: usize = 30;
const SWATCH_HEIGHT: Size = Size::Custom(28.0);
const PROGRESS_WIDTH: Size = Size::Custom(120.0);
const DIALOG_WIDTH: Size = Size::Dialog;
const LAUNCHER_ICON_PIXELS: u32 = 64;
const LAUNCHER_ICON_BORDER: u32 = 6;
const ROW_COUNT: usize = 10_000;
const ROWS_HEIGHT: f32 = 360.0;
const ROW_HEIGHT: f32 = 34.0;
const COMPACT_ROW_HEIGHT: f32 = 25.0;
const ROW_PADDING_HORIZONTAL: Space = Space::Lg;
const ROW_PADDING_VERTICAL: Space = Space::Md;
const COMPACT_ROW_PADDING_VERTICAL: Space = Space::Xs;
const STRIP_COUNT: usize = 200;
const STRIP_HEIGHT: f32 = 52.0;
const STRIP_ITEM_WIDTH: f32 = 120.0;
const FITTED_SCROLL_HEIGHT: f32 = 200.0;
const FITTED_SCROLL_ROWS: usize = 3;
const LOG_HEIGHT: Size = Size::Custom(200.0);
const LOG_LINE_ESTIMATE: f32 = 20.0;
const LOG_FIRST_LINES: usize = 12;
const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_millis(400);
const LOG_WORDS: [&str; 8] = [
    "fetched",
    "the index",
    "and compiled",
    "four crates,",
    "then linked",
    "the binary",
    "in",
    "a moment.",
];
const SERVER_LINES: usize = 5_000;
const SERVER_LOG_HEIGHT: f32 = 200.0;
const SERVER_LINE_HEIGHT: f32 = 20.0;
const SERVER_MESSAGES: [&str; 6] = [
    "listening on 127.0.0.1:8080",
    "GET /index.html 200",
    "GET /style.css 304",
    "cache miss for /api/rows?page=3",
    "POST /api/rows 201",
    "worker 2 idle",
];
const EDITOR_HEIGHT: f32 = 320.0;
const STAGE_HEIGHT: f32 = 360.0;
const STAGE_VIEW: PanZoomView = PanZoomView::new(beui::pos2(200.0, 120.0), 0.8);
const STAGE_CARDS: [(f32, f32, f32, f32, &str); 4] = [
    (0.0, 0.0, 170.0, 90.0, "Inbox"),
    (230.0, 30.0, 150.0, 80.0, "Notes"),
    (50.0, 150.0, 210.0, 110.0, "Canvas"),
    (320.0, 190.0, 130.0, 70.0, "Archive"),
];
const STAGE_ZOOM_STEP: f32 = 1.3;
const STAGE_LABEL_SIZE: f32 = 15.0;
const STAGE_PADDING: Space = Space::Md;
const EDITOR_TEXT: &str = "# Notes\n\nA **multiline** editor with a gutter, wrapping and `markdown`\nstyling.\n\n- [x] click a checkbox\n- [ ] press Ctrl+F to find\n- [ ] type :smile to pick an emoji\n";
const TREE_NODES: [(&str, usize); 9] = [
    ("Project", 0),
    ("src", 1),
    ("main.rs", 2),
    ("demo.rs", 2),
    ("assets", 1),
    ("logo.png", 2),
    ("theme.toml", 2),
    ("README.md", 1),
    ("Cargo.toml", 1),
];
const TOAST_AREA_HEIGHT: Size = Size::Custom(380.0);
const FRUITS: [&str; 6] = ["Apple", "Banana", "Cherry", "Date", "Grape", "Mango"];

#[sample]
#[component]
pub(crate) fn LabelledSwitch(label: &'static str, on: Binding<bool>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Switch label on />
            <Text>{label}</Text>
        </List>
    }
}

#[component]
pub(crate) fn DockingPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="The dock around this page" code={vec![DemoShell::SOURCE]}>
                <List spacing=SECTION_SPACING>
                    <Text style=TextStyle::PARAGRAPH>
                        "Drag a tab by its label. Drop it over the middle of a pane to join \
                         that pane, over an edge to split it, or over a tab bar to land between the \
                         tabs there. With a finger, pull the tab down out of its bar first: sliding \
                         along the bar scrolls it. The arrow keys walk a tab bar, and Ctrl+Tab and \
                         Ctrl+Shift+Tab walk the tabs of whichever pane you are in."
                    </Text>
                    <Text style=TextStyle::PARAGRAPH>
                        "Drag the bar between two panes to resize them, or focus it with Tab \
                         and use the arrow keys. Drag the grip at the start of a pane's bar to move \
                         the whole pane, or right-click it to move the tabs into a sidebar and back, \
                         or to show only their icons. Dragging a sidebar's edge on past its \
                         narrowest shows only icons too, and any drag back the other way shows \
                         the titles again."
                    </Text>
                    <Text style=TextStyle::PARAGRAPH>
                        "Hold Alt while dragging a tab, or right-click one and pop it out, to \
                         float it in a window. A window moves by the grip at the left of its bar and \
                         resizes from any edge."
                    </Text>
                    <Text style=TextStyle::PARAGRAPH>
                        "Drop a tab onto the middle of another tab to group them: the group \
                         becomes one tab, and choosing it shows a second tab bar holding the tabs \
                         inside. A group ungroups from its own right-click menu."
                    </Text>
                    <Text style=TextStyle::PARAGRAPH>
                        "A page picked in Components takes the place of the page shown, in \
                         the same tab. Hold Ctrl (Cmd on a Mac) while picking one to open it in a \
                         tab of its own instead."
                    </Text>
                    <Text style=TextStyle::CAPTION>
                        "A dock narrower than a phone's width stacks its panes: one tab fills it, and the bar goes back or switches tabs."
                    </Text>
                </List>
            </Sample>
            <Sample title="Switching tabs" code={vec![switch_tabs_with_alt_q::SOURCE]}>
                <Text style=TextStyle::PARAGRAPH>
                    "Hold Alt and press Q to switch between the tabs you looked at last, \
                     in every pane and window: each Q moves one further back, Shift+Q one \
                     forward, letting go of Alt shows the one chosen, and Escape with Alt still \
                     held keeps the tab you were on. The dock binds no key to this itself; the demo binds Alt+Q \
                     where a desktop binds Alt+Tab."
                </Text>
            </Sample>
            <Sample title="State follows the tab" code={vec![EditCounter::SOURCE]}>
                <EditCounter />
            </Sample>
            <Sample title="Fullscreen" code={vec![FullscreenTab::SOURCE]}>
                <FullscreenTab />
            </Sample>
            <Sample title="Moving windows by their content" code={vec![ContentDrag::SOURCE]}>
                <ContentDrag />
            </Sample>
            <Sample
                title="Dialogs attached to a tab"
                code={vec![AttachedDialogs::SOURCE, AttachedDialog::SOURCE, AttachedCard::SOURCE]}
            >
                <AttachedDialogs />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn AttachedDialogs() -> NodeId {
    let dialogs = use_context::<DemoDialogs>();
    let tab = use_dock_tab().map(|control| control.tab());
    let pointing = NodeRef::new();
    let open_card = move |anchor: Option<Rect>| {
        if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
            dialogs.open_card(tab, anchor);
        }
    };
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
                <Button
                    @test_id="demo.dialog.open"
                    label="Open a dialog"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
                            dialogs.open(tab, false);
                        }
                    }}
                />
                <Button
                    @test_id="demo.dialog.modal"
                    label="Open a modal dialog"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
                            dialogs.open(tab, true);
                        }
                    }}
                />
                <Button
                    @test_id="demo.dialog.card"
                    label="Open a card"
                    variant=ButtonVariant::Secondary
                    on_click={move || open_card(None)}
                />
                <Button
                    @test_id="demo.dialog.pointing"
                    @node_ref=&pointing
                    label="Open a card from here"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        open_card(pointing.try_get().and_then(|button| {
                            with_document(|document| document.node_rect(button))
                        }))
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "A dialog hangs from the tab that opened it: it floats in the middle of \
                 that tab, hides when another tab is shown in its place and comes back with it, \
                 and is never a tab of its own. A modal one takes every press and key meant \
                 for its tab until it closes, and the focus goes back to the tab when it does."
            </Text>
            <Text style=TextStyle::CAPTION>
                "A card floats just under the tab bar, centred on the tab but free to \
                 reach past its pane, dims the tab and waits for a choice, with a dot on the tab \
                 meanwhile. Pressing the dimmed tab closes it. Other tabs and panes stay usable, \
                 and the card keeps what was done in it while another tab is shown. On a phone it \
                 is a sheet from the bottom of the screen."
            </Text>
            <Text style=TextStyle::CAPTION>
                "A card opened from a control points at it instead, like a popover: it \
                 opens under the control, or above it where there is no room below, and leaves \
                 the tab undimmed. Pressing elsewhere on the tab closes it, and it keeps its \
                 place on the tab when the pane is moved or resized."
            </Text>
        </List>
    }
}

#[sample]
#[component]
pub(crate) fn AttachedDialog(modal: bool) -> NodeId {
    let dialogs = use_context::<DemoDialogs>();
    let control = use_dock_tab();
    let tab = control.as_ref().map(DockTabControl::tab);
    let message = match modal {
        true => "This tab cannot be used until this dialog closes.",
        false => "The tab under this dialog can still be used.",
    };
    let id = tab.map_or(0, TabId::value);
    view! {
        <Frame padding=PAGE_PADDING>
            <List spacing=SECTION_SPACING>
                <Text>{message}</Text>
                <List direction=Direction::Horizontal spacing=ROW_SPACING>
                    <Button
                        @test_id={format!("demo.dialog.{id}.nested")}
                        label="Open a modal dialog over this one"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
                                dialogs.open(tab, true);
                            }
                        }}
                    />
                    <Button
                        @test_id={format!("demo.dialog.{id}.close")}
                        label="Close"
                        variant=ButtonVariant::Primary
                        on_click={move || {
                            if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
                                dialogs.close(tab);
                            }
                        }}
                    />
                </List>
            </List>
        </Frame>
    }
}

#[sample]
#[component]
pub(crate) fn AttachedCard() -> NodeId {
    let dialogs = use_context::<DemoDialogs>();
    let tab = use_dock_tab().map(|control| control.tab());
    let id = tab.map_or(0, TabId::value);
    let presses = create_signal(0_u32);
    let label = create_memo(move || match presses.get() {
        0 => "Press me".to_owned(),
        count => format!("Pressed {count} times"),
    });
    let close = move || {
        if let (Some(dialogs), Some(tab)) = (dialogs.as_ref(), tab) {
            dialogs.close(tab);
        }
    };
    view! {
        <List spacing=Space::None>
            <Frame padding=PAGE_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>
                        "Choose something"
                    </Text>
                    <Button
                        icon_only=true
                        @test_id={format!("demo.card.{id}.close")}
                        glyph=ICON_X
                        label="Close"
                        variant=ButtonVariant::Ghost
                        on_click={close}
                    />
                </List>
            </Frame>
            <Separator />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=PAGE_PADDING>
                    <List spacing=SECTION_SPACING>
                        <Text>
                            "Only the tab under this card waits for it. Show another tab \
                             and come back: the count below is kept."
                        </Text>
                        <Button
                            @test_id={format!("demo.card.{id}.count")}
                            label={label}
                            variant=ButtonVariant::Secondary
                            on_click={move || presses.update(|presses| *presses += 1)}
                        />
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[sample]
#[component]
fn ContentDrag() -> NodeId {
    let modifier = use_context::<DragModifier>();
    let on = create_memo(move || {
        modifier
            .as_ref()
            .is_some_and(|modifier| modifier.held.get().is_some())
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Switch
                    @test_id="demo.drag_modifier"
                    label="Alt moves and resizes windows"
                    on={Binding::new(on, move |on: bool| {
                        if let Some(modifier) = modifier.as_ref() {
                            modifier.held.set(on.then_some(Modifiers::ALT));
                        }
                    })}
                />
                <Text>"Alt moves and resizes windows"</Text>
            </List>
            <Text style=TextStyle::CAPTION>
                "A dock can be given a drag modifier, which is off unless asked for. \
                 While it is held, dragging anywhere in a window, its bar and tabs included, \
                 moves the whole window, and dragging with the right button resizes it from \
                 its nearest edge or corner, or moves the split nearest the pointer in a docked \
                 pane. Nothing in the dock or its tabs sees those presses. The Linux desktop \
                 uses Super."
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn EditCounter() -> NodeId {
    let count = create_signal(0u32);
    let counted = create_memo(move || format!("{} edits", count.get()));
    let moved = create_memo(move || count.get() > 0);
    let edit = Action::new("demo.edit", "Edit", move || {
        count.update(|count| *count += 1)
    })
    .glyph(ICON_PENCIL)
    .pin(Pin::Icon)
    .detached();
    let pinned = edit.clone();
    dock_menu(create_memo(move || vec![MenuNode::action(&pinned)]));
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Button action={edit} variant=ButtonVariant::Secondary />
                <Text style=TextStyle::CAPTION>{counted}</Text>
            </List>
            <Show condition={moved}>
                <Text style=TextStyle::CAPTION>
                    "Drag this tab somewhere else: the count comes with it."
                </Text>
            </Show>
        </List>
    }
}

#[sample]
#[component]
fn FullscreenTab() -> NodeId {
    let control = use_dock_tab();
    if let Some(control) = control.as_ref() {
        control.offer_fullscreen();
    }
    let shown = create_memo(move || control.as_ref().is_some_and(|control| control.fullscreen()));
    let label = create_memo(move || match shown.get() {
        true => "Leave fullscreen".to_owned(),
        false => "Show this tab fullscreen".to_owned(),
    });
    on_shortcut(move |press: beui::KeyPress| {
        let leave = press.pressed && press.key == beui::Key::Escape && shown.get_untracked();
        if let Some(control) = control.as_ref().filter(|_| leave) {
            control.leave_fullscreen();
        }
        leave
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=ROW_SPACING>
                <Button
                    @test_id="demo.fullscreen"
                    label
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        let Some(control) = control.as_ref() else {
                            return;
                        };
                        match shown.get_untracked() {
                            true => control.leave_fullscreen(),
                            false => control.enter_fullscreen(None),
                        }
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "The tab covers the whole window, its bar and every pane around it. \
                 Escape or the same button brings it back. Full screen in the tab's \
                 right-click menu shows it the same way."
            </Text>
        </List>
    }
}

#[component]
pub(crate) fn TextPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Text styles" code={vec![TypeScale::SOURCE]}>
                <TypeScale />
            </Sample>
            <Sample title="Sizes" code={vec![TextSizes::SOURCE]}>
                <TextSizes />
            </Sample>
            <Sample title="Colour roles and weights" code={vec![TextColors::SOURCE]}>
                <TextColors />
            </Sample>
            <Sample title="Icons in text" code={vec![InlineIcons::SOURCE]}>
                <InlineIcons />
            </Sample>
            <Sample title="Inherited style" code={vec![InheritedStyle::SOURCE]}>
                <InheritedStyle />
            </Sample>
            <Sample title="Icons and chips" code={vec![IconsAndChips::SOURCE]}>
                <IconsAndChips />
            </Sample>
            <Sample title="Shortcuts" code={vec![Shortcuts::SOURCE]}>
                <Shortcuts />
            </Sample>
            <Sample title="Links" code={vec![Links::SOURCE]}>
                <Links />
            </Sample>
            <Sample title="Selectable text" code={vec![SelectableParagraph::SOURCE]}>
                <SelectableParagraph />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn TypeScale() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::DISPLAY>"Display"</Text>
            <Text style=TextStyle::TITLE>"Title"</Text>
            <Text style=TextStyle::HEADING>"Heading"</Text>
            <Text>"Body text for labels and values."</Text>
            <Text style=TextStyle::CAPTION>"Caption text, muted, for secondary details."</Text>
            <Text style=TextStyle::PARAGRAPH>
                "A paragraph wraps to the width it is given, so it reads well \
                 in a narrow pane as well as a wide one."
            </Text>
            <Text style=TextStyle::CODE>"let answer = 6 * 7;"</Text>
        </List>
    }
}

#[sample]
#[component]
fn TextSizes() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <ForEach keys={TextSize::ALL.to_vec()}>|size: TextSize|
                <Text size>
                    <Icon glyph=ICON_TYPE />
                    {format!("{} {}", size.name(), size.px())}
                </Text>
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn TextColors() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <ForEach keys={TextColor::ROLES.to_vec()}>|color: TextColor|
                <Text color>{format!("The {} role", color.name())}</Text>
            </ForEach>
            <Text>
                "Regular, "
                <Span weight=Weight::Bold>"bold"</Span>
                ", "
                <Span italic=true>"italic"</Span>
                " and "
                <Span family=Family::Mono>"monospace"</Span>
                " in one line."
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn InlineIcons() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text>
                <Icon glyph=ICON_SAVE />
                "Save"
            </Text>
            <Text style=TextStyle::CAPTION>
                <Icon glyph=ICON_CLOCK />
                "Edited five minutes ago"
            </Text>
            <Text size=TextSize::Lg color=TextColor::Success>
                <Icon glyph=ICON_CIRCLE_CHECK />
                "Every check passed"
            </Text>
            <Text>
                "Open"
                <Icon glyph=ICON_EXTERNAL_LINK color=TextColor::Accent />
            </Text>
            <Text>
                "An icon sits in a sentence like a word: the "
                <Icon glyph=ICON_FOLDER />
                "folder and the "
                <Icon glyph=ICON_PAPERCLIP color=TextColor::Muted />
                "attachment keep their room from the text, and a selection never takes them."
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn InheritedStyle() -> NodeId {
    view! {
        <TextStyleProvider style={TextStyle::new().size(TextSize::Sm).color(TextColor::Muted)}>
            <List spacing=SECTION_SPACING>
                <Text>"Small and muted, from the provider around it."</Text>
                <Text weight=Weight::Bold>"Still small and muted, but bold."</Text>
                <Text color=TextColor::Danger>
                    <Icon glyph=ICON_TRIANGLE_ALERT />
                    "Its own colour wins over the provider's."
                </Text>
            </List>
        </TextStyleProvider>
    }
}

#[sample]
#[component]
fn IconsAndChips() -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Text align=TextAlign::Center>
                <Icon glyph=ICON_STAR />
            </Text>
            <Text size=TextSize::Xl color=TextColor::Accent align=TextAlign::Center>
                <Icon glyph=ICON_PALETTE />
            </Text>
            <Chip label="Chip" />
            <Chip label="Another chip" />
        </List>
    }
}

#[sample]
#[component]
fn Shortcuts() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Shortcut keys="Tab" description="move focus to the next control" />
            <Shortcut keys="Shift+Tab" description="move focus back" />
            <Shortcut keys="Enter" description="activate the focused control" />
            <Shortcut keys="Ctrl+Z" description="undo an edit in a text field" />
            <Shortcut keys="Ctrl+Shift+I" description="open the inspector" />
            <Shortcut keys="Ctrl+Shift+C" description="pick a node to inspect" />
        </List>
    }
}

#[sample]
#[component]
fn Links() -> NodeId {
    let followed = create_signal("Nothing followed yet".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xl wrap=true>
                <Link
                    label="Open the grid"
                    glyph=ICON_LAYOUT_GRID
                    on_click={move || followed.set("Followed: Open the grid".to_owned())}
                />
                <Link
                    label="Read the guide"
                    on_click={move || followed.set("Followed: Read the guide".to_owned())}
                />
                <Link label="Unavailable" disabled=true />
            </List>
            <Text style=TextStyle::CAPTION>{followed}</Text>
        </List>
    }
}

#[sample]
#[component]
fn SelectableParagraph() -> NodeId {
    view! {
        <SelectableText child_size=ItemSize::Percent(100.0)>
            <Text style=TextStyle::PARAGRAPH>
                <Icon glyph=ICON_INFO />
                "Drag across this paragraph to select it, or long-press a word with a finger. \
                 Right-click it for Copy and Select All. The icon is not part of the text, so a \
                 copy never carries it."
            </Text>
        </SelectableText>
    }
}

#[component]
pub(crate) fn ButtonsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Buttons" code={vec![ButtonVariants::SOURCE]}>
                <ButtonVariants />
            </Sample>
            <Sample title="Icon buttons" code={vec![IconButtons::SOURCE]}>
                <IconButtons />
            </Sample>
            <Sample title="Row buttons" code={vec![RowButtons::SOURCE, badge::SOURCE]}>
                <RowButtons />
            </Sample>
            <Sample title="Toggle button" code={vec![PinToggle::SOURCE]}>
                <PinToggle />
            </Sample>
            <Sample title="Tooltips" code={vec![HoverTooltip::SOURCE]}>
                <HoverTooltip />
            </Sample>
            <Sample title="Focus ring" code={vec![FocusedTile::SOURCE]}>
                <FocusedTile />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn FocusedTile() -> NodeId {
    let opened = create_signal("Tab to the tile, then press Enter".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=Space::None>
                <beui::unstyled::Button
                    label="Open the album"
                    on_click={move || opened.set("Opened the album".to_owned())}
                >|handle: beui::unstyled::ButtonHandle|
                    <FocusRing focused={handle.focused} radius=Radius::Lg offset=Stroke::Focus>
                        <Card>
                            <Text>{handle.label}</Text>
                        </Card>
                    </FocusRing>
                </beui::unstyled::Button>
            </List>
            <Text style=TextStyle::CAPTION>{opened}</Text>
        </List>
    }
}

#[sample]
#[component]
fn ButtonVariants() -> NodeId {
    let clicked = create_signal("Nothing clicked yet".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md wrap=true>
                <Button
                    label="Primary"
                    variant=ButtonVariant::Primary
                    on_click={move || clicked.set("Clicked Primary".to_owned())}
                />
                <Button
                    label="Secondary"
                    variant=ButtonVariant::Secondary
                    on_click={move || clicked.set("Clicked Secondary".to_owned())}
                />
                <Button
                    label="Danger"
                    variant=ButtonVariant::Danger
                    on_click={move || clicked.set("Clicked Danger".to_owned())}
                />
                <Button
                    label="Ghost"
                    variant=ButtonVariant::Ghost
                    on_click={move || clicked.set("Clicked Ghost".to_owned())}
                />
                <Button
                    label="With an icon"
                    glyph=ICON_PLAY
                    variant=ButtonVariant::Secondary
                    on_click={move || clicked.set("Clicked With an icon".to_owned())}
                />
                <Button label="Compact" variant=ButtonVariant::Secondary size=ButtonSize::Compact />
                <Button label="Disabled" variant=ButtonVariant::Primary disabled=true />
            </List>
            <Text style=TextStyle::CAPTION>{clicked}</Text>
        </List>
    }
}

#[sample]
#[component]
fn RowButtons() -> NodeId {
    let chosen = create_signal(0usize);
    view! {
        <Frame width={Extent::at_most(360.0)}>
            <List spacing=Space::Xs2>
                <ListRow
                    label="Wi-Fi"
                    detail="Home network"
                    glyph=ICON_WIFI
                    trailing=RowTrailing::Chevron
                    on_click={move || chosen.set(0)}
                />
                <ListRow
                    label="Chat"
                    detail="3 unread"
                    image={Some(badge(Color32::from_rgb(0x58, 0x65, 0xf2)))}
                    selected={create_memo(move || chosen.get() == 1)}
                    on_click={move || chosen.set(1)}
                />
                <ListRow
                    label="Dark theme"
                    glyph=ICON_CONTRAST
                    trailing=RowTrailing::Check
                    on_click={move || chosen.set(2)}
                />
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn IconButtons() -> NodeId {
    let clicked = create_signal("Nothing clicked yet".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md wrap=true>
                <Button
                    icon_only=true
                    glyph=ICON_COPY
                    label="Copy"
                    variant=ButtonVariant::Primary
                    on_click={move || clicked.set("Clicked Copy".to_owned())}
                />
                <Button
                    icon_only=true
                    glyph=ICON_SHARE_2
                    label="Share"
                    on_click={move || clicked.set("Clicked Share".to_owned())}
                />
                <Button
                    icon_only=true
                    glyph=ICON_TRASH
                    label="Delete"
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
                    on_click={move || clicked.set("Clicked Delete".to_owned())}
                />
                <Button
                    icon_only=true
                    glyph=ICON_UNDO_2
                    label="Undo"
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Toolbar
                    on_click={move || clicked.set("Clicked Undo".to_owned())}
                />
                <Button icon_only=true glyph=ICON_PENCIL label="Edit" disabled=true />
            </List>
            <Text style=TextStyle::CAPTION>
                "Hover an icon button to see its label as a tooltip."
            </Text>
            <Text style=TextStyle::CAPTION>{clicked}</Text>
        </List>
    }
}

#[sample]
#[component]
fn PinToggle() -> NodeId {
    let pressed = create_signal(false);
    let pinned = create_memo(move || match pressed.get() {
        true => "Selection is pinned".to_owned(),
        false => "Selection is unpinned".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <ToggleButton label="Pin selection" pressed />
            </List>
            <Text style=TextStyle::CAPTION>{pinned}</Text>
        </List>
    }
}

#[sample]
#[component]
fn HoverTooltip() -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Tooltip label="Tooltips wrap anything that can be hovered or focused">
                <Chip label="Hover me" @test_id="demo.tooltip.long" />
            </Tooltip>
            <Tooltip label="Short">
                <Chip label="Or me" @test_id="demo.tooltip.short" />
            </Tooltip>
        </List>
    }
}

#[component]
pub(crate) fn InputsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Text input" code={vec![NameInput::SOURCE, greeting_label::SOURCE]}>
                <NameInput />
            </Sample>
            <Sample title="On-screen keyboard" code={vec![KeyboardInput::SOURCE]}>
                <KeyboardInput />
            </Sample>
            <Sample title="Number input" code={vec![SeatsInput::SOURCE]}>
                <SeatsInput />
            </Sample>
            <Sample title="One value, two bindings" code={vec![Temperatures::SOURCE]}>
                <Temperatures />
            </Sample>
            <Sample title="Sliders and progress" code={vec![LoadSliders::SOURCE]}>
                <LoadSliders />
            </Sample>
            <Sample title="Waiting for data" code={vec![SlowGreeting::SOURCE]}>
                <SlowGreeting />
            </Sample>
            <Sample
                title="Disabled controls"
                code={vec![LockedControls::SOURCE, LabelledSwitch::SOURCE]}
            >
                <LockedControls />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn NameInput() -> NodeId {
    let name = create_signal(String::new());
    let greeting = create_memo(move || name.with(|name| greeting_label(name)));
    view! {
        <List spacing=SECTION_SPACING>
            <TextInput value={name} placeholder="Type a name" />
            <Text style=TextStyle::CAPTION>{greeting}</Text>
            <Text style=TextStyle::CAPTION>
                "Click to place the caret, drag to select, and Ctrl+Z to undo."
            </Text>
            <TextInput
                value=String::new()
                placeholder="A note, over several lines"
                multiline=true
            />
            <TextInput
                value="https://blocks.example/invite/3f9c"
                label="Invite link"
                copyable=true
            />
        </List>
    }
}

#[sample]
#[component]
fn KeyboardInput() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <TextInput value=String::new() placeholder="Type with the keys below" />
            <Frame height={Size::Custom(240.0)}>
                <SoftwareKeyboard />
            </Frame>
            <Text style=TextStyle::CAPTION>
                "Tap a key, or drag toward a corner for what it shows there. Circle up, right and down for a capital."
            </Text>
            <Button
                label="Open the on-screen keyboard"
                variant=ButtonVariant::Secondary
                on_click={toggle_keyboard}
            />
        </List>
    }
}

#[sample]
fn greeting_label(name: &str) -> String {
    match name.is_empty() {
        true => "Nobody yet".to_owned(),
        false => format!("Hello, {name}"),
    }
}

#[sample]
#[component]
fn Temperatures() -> NodeId {
    let celsius = create_signal(20.0f64);
    let fahrenheit = Binding::new(
        create_memo(move || celsius.get() * 9.0 / 5.0 + 32.0),
        move |fahrenheit: f64| celsius.set((fahrenheit - 32.0) * 5.0 / 9.0),
    );
    view! {
        <List spacing=ROW_SPACING>
            <Text style=TextStyle::CAPTION>"Celsius, a signal"</Text>
            <NumberInput value={celsius} label="Celsius" @test_id="demo.temperature.celsius" />
            <Text style=TextStyle::CAPTION>"Fahrenheit, derived from it both ways"</Text>
            <NumberInput
                value={fahrenheit}
                label="Fahrenheit"
                @test_id="demo.temperature.fahrenheit"
            />
        </List>
    }
}

#[sample]
#[component]
fn SeatsInput() -> NodeId {
    let seats = create_signal(4.0f64);
    let seats_text = create_memo(move || format!("{} seats", seats.get()));
    view! {
        <List spacing=SECTION_SPACING>
            <NumberInput value={seats} min=1.0 max=12.0 label="Seats" />
            <Text style=TextStyle::CAPTION>{seats_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn LoadSliders() -> NodeId {
    let load = create_signal(0.4f32);
    let load_text = create_memo(move || format!("{}%", (load.get() * 100.0).round()));
    let zoom = create_signal(1.0f32);
    let zoom_text = create_memo(move || format!("{:.2}x", zoom.get()));
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text style=TextStyle::CAPTION>"Simulated load"</Text>
                <Text
                    style=TextStyle::CAPTION
                    @sizing=ItemSize::Percent(100.0)
                    align=TextAlign::End
                >
                    {load_text}
                </Text>
            </List>
            <Slider value={load} label="Load" />
            <Progress value={create_memo(move || Some(load.get()))} label="Load" />
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text style=TextStyle::CAPTION>"Zoom, with 1x in the middle"</Text>
                <Text
                    style=TextStyle::CAPTION
                    @sizing=ItemSize::Percent(100.0)
                    align=TextAlign::End
                >
                    {zoom_text}
                </Text>
            </List>
            <Slider
                value={zoom}
                min=ZOOM_MIN
                max=ZOOM_MAX
                scale={SliderScale::Midpoint(ZOOM_MIDPOINT)}
                label="Zoom"
            />
            <Text style=TextStyle::CAPTION>"Something that takes as long as it takes"</Text>
            <Progress width=PROGRESS_WIDTH label="Working" />
        </List>
    }
}

#[sample]
#[component]
fn SlowGreeting() -> NodeId {
    let answer = create_signal::<Ready<String>>(Err(Pending::Loading));
    let arrive = move || answer.set(Ok("Hello, the data is here".to_owned()));
    let fail = move || answer.set(Err(Pending::Failed("The server said no".to_owned())));
    let fallback = |pending: Pending| match pending {
        Pending::Loading => view! {
            <Progress width=PROGRESS_WIDTH label="Waiting" />
        },
        Pending::Failed(error) => view! {
            <Text style=TextStyle::CAPTION>{error}</Text>
        },
    };
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=ROW_SPACING>
                <Button
                    label="Arrive"
                    variant=ButtonVariant::Primary
                    on_click={arrive}
                    @test_id="demo.loading.arrive"
                />
                <Button label="Fail" variant=ButtonVariant::Secondary on_click={fail} />
            </List>
            <Loading fallback>|| {
                let greeting = create_memo(move || answer.get());
                view! {
                    <Text @test_id="demo.loading.content">{greeting}</Text>
                }
            }</Loading>
        </List>
    }
}

#[sample]
#[component]
fn LockedControls() -> NodeId {
    let locked = create_signal(true);
    view! {
        <List spacing=SECTION_SPACING>
            <LabelledSwitch label="Locked" on={locked} />
            <TextInput value="Read only while locked" disabled={locked} />
            <Select
                options={view! {
                    <ChoiceOption label="String" />
                    <ChoiceOption label="Number" />
                }}
                selected=Some(0)
                label="Field type"
                disabled={locked}
            />
            <Checkbox label="Minimum" checked=false disabled={locked} />
            <Button label="Save" variant=ButtonVariant::Primary disabled={locked} />
        </List>
    }
}

#[sample]
#[component]
fn LightSwitches() -> NodeId {
    let kitchen = create_signal(true);
    let hall = create_signal(false);
    let every = create_memo(move || kitchen.get() && hall.get());
    let mixed = create_memo(move || kitchen.get() != hall.get());
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Switch
                    label="Every light"
                    on={Binding::new(every, move |on: bool| {
                        kitchen.set(on);
                        hall.set(on);
                    })}
                    indeterminate={mixed}
                />
                <Text>"Every light"</Text>
            </List>
            <LabelledSwitch label="Kitchen" on={kitchen} />
            <LabelledSwitch label="Hall" on={hall} />
        </List>
    }
}

#[component]
pub(crate) fn ChoicesPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Radio group" code={vec![UpdateModes::SOURCE]}>
                <UpdateModes />
            </Sample>
            <Sample title="Listbox" code={vec![ColorListbox::SOURCE]}>
                <ColorListbox />
            </Sample>
            <Sample title="Select" code={vec![FruitSelect::SOURCE]}>
                <FruitSelect />
            </Sample>
            <Sample title="Switches with a mixed state" code={vec![LightSwitches::SOURCE]}>
                <LightSwitches />
            </Sample>
            <Sample title="Tabs" code={vec![ScaleTabs::SOURCE]}>
                <ScaleTabs />
            </Sample>
            <Sample title="Segmented control" code={vec![PeriodSegments::SOURCE]}>
                <PeriodSegments />
            </Sample>
            <Sample title="Tabs and segments without room" code={vec![NarrowChoices::SOURCE]}>
                <NarrowChoices />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn UpdateModes() -> NodeId {
    let modes = ["Automatic", "Manual", "Scheduled"];
    let mode = create_signal(Some(0));
    let mode_text = create_memo(move || {
        mode.get()
            .map_or_else(String::new, |index| format!("{} updates", modes[index]))
    });
    view! {
        <List spacing=SECTION_SPACING>
            <RadioGroup
                options={view! {
                    <ChoiceOption label="Automatic" />
                    <ChoiceOption label="Manual" />
                    <ChoiceOption label="Scheduled" />
                }}
                selected={mode}
            />
            <Text style=TextStyle::CAPTION>{mode_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn ColorListbox() -> NodeId {
    let colors = ["Amber", "Blue", "Green", "Purple"];
    let color = create_signal(Some(1));
    let color_text = create_memo(move || {
        color
            .get()
            .map_or_else(String::new, |index| format!("{} selected", colors[index]))
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>"Type to search"</Text>
            <Listbox
                options={view! {
                    <ChoiceOption label="Amber" />
                    <ChoiceOption label="Blue" />
                    <ChoiceOption label="Green" />
                    <ChoiceOption label="Purple" />
                }}
                selected={color}
            />
            <Text style=TextStyle::CAPTION>{color_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn FruitSelect() -> NodeId {
    let fruit = create_signal(Some(0));
    let fruit_text = create_memo(move || {
        fruit.get().and_then(|index| FRUITS.get(index)).map_or_else(
            || "Nothing selected".to_owned(),
            |label| format!("{label} selected"),
        )
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Select
                options={view! {
                    <ForEach keys={FRUITS.to_vec()}>|label: &'static str|
                        <ChoiceOption label />
                    </ForEach>
                }}
                selected={fruit}
                label="Favorite fruit"
                @test_id="demo.fruit"
            />
            <Text style=TextStyle::CAPTION>{fruit_text}</Text>
            <Select
                options={view! {
                    <ForEach keys={FRUITS.to_vec()}>|label: &'static str|
                        <ChoiceOption label />
                    </ForEach>
                }}
                selected=None
                label="Add a fruit"
                placeholder="Add a fruit"
            />
        </List>
    }
}

#[sample]
#[component]
fn ScaleTabs() -> NodeId {
    let scale = create_signal(1usize);
    let scale_text = create_memo(move || format!("Drawn at {}x", scale.get() + 1));
    view! {
        <List spacing=SECTION_SPACING>
            <Tabs
                options={view! {
                    <ChoiceOption label="1x" />
                    <ChoiceOption label="2x" />
                    <ChoiceOption label="3x" />
                }}
                selected={scale}
            />
            <Text style=TextStyle::CAPTION>{scale_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn PeriodSegments() -> NodeId {
    let period = create_signal(1usize);
    let period_text = create_memo(move || {
        format!(
            "Showing the {} view",
            ["day", "week", "month", "year"][period.get()]
        )
    });
    view! {
        <List spacing=SECTION_SPACING>
            <SegmentedControl
                label="Period"
                options={view! {
                    <ChoiceOption label="Day" />
                    <ChoiceOption label="Week" />
                    <ChoiceOption label="Month" />
                    <ChoiceOption label="Year" />
                }}
                selected={period}
            />
            <Text style=TextStyle::CAPTION>{period_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn NarrowChoices() -> NodeId {
    view! {
        <List
            @test_id="demo.choices.narrow"
            direction=Direction::Horizontal
            spacing=SECTION_SPACING
        >
            <Frame width={Extent::px(NARROW_CHOICE_WIDTH)}>
                <Tabs
                    label="Section"
                    options={view! {
                        <ChoiceOption label="Overview" />
                        <ChoiceOption label="Details" />
                        <ChoiceOption label="History" />
                    }}
                />
            </Frame>
            <Frame width={Extent::px(NARROW_CHOICE_WIDTH)}>
                <SegmentedControl
                    label="Sort"
                    options={view! {
                        <ChoiceOption label="Newest" />
                        <ChoiceOption label="Oldest" />
                        <ChoiceOption label="Largest" />
                    }}
                    selected=2
                />
            </Frame>
        </List>
    }
}

fn today() -> Date {
    use_context::<Today>().map_or_else(Date::today, |Today(today)| today)
}

#[component]
pub(crate) fn PickersPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Date" code={vec![DayField::SOURCE]}>
                <DayField />
            </Sample>
            <Sample title="Time, on a 12-hour clock" code={vec![AlarmField::SOURCE]}>
                <AlarmField />
            </Sample>
            <Sample title="Date and time, which can be cleared" code={vec![MeetingField::SOURCE]}>
                <MeetingField />
            </Sample>
            <Sample title="Calendar" code={vec![BookingCalendar::SOURCE]}>
                <BookingCalendar />
            </Sample>
        </ScrollPage>
    }
}

#[component]
pub(crate) fn ColorsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Color wheel" code={vec![HueWheel::SOURCE]}>
                <HueWheel />
            </Sample>
            <Sample title="OKLCH color wheel" code={vec![PerceptualWheel::SOURCE]}>
                <PerceptualWheel />
            </Sample>
            <Sample title="Color input" code={vec![AccentInput::SOURCE]}>
                <AccentInput />
            </Sample>
            <Sample title="Color picker" code={vec![PaintPicker::SOURCE]}>
                <PaintPicker />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn DayField() -> NodeId {
    let today = today();
    let day = create_signal(Some(DateTime::new(today, Time::MIDNIGHT)));
    let day_text = create_memo(move || match day.get() {
        Some(day) => format!("{} was chosen", day.date.label()),
        None => "No day chosen".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <DateTimeField value={day} parts=DateTimeParts::Date label="Day" today={Some(today)} />
            <Text style=TextStyle::CAPTION>{day_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn AlarmField() -> NodeId {
    let today = today();
    let alarm = create_signal(Some(DateTime::new(today, Time::new(7, 30))));
    let alarm_text = create_memo(move || match alarm.get() {
        Some(alarm) => format!("The alarm rings at {}", alarm.time.format(HourCycle::H12)),
        None => "No alarm".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <DateTimeField
                value={alarm}
                parts=DateTimeParts::Time
                hour_cycle=HourCycle::H12
                label="Alarm"
                today={Some(today)}
            />
            <Text style=TextStyle::CAPTION>{alarm_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn MeetingField() -> NodeId {
    let today = today();
    let meeting = create_signal(None::<DateTime>);
    let meeting_text = create_memo(move || match meeting.get() {
        Some(meeting) => format!(
            "Meeting on {} at {}",
            meeting.date.label(),
            meeting.time.format(HourCycle::H24)
        ),
        None => "No meeting yet: type one in or pick it".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <DateTimeField
                value={meeting}
                label="Meeting"
                today={Some(today)}
                clearable=true
                step_minutes=30
            />
            <Text style=TextStyle::CAPTION>{meeting_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn BookingCalendar() -> NodeId {
    let today = today();
    let booked = create_signal(None::<Date>);
    let booked_text = create_memo(move || match booked.get() {
        Some(booked) => format!("Booked for {}", booked.label()),
        None => "Bookings open for the next 60 days".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>"Limited to the next 60 days"</Text>
            <Calendar
                selected={booked}
                min={Some(today)}
                max={Some(today.add_days(60))}
                today={Some(today)}
            />
            <Text style=TextStyle::CAPTION>{booked_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn AccentInput() -> NodeId {
    let accent = create_signal(Color32::from_rgba_unmultiplied(0xF7, 0x6B, 0x15, 0xC0));
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>"Click the swatch to pick"</Text>
            <ColorInput value={accent} label="Accent" />
        </List>
    }
}

#[sample]
#[component]
fn PaintPicker() -> NodeId {
    let paint = create_signal(Color32::from_rgb(0x3E, 0x63, 0xDD));
    let preview = create_signal(None::<Color32>);
    let shown_paint = create_memo(move || preview.get().unwrap_or(paint.get()));
    let paint_text =
        create_memo(move || format!("Painting in {}", beui::format_hex(shown_paint.get(), false)));
    view! {
        <List spacing=SECTION_SPACING>
            <ColorPicker value={paint} alpha=false on_preview={preview} />
            <Frame height=SWATCH_HEIGHT fill={ColorRole::custom(shown_paint)} radius=Radius::Md />
            <Text style=TextStyle::CAPTION>{paint_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn HueWheel() -> NodeId {
    let paint = create_signal(Color32::from_rgb(0x30, 0xA4, 0x6C));
    let preview = create_signal(None::<Color32>);
    let shown_paint = create_memo(move || preview.get().unwrap_or(paint.get()));
    let paint_text =
        create_memo(move || format!("Painting in {}", beui::format_hex(shown_paint.get(), false)));
    view! {
        <List spacing=SECTION_SPACING>
            <ColorWheel value={paint} on_preview={preview} />
            <Frame height=SWATCH_HEIGHT fill={ColorRole::custom(shown_paint)} radius=Radius::Md />
            <Text style=TextStyle::CAPTION>{paint_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn PerceptualWheel() -> NodeId {
    let paint = create_signal(Color32::from_rgb(0xD6, 0x40, 0x9F));
    let preview = create_signal(None::<Color32>);
    let shown_paint = create_memo(move || preview.get().unwrap_or(paint.get()));
    let paint_text = create_memo(move || {
        let color = beui::Oklch::from_color(shown_paint.get());
        format!(
            "oklch({:.0}% {:.3} {:.0})",
            color.lightness * 100.0,
            color.chroma,
            color.hue
        )
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>"Turning the hue keeps lightness and chroma"</Text>
            <OklchColorWheel value={paint} on_preview={preview} />
            <Frame height=SWATCH_HEIGHT fill={ColorRole::custom(shown_paint)} radius=Radius::Md />
            <Text style=TextStyle::CAPTION>{paint_text}</Text>
        </List>
    }
}

#[component]
pub(crate) fn MenusPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Context menu" code={vec![CardMenu::SOURCE]}>
                <CardMenu />
            </Sample>
            <Sample title="Menus built from actions" code={vec![ActionMenu::SOURCE]}>
                <ActionMenu />
            </Sample>
            <Sample title="Action bar" code={vec![EditorActions::SOURCE]}>
                <EditorActions />
            </Sample>
            <Sample title="Menu buttons" code={vec![SortMenus::SOURCE]}>
                <SortMenus />
            </Sample>
            <Sample
                title="Menu pages and trees"
                code={vec![NewBlockMenu::SOURCE, FolderRow::SOURCE, folder_shown::SOURCE]}
            >
                <NewBlockMenu />
            </Sample>
            <Sample title="Menu captions and grids" code={vec![NewSlideMenu::SOURCE]}>
                <NewSlideMenu />
            </Sample>
            <Sample title="Menu opened by a model" code={vec![HeldMenu::SOURCE]}>
                <HeldMenu />
            </Sample>
            <Sample title="Picture menu buttons" code={vec![TrayIcons::SOURCE, badge::SOURCE]}>
                <TrayIcons />
            </Sample>
            <Sample title="Split buttons" code={vec![RunButtons::SOURCE]}>
                <RunButtons />
            </Sample>
            <Sample title="Global actions" code={vec![GlobalActions::SOURCE]}>
                <GlobalActions />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn EditorActions() -> NodeId {
    let said = create_signal("Nothing run yet".to_owned());
    let action = move |id: &str, label: &'static str, glyph: Glyph| {
        Action::new(id, label, move || said.set(format!("Ran {label}"))).glyph(glyph)
    };
    let undo = action("toolbar.undo", "Undo", ICON_UNDO_2)
        .pin(Pin::Icon)
        .detached();
    let share = action("toolbar.share", "Share", ICON_SHARE_2)
        .pin(Pin::Label)
        .detached();
    let rename = action("toolbar.rename", "Rename", ICON_PENCIL).detached();
    let delete = action("toolbar.delete", "Delete", ICON_TRASH).detached();
    let menu = vec![
        MenuNode::action(&undo),
        MenuNode::action(&share),
        MenuNode::Separator,
        MenuNode::action(&rename),
        MenuNode::action(&delete),
    ];
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::PARAGRAPH>
                "An action bar shows the actions pinned to it and offers the rest under More. \
                 Where there is no room for labels, the actions pinned with one move into the \
                 menu too."
            </Text>
            <ActionBar menu={menu.clone()} test_prefix={"bar".to_owned()} />
            <ActionBar
                menu
                labels=ActionLabels::Overflow
                size=ButtonSize::Regular
                test_prefix={"bar.narrow".to_owned()}
            />
            <Text style=TextStyle::CAPTION>{said}</Text>
        </List>
    }
}

#[sample]
#[component]
fn GlobalActions() -> NodeId {
    let locks = create_signal(0);
    let open = create_signal(false);
    let lock = Action::new("demo.lock", "Lock the screen", move || {
        locks.update(|locks| *locks += 1)
    })
    .glyph(ICON_LOCK)
    .shortcut(Chord::logo(Key::L))
    .at(KeyAt::Window)
    .capture()
    .register();
    let palette = Action::new("demo.palette", "Command palette", move || open.set(true))
        .shortcut(Chord::logo(Key::P))
        .at(KeyAt::Window)
        .register();
    let lock_keys = lock.shortcut_label().unwrap_or_default();
    let palette_keys = palette.shortcut_label().unwrap_or_default();
    let locked = create_memo(move || format!("Locked {} times", locks.get()));
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::PARAGRAPH>
                "A global action answers its chord wherever the focus is, even in a text \
                 field or under an open menu; Super chords count like Ctrl and Alt. While a \
                 program's window or a plugin editor has the focus, its keys are its own, unless \
                 the action intercepts them."
            </Text>
            <Shortcut
                keys={lock_keys}
                description="lock the screen: intercepts, so it works over any program"
            />
            <Shortcut
                keys={palette_keys}
                description="open the palette: global, but a focused program keeps the keys"
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md wrap=true>
                <Button action={lock} variant=ButtonVariant::Primary />
                <Button
                    @test_id="demo.palette.open"
                    action={palette}
                    variant=ButtonVariant::Secondary
                />
            </List>
            <Text style=TextStyle::CAPTION>{locked}</Text>
            <CommandPalette open />
        </List>
    }
}

#[sample]
#[component]
fn ActionMenu() -> NodeId {
    let wrapped = create_signal(true);
    let size = create_signal(14u32);
    let copies = create_signal(0);
    let copy = Action::new("demo.menu.copy", "Copy", move || {
        copies.update(|copies| *copies += 1)
    })
    .glyph(ICON_COPY)
    .shortcut(Chord::ctrl(Key::C))
    .register();
    let wrap = Action::new("demo.menu.wrap", "Wrap lines", move || {
        wrapped.update(|wrapped| *wrapped = !*wrapped)
    })
    .checked(wrapped)
    .register();
    let link = Action::new(
        "demo.menu.link",
        "Copy a link to this paragraph",
        move || copies.update(|copies| *copies += 1),
    )
    .shortcut(Chord::ctrl(Key::C).alt().shift())
    .register();
    let sized = |id: &str, label: &'static str, points: u32| {
        Action::new(id, label, move || size.set(points))
            .radio(create_memo(move || size.get() == points))
            .register()
    };
    let nodes = vec![
        MenuNode::action(&copy),
        MenuNode::action(&link),
        MenuNode::Separator,
        MenuNode::action(&wrap),
        MenuNode::submenu(
            "demo.menu.size",
            "Text size",
            ICON_A_LARGE_SMALL,
            [
                sized("demo.menu.small", "Small", 12),
                sized("demo.menu.medium", "Medium", 14),
                sized("demo.menu.large", "Large", 18),
            ]
            .iter()
            .map(MenuNode::action)
            .collect(),
        ),
    ];
    let state = create_memo(move || {
        format!(
            "Copied {} times, wrapping {}, {} pt",
            copies.get(),
            if wrapped.get() { "on" } else { "off" },
            size.get()
        )
    });
    view! {
        <List spacing=SECTION_SPACING>
            <ContextMenu
                items={view! {
                    <unstyled::MenuNodes nodes test_prefix="demo.actions" />
                }}
            >
                <Card @test_id="demo.actions">
                    <Text style=TextStyle::PARAGRAPH>
                        "One description of actions, separators and submenus fills this menu, a \
                         plugin's frame menu and the command palette. Each row shows its action's \
                         shortcut, a checkbox ticks while its action is checked, and a radio group \
                         ticks the chosen one."
                    </Text>
                </Card>
            </ContextMenu>
            <Text style=TextStyle::CAPTION>{state}</Text>
        </List>
    }
}

#[sample]
#[component]
fn CardMenu() -> NodeId {
    let menu_text = create_signal("Nothing chosen yet".to_owned());
    let copied = create_signal(false);
    let nothing_copied = create_memo(move || !copied.get());
    view! {
        <List spacing=SECTION_SPACING>
            <ContextMenu
                items={view! {
                    <unstyled::MenuItem label="Copy" glyph=ICON_COPY detail="Ctrl+C" />
                    <unstyled::MenuItem
                        label="Paste"
                        glyph=ICON_CLIPBOARD
                        detail="Ctrl+V"
                        disabled={nothing_copied}
                    />
                    <unstyled::MenuItem label="Share" glyph=ICON_SHARE_2>
                        <unstyled::MenuItem label="Email" glyph=ICON_MAIL />
                        <unstyled::MenuItem label="Link" glyph=ICON_LINK />
                    </unstyled::MenuItem>
                    <unstyled::MenuItem
                        label="Delete"
                        glyph=ICON_TRASH
                        danger=true
                        separated=true
                    />
                }}
                on_select={move |path: Vec<usize>| {
                    let label = match path.as_slice() {
                        [0] => {
                            copied.set(true);
                            "Copy".to_owned()
                        }
                        [1] => "Paste".to_owned(),
                        [2, 0] => "Share > Email".to_owned(),
                        [2, 1] => "Share > Link".to_owned(),
                        [3] => "Delete".to_owned(),
                        other => format!("{other:?}"),
                    };
                    menu_text.set(format!("Chose: {label}"));
                }}
            >
                <Card>
                    <List spacing=Space::Xs>
                        <Text style=TextStyle::CAPTION>"Right-click or long-press this card"</Text>
                        <Text style=TextStyle::PARAGRAPH>
                            "A mouse opens the menu at the pointer and a finger opens \
                             it as a sheet. Paste stays disabled until Copy is chosen; Share \
                             opens a submenu on hover or Right Arrow, and Left Arrow closes it."
                        </Text>
                    </List>
                </Card>
            </ContextMenu>
            <Text style=TextStyle::CAPTION>{menu_text}</Text>
        </List>
    }
}

#[sample]
fn badge(color: Color32) -> Image {
    const SIDE: u32 = 32;
    let center = (SIDE as f32 - 1.0) / 2.0;
    let pixels = (0..SIDE * SIDE)
        .flat_map(|at| {
            let (x, y) = ((at % SIDE) as f32 - center, (at / SIDE) as f32 - center);
            match x * x + y * y <= center * center {
                true => color.to_array(),
                false => [0, 0, 0, 0],
            }
        })
        .collect();
    Image::from_rgba(SIDE, SIDE, pixels)
}

#[sample]
#[component]
fn TrayIcons() -> NodeId {
    let outcome = create_signal("Nothing done yet".to_owned());
    let sync_open = create_signal(false);
    let sync_open = Binding::new(sync_open, move |open: bool| {
        sync_open.set(open);
        if open {
            outcome.set("Opened the Sync menu".to_owned());
        }
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Compact
                    secondary_opens=true
                    label="Chat"
                    image={Some(badge(Color32::from_rgb(0x58, 0x65, 0xf2)))}
                    click_opens=false
                    items={view! {
                        <unstyled::MenuItem label="Open" />
                        <unstyled::MenuItem label="Quit" separated=true />
                    }}
                    on_click={move || outcome.set("Clicked Chat".to_owned())}
                    on_middle_click={move || outcome.set("Middle-clicked Chat".to_owned())}
                    on_scroll={move |_: ScrollGesture| outcome.set("Scrolled over Chat".to_owned())}
                    on_select={move |path: Vec<usize>| {
                        let label = match path.as_slice() {
                            [0] => "Open",
                            _ => "Quit",
                        };
                        outcome.set(format!("Chose {label}"));
                    }}
                />
                <MenuButton
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Compact
                    secondary_opens=true
                    label="Sync"
                    open={sync_open}
                    image={None}
                    glyph=ICON_BLOCKS
                    items={view! {
                        <unstyled::MenuItem label="Pause syncing" />
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "A picture, or a glyph without one. Chat activates on a click and shows \
                 its menu on a right click; Sync shows its menu on either."
            </Text>
            <Text style=TextStyle::CAPTION>{outcome}</Text>
        </List>
    }
}

#[sample]
#[component]
fn SortMenus() -> NodeId {
    let sort_text = create_signal("Sorted by name".to_owned());
    let done = create_signal(false);
    let status = create_memo(move || match done.get() {
        true => "Done".to_owned(),
        false => "In progress".to_owned(),
    });
    let status_fill = create_memo(move || match done.get() {
        true => ColorRole::Success,
        false => ColorRole::Warning,
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md wrap=true>
                <MenuButton
                    label="Sort"
                    items={view! {
                        <unstyled::MenuItem label="By name" />
                        <unstyled::MenuItem label="By date" />
                        <unstyled::MenuItem label="By size" />
                    }}
                    on_select={move |path: Vec<usize>| {
                        let by = match path.as_slice() {
                            [0] => "name",
                            [1] => "date",
                            _ => "size",
                        };
                        sort_text.set(format!("Sorted by {by}"));
                    }}
                />
                <MenuButton
                    label="More"
                    glyph=ICON_MENU
                    icon_only=true
                    arrow=false
                    variant=ButtonVariant::Ghost
                    items={view! {
                        <unstyled::MenuItem label="Reverse the order" />
                    }}
                    on_select={move |_: Vec<usize>| sort_text.set("Sorted in reverse".to_owned())}
                />
                <MenuButton
                    label={status}
                    variant=ButtonVariant::Ghost
                    arrow=false
                    leading={view! {
                        <Frame width=Size::Track height=Size::Track radius=Radius::Full fill={status_fill} />
                    }}
                    items={view! {
                        <unstyled::MenuItem label="In progress" />
                        <unstyled::MenuItem label="Done" />
                    }}
                    on_select={move |path: Vec<usize>| done.set(path.as_slice() == [1])}
                />
            </List>
            <Text style=TextStyle::CAPTION>{sort_text}</Text>
        </List>
    }
}

const FOLDERS: [(&str, usize, Option<usize>); 10] = [
    ("Home", 0, None),
    ("Work", 1, Some(0)),
    ("Projects", 2, Some(1)),
    ("Alpha", 3, Some(2)),
    ("Beta", 3, Some(2)),
    ("Notes", 2, Some(1)),
    ("Personal", 1, Some(0)),
    ("Recipes", 2, Some(6)),
    ("Travel", 2, Some(6)),
    ("Archive", 1, Some(0)),
];
const MORE_BLOCKS: [&str; 24] = [
    "Checklist",
    "Calendar",
    "Chat",
    "Code",
    "Contacts",
    "Diagram",
    "Drawing",
    "Email",
    "Feed",
    "Form",
    "Gallery",
    "Kanban board",
    "Map",
    "Music",
    "Outline",
    "Presentation",
    "Recipe",
    "Spreadsheet",
    "Terminal",
    "Timeline",
    "Timer",
    "Video",
    "Web page",
    "Whiteboard",
];
const SLIDE_LAYOUTS: [(&str, Glyph); 6] = [
    ("Title", ICON_HEADING),
    ("Title and body", ICON_TEXT_ALIGN_START),
    ("Two columns", ICON_COLUMNS_3),
    ("Picture", ICON_IMAGE),
    ("Quote", ICON_QUOTE),
    ("Blank", ICON_SQUARE),
];

#[sample]
fn folder_shown(folder: usize, expanded: &[usize]) -> bool {
    let mut parent = FOLDERS[folder].2;
    while let Some(above) = parent {
        if !expanded.contains(&above) {
            return false;
        }
        parent = FOLDERS[above].2;
    }
    true
}

#[sample]
#[component]
fn FolderRow(
    folder: usize,
    location: Signal<usize>,
    expanded: Signal<Vec<usize>>,
) -> unstyled::MenuItem {
    let (name, depth, _) = FOLDERS[folder];
    let branch = FOLDERS.iter().any(|(_, _, parent)| *parent == Some(folder));
    let open = Binding::new(
        Prop::dynamic(move || branch.then(|| expanded.get().contains(&folder))),
        move |open: Option<bool>| {
            expanded.update(|expanded| match open {
                Some(true) => expanded.push(folder),
                _ => expanded.retain(|other| *other != folder),
            })
        },
    );
    let checked = create_memo(move || Some(location.get() == folder));
    view! {
        <unstyled::MenuItem
            label=name
            glyph=ICON_FOLDER
            depth={Some(depth)}
            expanded={open}
            checked
            then=MenuThen::Back
            row_test_id={format!("demo.new.folder.{folder}")}
            on_click={move || location.set(folder)}
        />
    }
}

#[sample]
#[component]
fn NewBlockMenu() -> NodeId {
    let theme = use_theme();
    let text_icon_color = create_memo(move || Some(TextColor::Success.resolve(&theme)));
    let location = create_signal(1_usize);
    let expanded = create_signal(vec![0_usize, 1]);
    let added = create_signal("Nothing added yet".to_owned());
    let shown = create_memo(move || {
        let open = expanded.get();
        (0..FOLDERS.len())
            .filter(|folder| folder_shown(*folder, &open))
            .collect::<Vec<_>>()
    });
    let place = create_memo(move || FOLDERS[location.get()].0.to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    @test_id="demo.new"
                    label="Add block"
                    glyph=ICON_PLUS
                    items={view! {
                        <unstyled::MenuItem
                            lead="New in"
                            label={place}
                            glyph=ICON_FOLDER
                            page=true
                            row_test_id="demo.new.location"
                        >
                            <ForEach keys={shown}>|folder: usize|
                                <FolderRow folder location expanded />
                            </ForEach>
                        </unstyled::MenuItem>
                        <unstyled::MenuItem
                            label="Text"
                            glyph=ICON_TEXT_ALIGN_START
                            glyph_color={text_icon_color}
                            separated=true
                        />
                        <unstyled::MenuItem label="Canvas" glyph=ICON_PEN_TOOL />
                        <unstyled::MenuItem label="Folder" glyph=ICON_FOLDER />
                        <unstyled::MenuItem label="Database" glyph=ICON_TABLE />
                        <unstyled::MenuItem
                            label="More blocks…"
                            glyph=ICON_BLOCKS
                            separated=true
                            page=true
                            row_test_id="demo.new.more"
                        >
                            <ForEach keys={(0..MORE_BLOCKS.len()).collect::<Vec<_>>()}>|kind: usize|
                                <unstyled::MenuItem label={MORE_BLOCKS[kind]} />
                            </ForEach>
                        </unstyled::MenuItem>
                        <unstyled::MenuItem label="Link existing…" glyph=ICON_LINK />
                        <unstyled::MenuItem label="Import a .block file…" glyph=ICON_FILE_UP />
                    }}
                    on_select={move |path: Vec<usize>| {
                        let kind = match path.as_slice() {
                            [1] => "a text",
                            [2] => "a canvas",
                            [3] => "a folder",
                            [4] => "a database",
                            [5, kind] => MORE_BLOCKS[*kind],
                            [6] => "a link",
                            [7] => "an import",
                            _ => return,
                        };
                        added.set(format!("Added {kind} in {}", place.get()));
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "New in opens a page of folders in place of the menu: arrows walk the \
                 tree, Right and Left open and close a folder, and choosing one comes back \
                 with it. More blocks opens a page long enough to scroll. Left or Escape goes \
                 back a page."
            </Text>
            <Text style=TextStyle::CAPTION>{added}</Text>
        </List>
    }
}

#[sample]
#[component]
fn HeldMenu() -> NodeId {
    let open = create_signal(false);
    let chosen = create_signal("Nothing chosen yet".to_owned());
    let button = NodeRef::new();
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    @node_ref=&button
                    @test_id="demo.held"
                    label="Ask for a colour"
                    variant=ButtonVariant::Secondary
                    on_click={move || open.set(true)}
                />
            </List>
            <MenuPopup
                anchor=button
                open
                min_width={Size::Custom(240.0)}
                on_dismiss={move || open.set(false)}
                on_select={move |path: Vec<usize>| {
                    let colour = ["Red", "Green", "Blue"][path.first().copied().unwrap_or_default()];
                    chosen.set(format!("Chose {colour}"));
                }}
                items={view! {
                    <unstyled::MenuItem label="Red" lead="Paint it" />
                    <unstyled::MenuItem label="Green" lead="Paint it" />
                    <unstyled::MenuItem label="Blue" lead="Paint it" />
                }}
            />
            <Text style=TextStyle::CAPTION>
                "The menu shows while a signal says so, at whatever node or point it is anchored to, and reports Escape or a press outside so the owner can close it. A lead is a few words before the label."
            </Text>
            <Text style=TextStyle::CAPTION>{chosen}</Text>
        </List>
    }
}

#[sample]
#[component]
fn NewSlideMenu() -> NodeId {
    let added = create_signal("No slide added yet".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    @test_id="demo.slide"
                    label="New slide"
                    glyph=ICON_PLUS
                    items={view! {
                        <unstyled::MenuCaption label="New slide" />
                        <unstyled::MenuGrid columns=2>
                            <ForEach
                                keys={(0..SLIDE_LAYOUTS.len()).collect::<Vec<_>>()}
                            >|layout: usize|
                                <unstyled::MenuItem
                                    label={SLIDE_LAYOUTS[layout].0}
                                    glyph={SLIDE_LAYOUTS[layout].1}
                                    row_test_id={format!("demo.slide.{layout}")}
                                />
                            </ForEach>
                        </unstyled::MenuGrid>
                        <unstyled::MenuItem label="Other blocks…" separated=true />
                        <unstyled::MenuItem label="Link existing…" glyph=ICON_LINK />
                    }}
                    on_select={move |path: Vec<usize>| {
                        let what = match path.as_slice() {
                            [1, layout] => SLIDE_LAYOUTS[*layout].0,
                            [2] => "another block",
                            _ => "a link",
                        };
                        added.set(format!("Added {what}"));
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "A caption heads a grid of layouts; arrow keys move across and down the \
                 grid and on into the rows below it."
            </Text>
            <Text style=TextStyle::CAPTION>{added}</Text>
        </List>
    }
}

#[sample]
#[component]
fn RunButtons() -> NodeId {
    let run_text = create_signal("Not running".to_owned());
    let save_text = create_signal("Not saved".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md wrap=true>
                <SplitButton
                    label="Run the app"
                    glyph=ICON_PLAY
                    variant=ButtonVariant::Primary
                    menu_label="More ways to run it"
                    items={view! {
                        <unstyled::MenuItem label="Run with fresh data" />
                        <unstyled::MenuItem label="Run on the web" />
                    }}
                    on_click={move || run_text.set("Running the app".to_owned())}
                    on_select={move |path: Vec<usize>| {
                        let how = match path.as_slice() {
                            [0] => "with fresh data",
                            _ => "on the web",
                        };
                        run_text.set(format!("Running the app {how}"));
                    }}
                />
                <SplitButton
                    label="Save"
                    variant=ButtonVariant::Secondary
                    menu_label="More ways to save"
                    items={view! {
                        <unstyled::MenuItem label="Save as a copy" />
                    }}
                    on_click={move || save_text.set("Saved".to_owned())}
                    on_select={move |_: Vec<usize>| save_text.set("Saved a copy".to_owned())}
                />
            </List>
            <Text style=TextStyle::CAPTION>{run_text}</Text>
            <Text style=TextStyle::CAPTION>{save_text}</Text>
        </List>
    }
}

#[component]
pub(crate) fn OverlaysPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Popover" code={vec![FiltersButton::SOURCE, FilterPopover::SOURCE]}>
                <FiltersButton />
            </Sample>
            <Sample title="Dialog" code={vec![DiscardDialog::SOURCE]}>
                <DiscardDialog />
            </Sample>
            <Sample title="Dialog with a field" code={vec![RenameDialog::SOURCE]}>
                <RenameDialog />
            </Sample>
            <Sample title="Dialog with busy and nested actions" code={vec![InviteDialog::SOURCE]}>
                <InviteDialog />
            </Sample>
            <Sample title="Launcher" code={vec![ProgramLauncher::SOURCE, program::SOURCE]}>
                <ProgramLauncher />
            </Sample>
            <Sample title="Menu as a sheet" code={vec![ActionsSheet::SOURCE]}>
                <ActionsSheet />
            </Sample>
            <Sample title="Long modal sheet" code={vec![LongSheet::SOURCE]}>
                <LongSheet />
            </Sample>
            <Sample
                title="Fullscreen"
                code={vec![FullscreenButton::SOURCE, FullscreenBody::SOURCE]}
            >
                <FullscreenButton />
            </Sample>
            <Sample title="Toasts" code={vec![ToastArea::SOURCE]}>
                <ToastArea />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn FiltersButton() -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Popover label="Filters" @test_id="demo.filters">|handle: PopoverHandle|
                <FilterPopover handle />
            </Popover>
        </List>
    }
}

#[sample]
#[component]
fn FilterPopover(handle: PopoverHandle) -> NodeId {
    let close = handle.close;
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::HEADING>"Filters"</Text>
            <Checkbox label="Only starred" checked=false />
            <Checkbox label="Include archived" checked=true />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Done"
                    variant=ButtonVariant::Primary
                    on_click={move || close.call(())}
                />
            </List>
        </List>
    }
}

#[sample]
#[component]
fn DiscardDialog() -> NodeId {
    let dialog = create_signal(false);
    let outcome = create_signal("Nothing discarded".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    @test_id={"demo.discard.open"}
                    label="Discard changes…"
                    variant=ButtonVariant::Secondary
                    on_click={move || dialog.set(true)}
                />
            </List>
            <Text style=TextStyle::CAPTION>{outcome}</Text>
            <Dialog
                open={dialog}
                title="Discard changes?"
                width=DIALOG_WIDTH
                kind=DialogKind::Alert
                id="demo.discard"
                actions={vec![
                    DialogAction::destructive("Discard", move || {
                        outcome.set("Discarded the changes".to_owned());
                        dialog.set(false);
                    }),
                    DialogAction::cancel("Keep"),
                ]}
                on_dismiss={move || dialog.set(false)}
            >
                <Text style=TextStyle::PARAGRAPH>
                    "Escape, a click outside or the back gesture dismisses a dialog."
                </Text>
            </Dialog>
        </List>
    }
}

#[sample]
#[component]
fn RenameDialog() -> NodeId {
    let dialog = create_signal(false);
    let name = create_signal("Notes".to_owned());
    let typed = create_signal(String::new());
    let blank = create_memo(move || typed.get().trim().is_empty());
    let rename = move || {
        name.set(typed.get_untracked().trim().to_owned());
        dialog.set(false);
    };
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    @test_id={"demo.rename.open"}
                    label="Rename…"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        typed.set(name.get_untracked());
                        dialog.set(true);
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION @test_id={"demo.rename.name"}>{name}</Text>
            <Dialog
                open={dialog}
                title="Rename"
                width=DIALOG_WIDTH
                id="demo.rename"
                actions={vec![
                    DialogAction::cancel("Cancel"),
                    DialogAction::primary("Rename", rename).disabled(blank),
                ]}
                on_dismiss={move || dialog.set(false)}
            >
                <TextInput
                    value={typed}
                    label="Name"
                    clearable=true
                    @test_id={"demo.rename.field"}
                />
            </Dialog>
        </List>
    }
}

#[sample]
#[component]
fn InviteDialog() -> NodeId {
    let dialog = create_signal(false);
    let email = create_signal(String::new());
    let sending = create_signal(false);
    let asking = create_signal(false);
    let blank = create_memo(move || email.get().trim().is_empty());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    @test_id={"demo.invite.open"}
                    label="Invite…"
                    variant=ButtonVariant::Secondary
                    on_click={move || dialog.set(true)}
                />
            </List>
            <Dialog
                open={dialog}
                title="Invite member"
                width=DIALOG_WIDTH
                id="demo.invite"
                actions={vec![
                    DialogAction::secondary("Clear", move || asking.set(true))
                        .disabled(blank)
                        .named("clear"),
                    DialogAction::cancel("Cancel"),
                    DialogAction::primary("Send", move || sending.set(true))
                        .disabled(blank)
                        .busy(sending),
                ]}
                on_dismiss={move || {
                    sending.set(false);
                    dialog.set(false);
                }}
            >
                <TextInput value={email} label="Email address" @test_id={"demo.invite.email"} />
                <Dialog
                    open={asking}
                    kind=DialogKind::Alert
                    title="Clear the address?"
                    id="demo.invite.clear"
                    actions={vec![
                        DialogAction::destructive("Clear", move || {
                            email.set(String::new());
                            asking.set(false);
                        }),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={move || asking.set(false)}
                >
                    <Text style=TextStyle::PARAGRAPH>
                        "A confirmation inside a dialog is an alert of its own."
                    </Text>
                </Dialog>
            </Dialog>
        </List>
    }
}

#[sample]
fn program(
    key: &str,
    title: &str,
    detail: &str,
    terms: &[&str],
    color: Option<Color32>,
) -> PaletteItem {
    let side = LAUNCHER_ICON_PIXELS;
    let image = color.map(|color| {
        let pixels = (0..side * side)
            .flat_map(|at| {
                let (x, y) = (at % side, at / side);
                let edge = x.min(y).min(side - 1 - x).min(side - 1 - y);
                match edge < LAUNCHER_ICON_BORDER {
                    true => [255, 255, 255, 255],
                    false => color.to_array(),
                }
            })
            .collect();
        Image::from_rgba(side, side, pixels)
    });
    PaletteItem {
        key: key.to_owned(),
        title: title.to_owned(),
        detail: detail.to_owned(),
        terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        glyph: Some(ICON_LAYOUT_GRID),
        image,
        ..PaletteItem::default()
    }
}

#[sample]
#[component]
fn ProgramLauncher() -> NodeId {
    let launcher = create_signal(false);
    let outcome = create_signal("Nothing launched".to_owned());
    let items = vec![
        program(
            "calculator",
            "Calculator",
            "Perform calculations",
            &["math"],
            Some(Color32::from_rgb(52, 120, 246)),
        ),
        program(
            "files",
            "Files",
            "Access and organize files",
            &["folder", "manager"],
            Some(Color32::from_rgb(245, 166, 35)),
        ),
        program(
            "firefox",
            "Firefox",
            "Browse the web",
            &["internet", "browser"],
            Some(Color32::from_rgb(230, 80, 40)),
        ),
        program(
            "terminal",
            "Terminal",
            "Use the command line",
            &["shell", "prompt"],
            None,
        ),
        program(
            "text",
            "Text Editor",
            "Edit text files",
            &["notepad"],
            Some(Color32::from_rgb(90, 180, 110)),
        ),
    ];
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    @test_id={"demo.launcher.open"}
                    label="Open the launcher"
                    variant=ButtonVariant::Secondary
                    on_click={move || launcher.set(true)}
                />
            </List>
            <Text style=TextStyle::CAPTION>{outcome}</Text>
            <CommandPalette
                open={launcher}
                items={PaletteItems::list(items)}
                score={PaletteItem::fuzzy_score as PaletteScore}
                placeholder="Search programs"
                label="Search programs"
                id="launcher"
                on_choose={move |key: String| outcome.set(format!("Launched {key}"))}
            />
        </List>
    }
}

#[sample]
#[component]
fn LongSheet() -> NodeId {
    let sheet = create_signal(false);
    let picked = create_signal("Nothing picked".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Pick a row"
                    variant=ButtonVariant::Secondary
                    on_click={move || sheet.set(true)}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "Swipe up on the sheet to raise it, then keep swiping to scroll its rows."
            </Text>
            <Text style=TextStyle::CAPTION>{picked}</Text>
            <ModalSheet open={sheet} rest=0.5 on_close={move || sheet.set(false)}>
                <List spacing=Space::None>
                    <ForEach keys={(1..=60).collect::<Vec<usize>>()}>|row: usize|
                        <ActionRow
                            label={format!("Row {row}")}
                            glyph=ICON_PENCIL
                            on_click={move || {
                                picked.set(format!("Picked row {row}"));
                                sheet.set(false);
                            }}
                        />
                    </ForEach>
                </List>
            </ModalSheet>
        </List>
    }
}

#[sample]
#[component]
fn ActionsSheet() -> NodeId {
    let outcome = create_signal("No action taken".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    label="Show actions"
                    items={view! {
                        <unstyled::MenuItem label="Duplicate" glyph=ICON_COPY_PLUS />
                        <unstyled::MenuItem label="Rename" glyph=ICON_PENCIL detail="Ctrl+R" />
                        <unstyled::MenuItem label="Share" glyph=ICON_SHARE_2 disabled=true />
                        <unstyled::MenuItem
                            label="Delete"
                            glyph=ICON_TRASH
                            danger=true
                            separated=true
                        />
                    }}
                    on_select={move |path: Vec<usize>| {
                        let done = match path.as_slice() {
                            [0] => "Duplicated",
                            [1] => "Renamed",
                            [3] => "Deleted",
                            _ => return,
                        };
                        outcome.set(done.to_owned());
                    }}
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "Tap the button to see its menu as a sheet; swipe it down, or tap above \
                 it, to close it. A click opens the same menu under the button."
            </Text>
            <Text style=TextStyle::CAPTION>{outcome}</Text>
        </List>
    }
}

#[sample]
#[component]
fn ToastArea() -> NodeId {
    let area = beui::reactive::NodeRef::new();
    let toasts = create_signal(vec![
        Toast {
            id: 1,
            message: "Saved a copy of the workspace.".to_owned(),
            danger: false,
            ..Toast::default()
        },
        Toast {
            id: 2,
            message: "Could not run firefox: the command was not found".to_owned(),
            danger: true,
            ..Toast::default()
        },
        message_toast(3),
    ]);
    let next = create_signal(4);
    let chosen = create_signal(String::new());
    view! {
        <Frame
            @node_ref=&area
            height=TOAST_AREA_HEIGHT
            fill=ColorRole::Surface
            radius=Radius::Lg
            padding=Space::Lg
        >
            <List spacing=Space::None>
                <List direction=Direction::Horizontal align=Align::Start spacing=Space::Md>
                    <Button
                        label="Show a toast"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            let id = next.get_untracked();
                            next.set(id + 1);
                            toasts.update(|toasts| toasts.push(Toast {
                                id,
                                message: format!("Toast {id}, gone in a few seconds."),
                                danger: false,
                                ..Toast::default()
                            }));
                        }}
                    />
                    <Button
                        label="Show a message"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            let id = next.get_untracked();
                            next.set(id + 1);
                            toasts.update(|toasts| toasts.push(message_toast(id)));
                        }}
                    />
                    <Text color=TextColor::Muted>{chosen}</Text>
                </List>
                <Toasts
                    anchor={area}
                    toasts={toasts}
                    on_dismiss={move |id: u64| toasts.update(|toasts| toasts.retain(|toast| toast.id != id))}
                    on_action={move |(id, key): (u64, String)| {
                        chosen.set(format!("Chose {key} on message {id}"));
                        toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                    }}
                    on_activate={move |id: u64| {
                        chosen.set(format!("Opened message {id}"));
                        toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                    }}
                />
            </List>
        </Frame>
    }
}

fn message_toast(id: u64) -> Toast {
    Toast {
        id,
        title: "Mia Chen".to_owned(),
        message: "Lunch at noon? The new place on Bridge Street has a table free.".to_owned(),
        actions: vec![
            ToastAction {
                key: "reply".to_owned(),
                label: "Reply".to_owned(),
            },
            ToastAction {
                key: "read".to_owned(),
                label: "Mark as read".to_owned(),
            },
        ],
        activates: true,
        sticky: true,
        ..Toast::default()
    }
}

#[sample]
#[component]
fn FullscreenButton() -> NodeId {
    let fullscreen = create_signal(false);
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Go fullscreen"
                    variant=ButtonVariant::Secondary
                    on_click={move || fullscreen.set(true)}
                />
            </List>
            <Fullscreen open={fullscreen} on_dismiss={move || fullscreen.set(false)}>
                <FullscreenBody on_close={move || fullscreen.set(false)} />
            </Fullscreen>
        </List>
    }
}

#[sample]
#[component]
fn FullscreenBody(on_close: ClickCallback) -> NodeId {
    view! {
        <Frame fill=ColorRole::Background padding=PAGE_PADDING>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::TITLE @sizing=ItemSize::Percent(100.0)>
                        "Fullscreen"
                    </Text>
                    <Button label="Close" variant=ButtonVariant::Secondary on_click={on_close} />
                </List>
                <Text style=TextStyle::PARAGRAPH>
                    "This covers the whole window. Escape closes it too."
                </Text>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn RowsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="A horizontal strip" code={vec![CardStrip::SOURCE, StripCard::SOURCE]}>
                <CardStrip />
            </Sample>
            <Sample title="A scroll that fits its rows" code={vec![FittedScroll::SOURCE]}>
                <FittedScroll />
            </Sample>
            <Sample title="A log that follows its end" code={vec![FollowingLog::SOURCE]}>
                <FollowingLog />
            </Sample>
            <Sample
                title="Ten thousand rows"
                code={vec![ThousandsOfRows::SOURCE, ScrollRow::SOURCE, ScrollRowFace::SOURCE]}
            >
                <ThousandsOfRows />
            </Sample>
            <Sample
                title="Selectable lines"
                code={vec![SelectableServerLog::SOURCE, ServerLine::SOURCE]}
            >
                <SelectableServerLog />
            </Sample>
        </ScrollPage>
    }
}

fn server_line(index: usize) -> String {
    format!(
        "{:>5}  {}",
        index + 1,
        SERVER_MESSAGES[index % SERVER_MESSAGES.len()]
    )
}

#[sample]
#[component]
fn SelectableServerLog() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "Drag across the lines to select them; dragging past an edge scrolls. Select \
                 All and Copy reach every line, including those the list never built."
            </Text>
            <Frame
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Md
            >
                <Scroll max_length=SERVER_LOG_HEIGHT @test_id="rows.server">
                    <Frame
                        padding={Padding::symmetric(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)}
                    >
                        <SelectableText
                            child_size=ItemSize::Percent(100.0)
                            @test_id="rows.server.selection"
                        >
                            <VirtualList
                                keys={(0..SERVER_LINES).collect::<Vec<usize>>()}
                                item_size=SERVER_LINE_HEIGHT
                                text={server_line}
                            >|index: ReadSignal<usize>|
                                <ServerLine index />
                            </VirtualList>
                        </SelectableText>
                    </Frame>
                </Scroll>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn ServerLine(index: ReadSignal<usize>) -> NodeId {
    let line = create_memo(move || server_line(index.get()));
    view! {
        <Text style=TextStyle::CODE>{line}</Text>
    }
}

#[sample]
#[component]
fn CardStrip() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "A horizontal list scrolls with Shift+scroll, a sideways swipe, a touch \
                 drag, or the arrow keys once something in it has focus."
            </Text>
            <Scroll @sizing=ItemSize::Fixed(STRIP_HEIGHT) direction=Direction::Horizontal>
                <VirtualList
                    direction=Direction::Horizontal
                    keys={(0..STRIP_COUNT).collect::<Vec<usize>>()}
                    item_size=STRIP_ITEM_WIDTH
                >|index: ReadSignal<usize>|
                    <StripCard index />
                </VirtualList>
            </Scroll>
        </List>
    }
}

#[sample]
#[component]
fn FittedScroll() -> NodeId {
    let count = create_signal(FITTED_SCROLL_ROWS);
    let rows = create_memo(move || (0..count.get()).collect::<Vec<usize>>());
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "With max_length a scroll is as long as what it holds, up to that length, \
                 and scrolls from there on."
            </Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Button
                    label="Add a row"
                    variant=ButtonVariant::Secondary
                    on_click={move || count.update(|count| *count += 1)}
                />
                <Button
                    label="Remove a row"
                    variant=ButtonVariant::Secondary
                    on_click={move || count.update(|count| *count = count.saturating_sub(1))}
                />
            </List>
            <Frame
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Md
            >
                <Scroll max_length=FITTED_SCROLL_HEIGHT>
                    <ForEach keys={rows}>|index: usize|
                        <Frame
                            padding={Padding::symmetric(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)}
                        >
                            <Text>{format!("Row {}", index + 1)}</Text>
                        </Frame>
                    </ForEach>
                </Scroll>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn FollowingLog() -> NodeId {
    let count = create_signal(LOG_FIRST_LINES);
    let lines = create_memo(move || (0..count.get()).collect::<Vec<usize>>());
    let running = create_signal(false);
    let api = ComponentRef::<ScrollApi>::new();
    let ticker = create_timer(move || {
        count.update(|count| *count += 1);
        Some(LOG_INTERVAL)
    });
    create_effect(move || match running.get() {
        true => ticker.start(LOG_INTERVAL),
        false => ticker.stop(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "With follow_end a scroll stays at its end while what it holds grows, \
                 until you scroll away; scrolling back to the end follows again."
            </Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
                <LabelledSwitch label="Running" on={running} />
                <Button
                    label="Add a line"
                    variant=ButtonVariant::Secondary
                    on_click={move || count.update(|count| *count += 1)}
                />
                <Button
                    label="Jump to the end"
                    variant=ButtonVariant::Secondary
                    on_click={move || api.get().scroll_to_end()}
                />
            </List>
            <Frame
                height=LOG_HEIGHT
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Md
            >
                <Scroll follow_end=true api>
                    <Frame
                        padding={Padding::symmetric(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)}
                    >
                        <VirtualList
                            keys={lines}
                            item_size=LOG_LINE_ESTIMATE
                        >|line: ReadSignal<usize>| {
                            let text = create_memo(move || log_line(line.get()));
                            view! {
                                <Text style=TextStyle::CODE>{text}</Text>
                            }
                        }</VirtualList>
                    </Frame>
                </Scroll>
            </Frame>
        </List>
    }
}

fn log_line(line: usize) -> String {
    let words = LOG_WORDS.len();
    let length = 1 + (line * 7) % (words * 3);
    let text = (0..length)
        .map(|word| LOG_WORDS[(line + word) % words])
        .collect::<Vec<_>>()
        .join(" ");
    format!("{:>4}  {text}", line + 1)
}

#[sample]
#[component]
fn StripCard(index: ReadSignal<usize>) -> NodeId {
    let label = create_memo(move || format!("Card {}", index.get()));
    view! {
        <Frame padding={Padding::symmetric(Space::Xs, Space::None)}>
            <Frame
                fill=ColorRole::SurfaceRaised
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Md
                padding={Padding::symmetric(Space::Lg, Space::Md)}
            >
                <Text align=TextAlign::Center>{label}</Text>
            </Frame>
        </Frame>
    }
}

#[sample]
#[component]
fn ThousandsOfRows() -> NodeId {
    let status = create_signal("Nothing selected".to_owned());
    let selected = create_signal(None::<usize>);
    let selection = create_selector(move || selected.get());
    let timings = create_signal(true);
    let compact = create_signal(false);
    let row_height = create_memo(move || match compact.get() {
        true => COMPACT_ROW_HEIGHT,
        false => ROW_HEIGHT,
    });
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING wrap=true>
                <Checkbox label="Show timings" checked={timings} />
                <LabelledSwitch label="Compact rows" on={compact} />
                <Text
                    style=TextStyle::CAPTION
                    @sizing=ItemSize::Percent(100.0)
                    align=TextAlign::End
                >
                    {status}
                </Text>
            </List>
            <Separator />
            <Scroll @sizing=ItemSize::Fixed(ROWS_HEIGHT)>
                <VirtualList
                    keys={(0..ROW_COUNT).collect::<Vec<usize>>()}
                    item_size={row_height}
                >|index: ReadSignal<usize>|
                    <ScrollRow
                        index
                        selection={selection}
                        timings={timings.read_only()}
                        compact={compact.read_only()}
                        on_click={move || {
                            let index = index.get_untracked();
                            selected.set(Some(index));
                            status.set(format!("Row {index} selected"));
                        }}
                    />
                </VirtualList>
            </Scroll>
        </List>
    }
}

#[sample]
#[component]
fn ScrollRow(
    index: ReadSignal<usize>,
    selection: Selector<Option<usize>>,
    timings: ReadSignal<bool>,
    compact: ReadSignal<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let selected = create_memo(move || selection.is_selected(&Some(index.get())));
    view! {
        <unstyled::Button on_click={on_click}>|handle|
            <ScrollRowFace index handle selected timings compact />
        </unstyled::Button>
    }
}

#[sample]
#[component]
fn ScrollRowFace(
    index: ReadSignal<usize>,
    handle: unstyled::ButtonHandle,
    selected: Memo<bool>,
    timings: ReadSignal<bool>,
    compact: ReadSignal<bool>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered, focused, ..
    } = handle;
    let vertical = create_memo(move || match compact.get() {
        true => COMPACT_ROW_PADDING_VERTICAL,
        false => ROW_PADDING_VERTICAL,
    });
    let label = create_memo(move || format!("Row {}", index.get()));
    let timing = create_memo(move || format!("{} ms", 7 + index.get() * 3 % 91));
    let value_color = create_memo(move || match selected.get() {
        true => TextColor::Accent,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || match (selected.get(), hovered.get()) {
        (true, _) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Hover,
        (false, false) => ColorRole::None,
    });
    view! {
        <Frame
            fill={fill_color}
            border={focus_border(focused, Border::new(ColorRole::Accent).width(Stroke::Focus))}
            radius=Radius::Md
            padding={create_memo(move || Padding::symmetric(ROW_PADDING_HORIZONTAL, vertical.get()))}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                <Text @sizing=ItemSize::Percent(100.0)>{label}</Text>
                <Show condition={timings}>
                    <Text style=TextStyle::CAPTION align=TextAlign::End color={value_color}>
                        {timing}
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn TreePage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Tree"
                code={vec![
                    ProjectTree::SOURCE,
                    tree_item::SOURCE,
                    tree_ancestors::SOURCE,
                    has_children::SOURCE,
                    visible_tree_rows::SOURCE,
                ]}
            >
                <ProjectTree />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ProjectTree() -> NodeId {
    let collapsed = create_signal(Vec::<usize>::new());
    let selected = create_signal(None::<usize>);
    let chosen = create_signal(None::<String>);
    let comfortable = create_signal(false);
    let names = create_signal(TREE_NODES.map(|(name, _)| name.to_owned()).to_vec());
    let editing = create_signal(None::<usize>);
    let name = move |row: usize| names.with(|names| names[row].clone());
    provide_density(create_memo(move || match comfortable.get() {
        true => Density::Comfortable,
        false => Density::Compact,
    }));
    let keys = create_memo(move || visible_tree_rows(&collapsed.get()));
    let status_text = create_memo(move || match (chosen.get(), selected.get()) {
        (Some(chosen), _) => chosen,
        (None, Some(row)) => format!("{} selected", name(row)),
        (None, None) => "Nothing selected".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "Arrow keys walk the tree and Enter selects; the left of a row, or an arrow key, opens and closes a folder; a row's menu opens from its button, a right click or a long press; F2 or Rename edits a name in place, Enter or clicking away keeps it and Escape puts it back"
            </Text>
            <LabelledSwitch
                @test_id="demo.tree.comfortable"
                label="Comfortable rows"
                on={comfortable}
            />
            <Tree
                @sizing=ItemSize::Fixed(TREE_HEIGHT)
                keys
                item={move |row: usize| tree_item(row, &name(row), &collapsed.get())}
                selected
                editing
                ancestors={tree_ancestors}
                row_test_id={|row: usize| format!("demo.tree.{row}")}
                menu={move |row: usize| Some(TreeMenu {
                    items: view! {
                        <unstyled::MenuItem
                            label="Rename"
                            glyph=ICON_PENCIL
                            row_test_id={"demo.tree.rename".to_owned()}
                        />
                        <unstyled::MenuItem label="Delete" glyph=ICON_TRASH danger=true />
                    },
                    on_select: Callback::new(move |path: Vec<usize>| match path.as_slice() {
                        [0] => editing.set(Some(row)),
                        _ => chosen.set(Some(format!("Delete {}", name(row)))),
                    }),
                })}
                on_edit={move |row: usize| editing.set(Some(row))}
                on_rename={move |(row, renamed): (usize, String)| {
                    chosen.set(Some(format!("Renamed {} to {renamed}", name(row))));
                    names.update(|names| names[row] = renamed);
                    editing.set(None);
                }}
                on_cancel_edit={move |row: usize| {
                    chosen.set(Some(format!("Kept {}", name(row))));
                    editing.set(None);
                }}
                on_reveal={move |row: usize| {
                    collapsed.update(|collapsed| {
                        collapsed.retain(|candidate| !tree_ancestors(row).contains(candidate));
                    });
                }}
                on_select={move |row: usize| {
                    chosen.set(None);
                    selected.set(Some(row));
                }}
                on_expand={move |(row, expanded): (usize, bool)| {
                    collapsed.update(|collapsed| {
                        collapsed.retain(|candidate| *candidate != row);
                        if !expanded {
                            collapsed.push(row);
                        }
                    });
                }}
            >|row: TreeRowFace<usize>|
                <Text>{create_memo(move || name(row.key))}</Text>
            </Tree>
            <Text style=TextStyle::CAPTION>{status_text}</Text>
        </List>
    }
}

#[sample]
fn tree_item(row: usize, name: &str, collapsed: &[usize]) -> TreeItem {
    TreeItem {
        label: name.to_owned(),
        depth: TREE_NODES[row].1,
        expandable: has_children(row),
        expanded: !collapsed.contains(&row),
    }
}

#[sample]
fn tree_ancestors(row: usize) -> Vec<usize> {
    let mut depth = TREE_NODES[row].1;
    let mut ancestors: Vec<usize> = (0..row)
        .rev()
        .filter(|candidate| {
            let above = TREE_NODES[*candidate].1 < depth;
            if above {
                depth = TREE_NODES[*candidate].1;
            }
            above
        })
        .collect();
    ancestors.reverse();
    ancestors
}

#[sample]
fn has_children(row: usize) -> bool {
    TREE_NODES
        .get(row + 1)
        .is_some_and(|(_, depth)| *depth > TREE_NODES[row].1)
}

#[sample]
fn visible_tree_rows(collapsed: &[usize]) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut hidden_below: Option<usize> = None;
    for (row, (_, depth)) in TREE_NODES.iter().enumerate() {
        match hidden_below {
            Some(limit) if *depth > limit => continue,
            _ => hidden_below = None,
        }
        rows.push(row);
        if collapsed.contains(&row) {
            hidden_below = Some(*depth);
        }
    }
    rows
}

#[component]
pub(crate) fn LayoutPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Cards in a stack" code={vec![StackedCards::SOURCE]}>
                <StackedCards />
            </Sample>
            <Sample title="Accordions" code={vec![Accordions::SOURCE]}>
                <Accordions />
            </Sample>
            <Sample title="Settings sections" code={vec![SettingsSections::SOURCE]}>
                <SettingsSections />
            </Sample>
            <Sample title="Table" code={vec![PlanetTable::SOURCE]}>
                <PlanetTable />
            </Sample>
            <Sample title="Separators and borders" code={vec![Separators::SOURCE]}>
                <Separators />
            </Sample>
            <Sample title="Frame tokens" code={vec![FrameTokens::SOURCE]}>
                <FrameTokens />
            </Sample>
            <Sample title="Pictures" code={vec![Pictures::SOURCE, sunset::SOURCE]}>
                <Pictures />
            </Sample>
            <Sample title="A scroll inside a scroll" code={vec![NestedScroll::SOURCE]}>
                <NestedScroll />
            </Sample>
            <Sample title="A scroll that bleeds into its card" code={vec![BleedingScroll::SOURCE]}>
                <BleedingScroll />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
fn sunset() -> Image {
    const SIDE: u32 = 48;
    let pixels = (0..SIDE * SIDE)
        .flat_map(|at| {
            let (x, y) = (at % SIDE, at / SIDE);
            let shade = |of: u32| (of * 255 / (SIDE - 1)) as u8;
            [255 - shade(y) / 2, shade(x) / 2 + 64, shade(y), 255]
        })
        .collect();
    Image::from_rgba(SIDE, SIDE, pixels)
}

#[sample]
#[component]
fn Pictures() -> NodeId {
    const RADII: [Radius; 4] = [Radius::None, Radius::Sm, Radius::Lg, Radius::Full];
    let image = sunset();
    view! {
        <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
            <ForEach keys={(0..RADII.len()).collect::<Vec<_>>()}>|index: usize| {
                let image = image.clone();
                view! {
                    <List spacing=Space::Xs>
                        <Frame width={Size::Xl2} height={Size::Xl2}>
                            <Picture image={Some(image)} fit=ImageFit::Fill radius={RADII[index]} />
                        </Frame>
                        <Text style=TextStyle::CAPTION>{RADII[index].name()}</Text>
                    </List>
                }
            }</ForEach>
        </List>
    }
}

#[sample]
#[component]
fn FrameTokens() -> NodeId {
    const BORDERS: [(&str, Border); 4] = [
        ("default", Border::DEFAULT),
        ("focus", Border::FOCUS),
        (
            "danger, focus width",
            Border {
                width: Stroke::Focus,
                ..Border::new(ColorRole::Danger)
            },
        ),
        (
            "focus offset",
            Border {
                offset: Stroke::Focus,
                ..Border::DEFAULT
            },
        ),
    ];
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
                <ForEach keys={ColorRole::ROLES.to_vec()}>|role: ColorRole|
                    <List spacing=Space::Xs>
                        <Frame
                            width={Size::Custom(88.0)}
                            height={Size::Lg}
                            fill=role
                            border=Border::DEFAULT
                            radius=Radius::Sm
                        />
                        <Text style=TextStyle::CAPTION>{role.name()}</Text>
                    </List>
                </ForEach>
            </List>
            <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
                <ForEach keys={(0..Radius::ALL.len()).collect::<Vec<_>>()}>|index: usize|
                    <List spacing=Space::Xs>
                        <Frame
                            width={Size::Xl2}
                            height={Size::Xl2}
                            fill=ColorRole::AccentSoft
                            radius={Radius::ALL[index]}
                        />
                        <Text style=TextStyle::CAPTION>{Radius::ALL[index].name()}</Text>
                    </List>
                </ForEach>
            </List>
            <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
                <ForEach keys={(0..Space::ALL.len()).collect::<Vec<_>>()}>|index: usize|
                    <List spacing=Space::Xs>
                        <Frame fill=ColorRole::Surface border=Border::DEFAULT radius=Radius::Sm>
                            <Frame padding={Space::ALL[index]}>
                                <Frame width=Size::Xs height=Size::Xs fill=ColorRole::AccentSoft />
                            </Frame>
                        </Frame>
                        <Text style=TextStyle::CAPTION>{Space::ALL[index].name()}</Text>
                    </List>
                </ForEach>
            </List>
            <List direction=Direction::Horizontal wrap=true align=Align::End spacing=ROW_SPACING>
                <ForEach keys={(0..7).collect::<Vec<_>>()}>|index: usize|
                    <List spacing=Space::Xs>
                        <Frame
                            width={Size::ALL[index]}
                            height={Size::ALL[index]}
                            fill=ColorRole::AccentSoft
                            radius=Radius::Sm
                        />
                        <Text style=TextStyle::CAPTION>{Size::ALL[index].name()}</Text>
                    </List>
                </ForEach>
            </List>
            <List direction=Direction::Horizontal wrap=true spacing=ROW_SPACING>
                <ForEach keys={(0..BORDERS.len()).collect::<Vec<_>>()}>|index: usize|
                    <List spacing=Space::Xs>
                        <Frame
                            width={Size::Custom(88.0)}
                            height={Size::Xl2}
                            fill=ColorRole::Surface
                            border={Some(BORDERS[index].1)}
                            radius=Radius::Md
                        />
                        <Text style=TextStyle::CAPTION>{BORDERS[index].0}</Text>
                    </List>
                </ForEach>
            </List>
        </List>
    }
}

#[sample]
#[component]
fn StackedCards() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::CAPTION>
                "A stack lays its children side by side, and one above the other once it is narrow."
            </Text>
            <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                <Card @sizing=ItemSize::Percent(50.0)>
                    <List spacing=Space::Xs>
                        <Text style=TextStyle::CAPTION>"Left"</Text>
                        <Text style=TextStyle::DISPLAY>"42"</Text>
                    </List>
                </Card>
                <Card @sizing=ItemSize::Percent(50.0)>
                    <List spacing=Space::Xs>
                        <Text style=TextStyle::CAPTION>"Right"</Text>
                        <Text style=TextStyle::DISPLAY>"17"</Text>
                    </List>
                </Card>
            </Stack>
        </List>
    }
}

#[sample]
#[component]
fn Accordions() -> NodeId {
    view! {
        <Card>
            <List spacing=ROW_SPACING>
                <Accordion title="About" open=true>
                    <Text style=TextStyle::PARAGRAPH>
                        "beui keeps a retained tree of nodes. Base nodes carry \
                         behaviour only, unstyled components compose them, and the \
                         styled components paint them."
                    </Text>
                </Accordion>
                <Separator />
                <Accordion title="Layout" open=false>
                    <Text style=TextStyle::PARAGRAPH>
                        "Layout is linear in the number of nodes, and only the \
                         parts of the tree whose inputs changed are laid out again."
                    </Text>
                </Accordion>
            </List>
        </Card>
    }
}

#[sample]
#[component]
fn PlanetTable() -> NodeId {
    const PLANETS: [(&str, &str, &str, &str); 3] = [
        ("Mercury", "88 days", "None", ""),
        ("Earth", "365 days", "1", "The Moon"),
        ("Jupiter", "12 years", "95", "Four large ones"),
    ];
    view! {
        <SettingsSection
            title="Planets"
            description="A table heads its rows down the side and its columns along the top. A cell can carry a caption under its value."
        >
            <Table
                label="Planets"
                heading="Planet"
                columns={vec![TableColumn::new("Year"), TableColumn::new("Moons").caption("Known")]}
            >
                <ForEach
                    keys={PLANETS.to_vec()}
                >|(name, year, moons, caption): (&'static str, &'static str, &'static str, &'static str)|
                    <TableRow
                        label={name.to_owned()}
                        @test_id={format!("table.{}", name.to_lowercase())}
                    >
                        <TableCell
                            text={year.to_owned()}
                            color=TextColor::Muted
                            @test_id={format!("table.{}.year", name.to_lowercase())}
                        />
                        <TableCell
                            text={moons.to_owned()}
                            caption={caption.to_owned()}
                            @test_id={format!("table.{}.moons", name.to_lowercase())}
                        />
                    </TableRow>
                </ForEach>
            </Table>
        </SettingsSection>
    }
}

#[sample]
#[component]
fn SettingsSections() -> NodeId {
    let sounds = create_signal(true);
    let volume = create_signal(DEFAULT_VOLUME);
    let size = create_signal(Some(1));
    let effects = create_signal(true);
    let volume_changed = create_memo(move || volume.get() != DEFAULT_VOLUME);
    let size_changed = create_memo(move || size.get() != Some(1));
    let haptics = create_signal(false);
    let haptics_pinned = create_signal(true);
    let haptics_changed = create_memo(move || haptics.get());
    let pinned = create_memo(move || Some(haptics_pinned.get()));
    let note = create_memo(move || match haptics_pinned.get() {
        true => "Kept on this device".to_owned(),
        false => String::new(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <SettingsSection
                title="Sound"
                description="A row lays its label beside its control, and stacks them once narrow. A changed setting is marked, and its menu brings back the default and can hold more, with a note under its label."
            >
                <SettingsRow
                    label="Sounds"
                    description="Play a sound when something finishes."
                    compact=true
                >
                    <Switch label="Sounds" on={sounds} />
                </SettingsRow>
                <SettingsRow
                    label="Volume"
                    separated=true
                    modified={volume_changed}
                    menu={view! {
                        <unstyled::MenuItem
                            label="Reset to default"
                            glyph={beui::icons::ICON_ROTATE_CCW.to_owned()}
                            on_click={move || volume.set(DEFAULT_VOLUME)}
                        />
                    }}
                >
                    <Slider
                        @sizing=ItemSize::Percent(100.0)
                        value={volume}
                        min=0.0
                        max=1.0
                        label="Volume"
                    />
                </SettingsRow>
                <SettingsRow
                    label="Text size"
                    separated=true
                    modified={size_changed}
                    menu={view! {
                        <unstyled::MenuItem
                            label="Reset to default"
                            glyph={beui::icons::ICON_ROTATE_CCW.to_owned()}
                            on_click={move || size.set(Some(1))}
                        />
                    }}
                >
                    <Select
                        @sizing=ItemSize::Percent(100.0)
                        width={Extent::fraction(1.0)}
                        options={view! {
                            <ChoiceOption label="Small" />
                            <ChoiceOption label="Medium" />
                            <ChoiceOption label="Large" />
                        }}
                        selected={size}
                        label="Text size"
                    />
                </SettingsRow>
                <SettingsRow
                    label="Vibrate"
                    separated=true
                    compact=true
                    modified={haptics_changed}
                    note={note}
                    menu={view! {
                        <unstyled::MenuItem
                            label="Keep on this device"
                            glyph={beui::icons::ICON_PIN.to_owned()}
                            checked={pinned}
                            on_click={move || haptics_pinned.update(|on| *on = !*on)}
                        />
                        <unstyled::MenuItem
                            label="Reset to default"
                            glyph={beui::icons::ICON_ROTATE_CCW.to_owned()}
                            on_click={move || haptics.set(false)}
                        />
                    }}
                >
                    <Switch label="Vibrate" on={haptics} />
                </SettingsRow>
                <SettingsLink
                    label="Sound effects"
                    description="A link row opens a page of its own."
                    separated=true
                    modified={effects}
                    on_click={|| {}}
                />
            </SettingsSection>
        </List>
    }
}

#[sample]
#[component]
fn Separators() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text>"One"</Text>
                <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                <Text>"Two"</Text>
                <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                <Text>"Three"</Text>
            </List>
            <Separator />
            <Frame border={Border::DEFAULT} radius=Radius::Lg padding=ROW_SPACING>
                <Text>"A border drawn around whatever it holds"</Text>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn NestedScroll() -> NodeId {
    view! {
        <Card>
            <List spacing=Space::None>
                <Scroll @sizing=ItemSize::Fixed(NESTED_SCROLL_HEIGHT)>
                    <List spacing=Space::Xs>
                        <ForEach
                            keys={(1..=NESTED_SCROLL_ROWS).collect::<Vec<usize>>()}
                        >|line: usize|
                            <Text>{format!("Line {line}")}</Text>
                        </ForEach>
                    </List>
                </Scroll>
            </List>
        </Card>
    }
}

#[sample]
#[component]
fn BleedingScroll() -> NodeId {
    view! {
        <Card>
            <List spacing=Space::None>
                <Scroll @sizing=ItemSize::Fixed(NESTED_SCROLL_HEIGHT) bleed=Bleed::CARD>
                    <List spacing=Space::Md>
                        <ForEach
                            keys={(1..=NESTED_SCROLL_ROWS).collect::<Vec<usize>>()}
                        >|topic: usize|
                            <Checkbox label={format!("Topic {topic}")} checked=false />
                        </ForEach>
                    </List>
                </Scroll>
            </List>
        </Card>
    }
}

#[component]
pub(crate) fn EditorPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Markdown editor" code={vec![MarkdownEditor::SOURCE]}>
                <MarkdownEditor />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn MarkdownEditor() -> NodeId {
    let document = Arc::new(TextBuffer::new(EDITOR_TEXT)) as Arc<dyn text_editor_core::Document>;
    let state = TextAreaState::new(document);
    state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Button
                    label="Bold"
                    variant=ButtonVariant::Secondary
                    on_click={move || state.execute(EditorCommand::Markdown(MarkdownCommand::Bold))}
                />
                <Button
                    label="Italic"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        state.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
                    }}
                />
            </List>
            <TextArea @sizing=ItemSize::Fixed(EDITOR_HEIGHT) state={state} />
        </List>
    }
}

#[component]
pub(crate) fn CanvasPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Pan and zoom"
                code={vec![
                    ZoomableStage::SOURCE,
                    scale_out::SOURCE,
                    scale_in::SOURCE,
                    CanvasStage::SOURCE,
                    CanvasBoard::SOURCE,
                    StageCard::SOURCE,
                ]}
            >
                <ZoomableStage />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ZoomableStage() -> NodeId {
    let stage_view = create_signal(STAGE_VIEW);
    let zoom_label = create_memo(move || format!("{:.0}%", stage_view.get().scale * 100.0));
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>{zoom_label}</Text>
                <Button
                    @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                    label="-"
                    variant=ButtonVariant::Secondary
                    on_click={move || stage_view.update(scale_out)}
                />
                <Button
                    @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                    label="+"
                    variant=ButtonVariant::Secondary
                    on_click={move || stage_view.update(scale_in)}
                />
                <Button
                    label="Reset"
                    variant=ButtonVariant::Secondary
                    on_click={move || stage_view.set(STAGE_VIEW)}
                />
            </List>
            <CanvasStage @sizing=ItemSize::Fixed(STAGE_HEIGHT) view={stage_view} />
            <Text style=TextStyle::CAPTION>
                "Scroll to pan, Shift+scroll sideways, Ctrl+scroll or pinch to zoom, \
                 and drag with the middle button. Tab to it for arrows, + and -."
            </Text>
        </List>
    }
}

#[sample]
fn scale_out(view: &mut PanZoomView) {
    view.scale = (view.scale / STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

#[sample]
fn scale_in(view: &mut PanZoomView) {
    view.scale = (view.scale * STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

#[sample]
#[component]
fn CanvasStage(#[prop(default = PanZoomView::IDENTITY)] view: Binding<PanZoomView>) -> NodeId {
    view! {
        <PanZoom view>|handle: PanZoomHandle| {
            let PanZoomHandle { view, focused, .. } = handle;
            view! {
                <CanvasBoard view focused />
            }
        }</PanZoom>
    }
}

#[sample]
#[component]
fn CanvasBoard(view: Memo<Option<CanvasView>>, focused: ReadSignal<bool>) -> NodeId {
    let [first, second, third, fourth] = STAGE_CARDS;
    view! {
        <Frame
            border={focus_border(focused, Border::new(ColorRole::Accent).width(Stroke::Focus))}
            radius=Radius::Md
        >
            <Canvas view>
                <StageCard card=first />
                <StageCard card=second />
                <StageCard card=third />
                <StageCard card=fourth />
            </Canvas>
        </Frame>
    }
}

#[sample]
#[component]
fn StageCard(card: (f32, f32, f32, f32, &'static str)) -> CanvasItem {
    let (x, y, width, height, label) = card;
    view! {
        <CanvasItem x y width height scales=true>
            <Frame
                fill=ColorRole::SurfaceRaised
                border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                radius=Radius::Lg
                padding=STAGE_PADDING
            >
                <Text size=TextSize::Custom(STAGE_LABEL_SIZE)>{label.to_string()}</Text>
            </Frame>
        </CanvasItem>
    }
}

#[component]
pub(crate) fn ThemesPage() -> NodeId {
    let windowed = use_context::<DemoWindow>().is_some();
    view! {
        <ScrollPage>
            <Sample
                title="Themes"
                code={vec![ThemedSamples::SOURCE, ThemeSample::SOURCE, LabelledSwitch::SOURCE]}
            >
                <ThemedSamples />
            </Sample>
            <Show condition={windowed}>
                <Sample title="Window" code={vec![WindowSample::SOURCE]}>
                    <WindowSample />
                </Sample>
            </Show>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ThemedSamples() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::PARAGRAPH>
                "Theme, in the toolbar, changes the theme of the whole document. \
                 A theme provider restyles only what it holds, as the samples below show."
            </Text>
            <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::DARK>
                    <ThemeSample name="Dark" />
                </ThemeProvider>
                <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::OLED>
                    <ThemeSample name="OLED dark" />
                </ThemeProvider>
            </Stack>
            <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::LIGHT>
                    <ThemeSample name="Light" />
                </ThemeProvider>
                <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::EPAPER>
                    <ThemeSample name="E-paper" />
                </ThemeProvider>
            </Stack>
        </List>
    }
}

#[sample]
#[component]
fn WindowSample() -> NodeId {
    let window = use_context::<DemoWindow>().expect("the demo provides its window");
    view! {
        <List spacing=SECTION_SPACING>
            <Text style=TextStyle::PARAGRAPH>
                "The Root at the top of the document sets the window's title and the \
                 colour it is cleared to, which shows wherever nothing paints over it."
            </Text>
            <TextInput @test_id="demo.window.title" value={window.title} label="Window title" />
            <RadioGroup
                options={view! {
                    <ChoiceOption label="Background" />
                    <ChoiceOption label="Surface" />
                    <ChoiceOption label="Raised surface" />
                }}
                selected={window.background}
            />
        </List>
    }
}

#[sample]
#[component]
fn ThemeSample(name: &'static str) -> NodeId {
    view! {
        <Frame fill=ColorRole::Background radius=Radius::Lg padding=ROW_SPACING>
            <Card>
                <List spacing=SECTION_SPACING>
                    <Text style=TextStyle::HEADING>{name}</Text>
                    <Text style=TextStyle::CAPTION>
                        "Every styled component reads its colors from the theme."
                    </Text>
                    <Checkbox label="A checkbox" checked=true />
                    <LabelledSwitch label="A switch" on=true />
                    <Slider value=0.6 label="A slider" />
                    <Progress value={Some(0.6)} />
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=Space::Md
                        wrap=true
                    >
                        <Button label="Primary" variant=ButtonVariant::Primary />
                        <Button label="Secondary" variant=ButtonVariant::Secondary />
                    </List>
                </List>
            </Card>
        </Frame>
    }
}
