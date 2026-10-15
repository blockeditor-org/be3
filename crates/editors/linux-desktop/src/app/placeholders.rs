use block_editor_beui::beui::icons::{ICON_OPEN_IN_NEW, ICON_REOPEN_WINDOW, ICON_WEB_ASSET};
use block_editor_beui::beui::reactive::{
    Align, ClickCallback, Direction, Frame, Justify, List, Memo, Show, StoredCell, component,
    create_effect, create_memo, pixels_per_point, untrack, view,
};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Icon, IconButton, Text, TextColor, TextSize, TextStyle, use_theme,
};
use block_editor_beui::beui::unstyled::{DockNode, DockPane, DockTab, DockWindow, Picture, TabId};
use block_editor_beui::beui::{Image, NodeId, Rect, pos2, vec2};
use block_editor_beui::{HostProgram, HostWindowId, ProgramAction, Programs};
use block_shell::{KeptWindow, Workspace, placeholder_tab};

const ICON_POINTS: f32 = 64.0;
const PICTURE_SIZE: f32 = 64.0;
const BODY_SPACING: f32 = 12.0;
const BUTTON_SPACING: f32 = 8.0;
const BODY_PADDING: f32 = 24.0;
const WINDOW_ORIGIN: f32 = 80.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const WINDOW_WIDTH: f32 = 800.0;
const WINDOW_HEIGHT: f32 = 500.0;

#[derive(Clone, Copy)]
pub(crate) struct Placeholders {
    workspace: Workspace,
    programs: Memo<Vec<HostProgram>>,
    pub(crate) keys: Memo<Vec<HostWindowId>>,
}

impl Placeholders {
    pub(crate) fn new(workspace: Workspace) -> Self {
        let editor = workspace.editor();
        let programs = editor.host_value::<Programs>();
        let keys = workspace.placeholders();
        let scale = pixels_per_point();
        let listed = StoredCell::new(false);
        create_effect(move || {
            if keys.with(Vec::is_empty) || listed.get() {
                return;
            }
            listed.set(true);
            let icon_size = (ICON_POINTS * scale.get_untracked()).ceil() as u32;
            untrack(|| editor.act(ProgramAction::List { icon_size }));
        });
        Self {
            workspace,
            programs,
            keys,
        }
    }

    fn kept(&self, key: HostWindowId) -> Memo<Option<KeptWindow>> {
        let workspace = self.workspace;
        create_memo(move || workspace.placeholder(key))
    }

    fn program(&self, kept: Memo<Option<KeptWindow>>) -> Memo<Option<HostProgram>> {
        let programs = self.programs;
        create_memo(move || {
            kept.with(|kept| {
                let kept = kept.as_ref()?;
                programs.with(|programs| program_for(kept, programs).cloned())
            })
        })
    }

    pub(crate) fn open(&self, key: HostWindowId) {
        let Some(kept) = self.workspace.placeholder(key) else {
            return;
        };
        let action = self
            .programs
            .with_untracked(|programs| launch_action(&kept, programs));
        if let Some(action) = action {
            self.workspace.editor().act(action);
        }
    }

    pub(crate) fn open_all(&self) {
        for key in self.keys.get_untracked() {
            self.open(key);
        }
    }
}

pub(crate) fn program_for<'a>(
    kept: &KeptWindow,
    programs: &'a [HostProgram],
) -> Option<&'a HostProgram> {
    if let Some(program) = &kept.origin.program {
        return programs.iter().find(|listed| listed.id == *program);
    }
    let app_id = kept.app_id.as_str();
    if app_id.is_empty() {
        return None;
    }
    let stem = |program: &HostProgram| program.id.trim_end_matches(".desktop").to_owned();
    programs
        .iter()
        .find(|program| {
            program
                .wm_class
                .as_deref()
                .is_some_and(|class| class.eq_ignore_ascii_case(app_id))
        })
        .or_else(|| {
            programs
                .iter()
                .find(|program| stem(program).eq_ignore_ascii_case(app_id))
        })
        .or_else(|| {
            programs.iter().find(|program| {
                stem(program)
                    .rsplit('.')
                    .next()
                    .is_some_and(|last| last.eq_ignore_ascii_case(app_id))
            })
        })
}

pub(crate) fn launch_action(kept: &KeptWindow, programs: &[HostProgram]) -> Option<ProgramAction> {
    let launch = Some(kept.key.0);
    let program = kept
        .origin
        .program
        .clone()
        .or_else(|| program_for(kept, programs).map(|program| program.id.clone()));
    match program {
        Some(program) => Some(ProgramAction::Launch { program, launch }),
        None if !kept.origin.command.is_empty() => Some(ProgramAction::Exec {
            command: kept.origin.command.clone(),
            launch,
        }),
        None => None,
    }
}

fn name(kept: &KeptWindow, program: Option<&HostProgram>) -> String {
    if let Some(program) = program {
        return program.name.clone();
    }
    let command = kept.origin.command.first().map(|command| {
        command
            .rsplit('/')
            .next()
            .unwrap_or(command.as_str())
            .to_owned()
    });
    match (kept.app_id.is_empty(), command) {
        (false, _) => kept.app_id.clone(),
        (true, Some(command)) => command,
        (true, None) => "Program".to_owned(),
    }
}

fn place(key: HostWindowId) -> Rect {
    let step = (key.0 % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
    Rect::from_min_size(
        pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step),
        vec2(WINDOW_WIDTH, WINDOW_HEIGHT),
    )
}

#[component]
pub(crate) fn PlaceholderWindow(placeholders: Placeholders, key: HostWindowId) -> DockNode<TabId> {
    let workspace = placeholders.workspace;
    let kept = placeholders.kept(key);
    let program = placeholders.program(kept);
    let name = create_memo(move || {
        kept.with(|kept| {
            kept.as_ref()
                .map_or_else(String::new, |kept| name(kept, program.get().as_ref()))
        })
    });
    let title = create_memo(move || {
        kept.with(|kept| match kept {
            Some(kept) if !kept.title.is_empty() => kept.title.clone(),
            _ => name.get(),
        })
    });
    let picture = create_memo(move || {
        program.with(|program| {
            let icon = program.as_ref()?.icon.as_ref()?;
            icon.is_whole()
                .then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
        })
    });
    let openable = create_memo(move || {
        kept.with(|kept| {
            kept.as_ref().is_some_and(|kept| {
                placeholders
                    .programs
                    .with(|programs| launch_action(kept, programs).is_some())
            })
        })
    });
    let pane = format!("window.{}", key.0);
    view! {
        <DockWindow id={pane.clone()} rect={place(key)}>
            <DockPane id={pane}>
                <DockTab
                    id={placeholder_tab(key)}
                    title
                    icon=ICON_WEB_ASSET
                    picture
                    on_close={move || workspace.dismiss(key)}
                >
                    <PlaceholderBody
                        key
                        name
                        title
                        picture
                        openable
                        on_open={move || placeholders.open(key)}
                        on_dismiss={move || workspace.dismiss(key)}
                    />
                </DockTab>
            </DockPane>
        </DockWindow>
    }
}

#[component]
fn PlaceholderBody(
    key: HostWindowId,
    name: Memo<String>,
    title: Memo<String>,
    picture: Memo<Option<Image>>,
    openable: Memo<bool>,
    on_open: ClickCallback,
    on_dismiss: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let pictured = create_memo(move || picture.with(Option::is_some));
    let plain = create_memo(move || !pictured.get());
    let unopenable = create_memo(move || !openable.get());
    let shown_title = create_memo(move || match title.get() == name.get() {
        true => String::new(),
        false => title.get(),
    });
    view! {
        <Frame
            color={theme.background}
            padding_horizontal=BODY_PADDING
            padding_vertical=BODY_PADDING
            @test_id={format!("desktop.placeholder.{}", key.0)}
        >
            <List spacing=BODY_SPACING align=Align::Center justify=Justify::Center>
                <Show condition={pictured}>
                    <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
                        <Picture image={picture} />
                    </Frame>
                </Show>
                <Show condition={plain}>
                    <Text size=TextSize::Xl5 color=TextColor::Muted>
                        <Icon glyph={ICON_WEB_ASSET.to_owned()} />
                    </Text>
                </Show>
                <Text style=TextStyle::HEADING string={name} />
                <Text style=TextStyle::CAPTION string={shown_title} />
                <List direction=Direction::Horizontal align=Align::Center spacing=BUTTON_SPACING>
                    <Button
                        label="Open"
                        glyph={ICON_OPEN_IN_NEW.to_owned()}
                        variant=ButtonVariant::Primary
                        disabled={unopenable}
                        @test_id={format!("desktop.placeholder.{}.open", key.0)}
                        on_click={on_open}
                    />
                    <Button
                        label="Dismiss"
                        variant=ButtonVariant::Secondary
                        @test_id={format!("desktop.placeholder.{}.dismiss", key.0)}
                        on_click={on_dismiss}
                    />
                </List>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn OpenAllButton(placeholders: Placeholders) -> NodeId {
    let keys = placeholders.keys;
    let label = create_memo(move || match keys.with(Vec::len) {
        1 => "Open the window from last time".to_owned(),
        count => format!("Open the {count} windows from last time"),
    });
    view! {
        <IconButton
            glyph={ICON_REOPEN_WINDOW.to_owned()}
            label={label}
            @test_id={"desktop.placeholders.open_all"}
            on_click={move || placeholders.open_all()}
        />
    }
}
