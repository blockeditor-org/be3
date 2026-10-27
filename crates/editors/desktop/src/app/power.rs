use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_LOCK, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::desktop;
use block_editor_beui::{Editor, PowerAction, PowerAvailability, PowerWatch};

const DIALOG_SPACING: Space = Space::Xl;

fn label(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Lock => "Lock",
        PowerAction::Suspend => "Suspend",
        PowerAction::Restart => "Restart",
        PowerAction::PowerOff => "Power off",
        PowerAction::LogOut => "Log out",
    }
}

fn glyph(action: PowerAction) -> Glyph {
    match action {
        PowerAction::Lock => ICON_LOCK,
        PowerAction::Suspend => ICON_MOON,
        PowerAction::Restart => ICON_ROTATE_CCW,
        PowerAction::PowerOff => ICON_POWER,
        PowerAction::LogOut => ICON_LOG_OUT,
    }
}

fn key(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Lock => "lock",
        PowerAction::Suspend => "suspend",
        PowerAction::Restart => "restart",
        PowerAction::PowerOff => "power-off",
        PowerAction::LogOut => "log-out",
    }
}

fn question(action: PowerAction) -> String {
    format!("{}?", label(action))
}

fn consequence(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Lock => "The screen locks until your password is entered.",
        PowerAction::Suspend => "The computer goes to sleep.",
        PowerAction::Restart => "Every program is asked to close, then the computer restarts.",
        PowerAction::PowerOff => "Every program is asked to close, then the computer turns off.",
        PowerAction::LogOut => "Every program is asked to close, then the session ends.",
    }
}

#[component]
fn PowerItem(action: PowerAction, available: Memo<PowerAvailability>) -> MenuItem {
    let disabled = create_memo(move || !available.get().allows(action));
    view! {
        <MenuItem
            label={label(action).to_owned()}
            glyph={glyph(action).to_owned()}
            disabled
            @test_id={format!("desktop.power.{}", key(action))}
        />
    }
}

#[component]
pub(crate) fn PowerMenu(editor: Editor) -> NodeId {
    let available = editor.watch::<PowerWatch>();
    let confirming = create_signal(PowerAction::LogOut);
    let open = create_signal(false);
    let chose = move |path: Vec<usize>| {
        let Some(action) = path
            .first()
            .and_then(|index| PowerAction::ALL.get(*index).copied())
        else {
            return;
        };
        if !available.get_untracked().allows(action) {
            return;
        }
        match action {
            PowerAction::Lock | PowerAction::Suspend => {
                editor.act(move || desktop::power::request(action))
            }
            _ => {
                confirming.set(action);
                open.set(true);
            }
        }
    };
    let title = create_memo(move || question(confirming.get()));
    let explained = create_memo(move || consequence(confirming.get()).to_owned());
    let verb = create_memo(move || label(confirming.get()).to_owned());
    let dismiss = move || open.set(false);
    let confirm = move || {
        open.set(false);
        editor.act(move || desktop::power::request(confirming.get_untracked()));
    };
    view! {
        <List spacing=Space::None>
            <MenuButton
                label="Power"
                glyph=ICON_POWER
                icon_only=true
                arrow=false
                @test_id={"desktop.power"}
                items={view! {
                    <PowerItem action=PowerAction::Lock available={available} />
                    <PowerItem action=PowerAction::Suspend available={available} />
                    <PowerItem action=PowerAction::Restart available={available} />
                    <PowerItem action=PowerAction::PowerOff available={available} />
                    <PowerItem action=PowerAction::LogOut available={available} />
                }}
                on_select={chose}
            />
            <Dialog
                open
                title
                kind=DialogKind::Alert
                id="desktop.power"
                actions={vec![DialogAction::primary(verb, confirm), DialogAction::cancel("Cancel")]}
                on_dismiss={dismiss}
            >
                <List spacing=DIALOG_SPACING @test_id={"desktop.power.dialog"}>
                    <Text style=TextStyle::PARAGRAPH>{explained}</Text>
                </List>
            </Dialog>
        </List>
    }
}
