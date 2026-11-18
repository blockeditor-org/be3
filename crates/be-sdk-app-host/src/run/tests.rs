use be_lib_geometry::{Rect, Vec2, pos2, vec2};
use be_sdk_api::{
    AppIdentity, EditorInstanceId, FontFace, Hello, ScreenId, ScreenLayout, ScreenPlacement,
    SurfaceSupport, ViewId,
    accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate},
};

use super::*;
use crate::accessibility::Placing;

mod a_restart_forgets_what_the_old_run_held_and_tells_the_new_one_again;
mod a_restarted_app_s_views_describe_nothing_and_offer_no_actions;

fn running() -> Run<()> {
    let mut run = Run::new(Session::new("Test", None), true, Preferences::default());
    connect(&mut run);
    run
}

/// Starts the run's session and connects the app.
fn connect(run: &mut Run<()>) {
    run.session.start(0);
    run.session.connect(
        &Hello {
            app: AppIdentity {
                id: "demo".into(),
                name: "Demo".into(),
                version: "1".into(),
            },
            surface: SurfaceSupport::None,
        },
        run.preferences(),
    );
}

fn telling(describing: bool) -> Telling<impl FnOnce() -> Fonts> {
    Telling {
        fonts: || Fonts {
            replace: false,
            faces: Vec::<FontFace>::new(),
        },
        describing,
        preferences: Preferences::default(),
        utc_offset: 60,
    }
}

fn layout(screens: &[u64]) -> ScreenLayout {
    ScreenLayout {
        screens: screens
            .iter()
            .map(|screen| ScreenPlacement {
                screen: ScreenId(*screen),
                instance: EditorInstanceId(1),
                view: ViewId(*screen),
                surface: *screen as u32,
                width: 10,
                height: 10,
                scale_factor_millis: 1000,
                raster_millis: 1000,
            })
            .collect(),
    }
}

/// A host's views: one screen's graft and the actions its view described.
struct Views {
    graft: Graft,
    actions: Vec<String>,
}

impl Descriptions for Views {
    fn grafts(&mut self, each: &mut dyn FnMut(&mut Graft)) {
        each(&mut self.graft);
    }

    fn forget_actions(&mut self) {
        self.actions.clear();
    }
}

fn described() -> Views {
    let mut graft = Graft::new(4);
    let mut button = Node::new(Role::Button);
    button.set_label("Save");
    let mut window = Node::new(Role::Window);
    window.set_children(vec![NodeId(2)]);
    graft.update(TreeUpdate {
        nodes: vec![(NodeId(1), window), (NodeId(2), button)],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(2),
    });
    Views {
        graft,
        actions: vec!["save".into()],
    }
}

/// How many nodes the graft sends under its pane, the pane among them.
fn grafted_nodes(graft: &mut Graft) -> usize {
    let placing = Placing {
        title: Some("Demo"),
        area: Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0)),
        origin: Vec2::ZERO,
        scale: 1.0,
    };
    graft.grafted(placing, true, false).nodes.len()
}
