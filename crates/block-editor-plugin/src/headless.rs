use block_plugin_api::{EditorInstanceId, EditorRegion, Message, ScreenLayout, ScreenPlacement};

use crate::{
    Artifacts, BeuiApp, Creation, Editor, EditorHost, Waker,
    editor_session::EditorSession,
    screens::Screens,
    session::{ClientSession, State},
};

pub type View = Box<dyn FnOnce() -> beui::NodeId>;

pub enum Adopted {
    Editor(Editor, Option<View>),
    Preview(Editor),
    Creation(Creation),
    Artifacts(Artifacts),
}

impl Adopted {
    pub(crate) fn host(&self) -> EditorHost {
        match self {
            Self::Editor(editor, _) | Self::Preview(editor) => editor.host().clone(),
            Self::Creation(creation) => creation.host().clone(),
            Self::Artifacts(artifacts) => artifacts.host().clone(),
        }
    }
}

pub struct HeadlessPlugin {
    session: ClientSession,
    screens: Screens,
    generation: u64,
}

impl HeadlessPlugin {
    pub fn new<A: BeuiApp>(id: &str, name: &str, version: &str) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new::<A>(Waker::default()),
            generation: 0,
        }
    }

    pub fn hello(&self) -> Message {
        self.session.hello()
    }

    pub fn adopt<A: BeuiApp>(&mut self, instance: EditorInstanceId, adopted: Adopted) {
        self.screens
            .adopt(instance, EditorSession::adopt::<A>(instance, adopted));
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        if matches!(self.session.state(), State::Closed | State::Failed) {
            return Vec::new();
        }
        let before = self.screens.layout().clone();
        self.screens.receive(&message);
        let replies = self.session.receive(message);
        let layout = self.screens.layout();
        if !layout.is_empty() && !layout.same_placements(&before) {
            self.generation += 1;
            self.screens.set_generation(self.generation);
        }
        replies
    }

    pub fn draw(&mut self) -> Vec<(ScreenPlacement, beui::FrameOutput)> {
        let layout = self.screens.layout().clone();
        let mut outputs = Vec::new();
        for placement in &layout.screens {
            let Some(session) = self.screens.session(placement.instance) else {
                continue;
            };
            outputs.push((*placement, session.run(placement.region, layout.generation)));
        }
        outputs
    }

    pub fn outbound(&mut self) -> Vec<Message> {
        self.screens.outbound()
    }

    pub fn layout(&self) -> &ScreenLayout {
        self.screens.layout()
    }

    pub fn document(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<&beui::Document> {
        self.screens.get(instance)?.document(region)
    }

    pub fn host(&self, instance: EditorInstanceId) -> Option<EditorHost> {
        Some(self.screens.get(instance)?.host().clone())
    }

    pub fn editor(&self, instance: EditorInstanceId) -> Option<Editor> {
        self.screens.get(instance)?.editor()
    }

    pub fn in_frame<T>(
        &mut self,
        instance: EditorInstanceId,
        run: impl FnOnce() -> T,
    ) -> Option<T> {
        let chrome = self.screens.session(instance)?.chrome_mut()?;
        Some(beui::reactive::with_reactive_scope(
            chrome.document_mut(),
            run,
        ))
    }
}
