use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_BEDTIME, ICON_LOCK, ICON_LOGOUT, ICON_POWER_SETTINGS_NEW, ICON_RESTART_ALT,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, Justify, List, Memo, clone, component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{Button, ButtonVariant, Dialog, MenuButton, Paragraph};
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::{Editor, Power, PowerAction, PowerAvailability};

const DIALOG_SPACING: f32 = 16.0;
const BUTTON_SPACING: f32 = 8.0;

fn label(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Lock => "Lock",
        PowerAction::Suspend => "Suspend",
        PowerAction::Restart => "Restart",
        PowerAction::PowerOff => "Power off",
        PowerAction::LogOut => "Log out",
    }
}

fn glyph(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Lock => ICON_LOCK,
        PowerAction::Suspend => ICON_BEDTIME,
        PowerAction::Restart => ICON_RESTART_ALT,
        PowerAction::PowerOff => ICON_POWER_SETTINGS_NEW,
        PowerAction::LogOut => ICON_LOGOUT,
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
            row_test_id={format!("desktop.power.{}", key(action))}
        />
    }
}

#[component]
pub(crate) fn PowerMenu(editor: Editor) -> NodeId {
    let available = editor.host_value::<Power>();
    let allowed = available.clone();
    let (confirming, set_confirming) = create_signal(PowerAction::LogOut);
    let (open, set_open) = create_signal(false);
    let requesting = editor.clone();
    let chose = clone!(set_open -> move |path: Vec<usize>| {
        let Some(action) = path
            .first()
            .and_then(|index| PowerAction::ALL.get(*index).copied())
        else {
            return;
        };
        if !allowed.get_untracked().allows(action) {
            return;
        }
        match action {
            PowerAction::Lock | PowerAction::Suspend => requesting.act(action),
            _ => {
                set_confirming.set(action);
                set_open.set(true);
            }
        }
    });
    let title = create_memo(clone!(confirming -> move || question(confirming.get())));
    let explained =
        create_memo(clone!(confirming -> move || consequence(confirming.get()).to_owned()));
    let verb = create_memo(clone!(confirming -> move || label(confirming.get()).to_owned()));
    let dismiss = clone!(set_open -> move || set_open.set(false));
    let cancel = clone!(set_open -> move || set_open.set(false));
    let confirm = move || {
        set_open.set(false);
        editor.act(confirming.get_untracked());
    };
    view! {
        <List spacing=0.0>
            <MenuButton
                label="Power"
                glyph={ICON_POWER_SETTINGS_NEW.to_owned()}
                icon_only=true
                arrow=false
                @test_id={"desktop.power"}
                items={view! {
                    <PowerItem action=PowerAction::Lock available={available.clone()} />
                    <PowerItem action=PowerAction::Suspend available={available.clone()} />
                    <PowerItem action=PowerAction::Restart available={available.clone()} />
                    <PowerItem action=PowerAction::PowerOff available={available.clone()} />
                    <PowerItem action=PowerAction::LogOut available={available.clone()} />
                }}
                on_select={chose}
            />
            <Dialog open title on_dismiss={dismiss}>
                <List spacing=DIALOG_SPACING @test_id={"desktop.power.dialog"}>
                    <Paragraph content={explained} />
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        justify=Justify::End
                        spacing=BUTTON_SPACING
                    >
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            @test_id={"desktop.power.cancel"}
                            on_click={cancel}
                        />
                        <Button
                            label={verb}
                            variant=ButtonVariant::Primary
                            @test_id={"desktop.power.confirm"}
                            on_click={confirm}
                        />
                    </List>
                </List>
            </Dialog>
        </List>
    }
}
