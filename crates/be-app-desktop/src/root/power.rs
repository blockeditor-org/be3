use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{ICON_LOCK, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::desktop;
use be_sdk_beui::{Editor, PowerAction, PowerAvailability, PowerWatch};
use beui::{Align, Direction};

const DIALOG_SPACING: Space = Space::Xl;
const ROW_SPACING: Space = Space::Xs;

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

pub(crate) fn unavailable(available: PowerAvailability, action: PowerAction) -> &'static str {
    match () {
        _ if available.allows(action) => "",
        _ if action == PowerAction::Lock => "The screen is already locked",
        _ if available.ending.is_some() => "The session is ending",
        _ if !available.session => "Only when be3 is your login session",
        _ => "This computer does not allow it",
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

#[derive(Clone, Copy)]
pub(crate) struct PowerChoice {
    editor: Editor,
    available: Memo<PowerAvailability>,
    confirming: Signal<PowerAction>,
    asking: Signal<bool>,
}

impl PowerChoice {
    pub(crate) fn new(editor: Editor) -> Self {
        Self {
            editor,
            available: editor.watch::<PowerWatch>(),
            confirming: create_signal(PowerAction::LogOut),
            asking: create_signal(false),
        }
    }

    fn choose(self, action: PowerAction) {
        if !self.available.get_untracked().allows(action) {
            return;
        }
        match action {
            PowerAction::Lock | PowerAction::Suspend => self
                .editor
                .act(move || desktop::power::request(action).detach()),
            _ => {
                self.confirming.set(action);
                self.asking.set(true);
            }
        }
    }
}

#[component]
pub(crate) fn PowerRow(choice: PowerChoice, on_chose: Callback<()>) -> NodeId {
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=ROW_SPACING
            @test_id={"desktop.power"}
        >
            <PowerButton action=PowerAction::Lock choice={choice} on_chose={on_chose} />
            <PowerButton action=PowerAction::Suspend choice={choice} on_chose={on_chose} />
            <Spacer />
            <PowerButton action=PowerAction::LogOut choice={choice} on_chose={on_chose} />
            <PowerButton action=PowerAction::Restart choice={choice} on_chose={on_chose} />
            <PowerButton action=PowerAction::PowerOff choice={choice} on_chose={on_chose} />
        </List>
    }
}

#[component]
fn PowerButton(action: PowerAction, choice: PowerChoice, on_chose: Callback<()>) -> NodeId {
    let available = choice.available;
    let disabled = create_memo(move || !available.get().allows(action));
    let description = create_memo(move || unavailable(available.get(), action).to_owned());
    view! {
        <Button
            icon_only=true
            glyph={glyph(action)}
            label={label(action).to_owned()}
            disabled
            description
            @test_id={format!("desktop.power.{}", key(action))}
            on_click={move || {
                on_chose.call(());
                choice.choose(action);
            }}
        />
    }
}

#[component]
pub(crate) fn PowerDialog(choice: PowerChoice) -> NodeId {
    let PowerChoice {
        editor,
        confirming,
        asking,
        ..
    } = choice;
    let title = create_memo(move || question(confirming.get()));
    let explained = create_memo(move || consequence(confirming.get()).to_owned());
    let verb = create_memo(move || label(confirming.get()).to_owned());
    let dismiss = move || asking.set(false);
    let confirm = move || {
        asking.set(false);
        editor.act(move || desktop::power::request(confirming.get_untracked()).detach());
    };
    view! {
        <Dialog
            open={asking}
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
    }
}
