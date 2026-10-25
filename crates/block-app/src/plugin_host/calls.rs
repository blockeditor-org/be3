use block_plugin_api::{
    BarAction, ChildId, EditorInstanceId, EditorMessage, EditorRegion, Message,
    embed::shell::Call as ShellCall,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outbound {
    Message(Message),
    Editor(EditorInstanceId, EditorCall),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditorCall {
    MenuPick(String),
    ChildBar {
        region: EditorRegion,
        child: ChildId,
        action: BarAction,
    },
}

impl From<Message> for Outbound {
    fn from(message: Message) -> Self {
        Self::Message(message)
    }
}

impl From<EditorMessage> for Outbound {
    fn from(message: EditorMessage) -> Self {
        Self::Message(Message::Editor(message))
    }
}

impl Outbound {
    #[cfg(test)]
    pub(super) fn messages(outbound: &[Self]) -> Vec<Message> {
        outbound
            .iter()
            .filter_map(|outbound| match outbound {
                Self::Message(message) => Some(message.clone()),
                Self::Editor(..) => None,
            })
            .collect()
    }

    pub(super) fn instance(&self) -> Option<EditorInstanceId> {
        match self {
            Self::Message(Message::Editor(message)) => Some(message.instance()),
            Self::Message(_) => None,
            Self::Editor(instance, _) => Some(*instance),
        }
    }

    pub(super) fn closes(&self) -> bool {
        matches!(
            self,
            Self::Message(Message::Editor(EditorMessage::Close { .. }))
        )
    }
}

#[derive(Debug)]
pub(super) enum Heard {
    Message(Message),
    Shell(ShellCall),
}

impl From<Message> for Heard {
    fn from(message: Message) -> Self {
        Self::Message(message)
    }
}

pub(super) fn instance_of(call: &ShellCall) -> EditorInstanceId {
    match call {
        ShellCall::Menu { instance, .. }
        | ShellCall::RunBarAction { instance, .. }
        | ShellCall::ChildMenuPick { instance, .. } => *instance,
    }
}
