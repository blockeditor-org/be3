use beui::{Align, Direction, ItemSize};
use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, icons::*};
use uuid::Uuid;

const PADDING: Space = Space::Xl3;

pub fn setup_command(block: Uuid) -> String {
    format!("BE3_AGENT_CREDENTIALS='<the agent's credentials>' be {block} terminal-host")
}

#[component]
pub(crate) fn Offline(editor: Editor) -> NodeId {
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=Space::Xl>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Text size=TextSize::Xl2>
                            <Icon glyph={ICON_UNPLUG.to_owned()} />
                        </Text>
                        <Text style=TextStyle::TITLE @test_id={"terminal.offline"}>
                            "No machine is running this terminal"
                        </Text>
                    </List>
                    <SetupSteps editor={editor} />
                    <Text style=TextStyle::CAPTION>
                        "This page changes to the terminal as soon as the machine connects."
                    </Text>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
pub(crate) fn SetupSteps(editor: Editor) -> NodeId {
    let command = setup_command(editor.block_id());
    let copied = command.clone();
    view! {
        <List spacing=Space::Xl>
            <Text style=TextStyle::PARAGRAPH>
                "A terminal block's shell runs on a machine you choose, which stays connected through the server. Nothing is running it now. To start one:"
            </Text>
            <List spacing=Space::Md>
                <Text>
                    "1. Make an agent: open the app menu, choose Agents, and add one. Keep the credentials it shows."
                </Text>
                <Text>"2. Share this terminal with the agent."</Text>
                <Text>
                    "3. On the machine that should run the shell, run this with the agent's credentials in place of the placeholder:"
                </Text>
            </List>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                    <SelectableText
                        @sizing=ItemSize::Percent(100.0)
                        child_size=ItemSize::Percent(100.0)
                    >
                        <Text family=Family::Mono @test_id={"terminal.command"}>{command}</Text>
                    </SelectableText>
                    <Button
                        icon_only=true
                        label="Copy command"
                        glyph={ICON_COPY.to_owned()}
                        variant=ButtonVariant::Ghost
                        @test_id={"terminal.copy-command"}
                        on_click={move || copy_text(copied.clone())}
                    />
                </List>
            </Card>
            <Text style=TextStyle::CAPTION>
                "The shell runs as that machine's user, with everything it can reach. Anyone who can edit this block can type into it."
            </Text>
        </List>
    }
}
