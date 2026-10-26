use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;

const SLACK: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Stage {
    Probe(usize),
    Show(usize),
}

impl Stage {
    fn settle(self, count: usize) -> Stage {
        match self {
            Stage::Probe(at) if at + 1 >= count => Stage::Show(at.min(count.saturating_sub(1))),
            Stage::Show(at) => Stage::Show(at.min(count.saturating_sub(1))),
            probe => probe,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Measured {
    width: Option<f32>,
    natural: Option<f32>,
}

#[component]
pub fn FirstFit(children: Children<NodeId>) -> NodeId {
    let items = children.into_run();
    let nodes = create_memo(move || items.items());
    let stage = create_signal(Stage::Probe(0));
    let measured = create_signal(Measured::default());
    let settled = create_signal(None::<f32>);
    let staged = create_memo(move || {
        let nodes = nodes.get();
        let stage = stage.get().settle(nodes.len());
        let at = match stage {
            Stage::Probe(at) | Stage::Show(at) => at,
        };
        nodes.get(at).map(|node| (stage, *node))
    });
    let keys = create_memo(move || staged.get().into_iter().collect::<Vec<_>>());
    create_effect(move || {
        let _ = nodes.get();
        settled.set(None);
        stage.set(Stage::Probe(0));
    });
    create_effect(move || {
        let Measured { width, natural } = measured.get();
        let count = nodes.get_untracked().len();
        let Some(width) = width else {
            return;
        };
        match stage.get_untracked().settle(count) {
            Stage::Probe(at) => {
                let Some(natural) = natural else {
                    return;
                };
                let next = match natural <= width + SLACK {
                    true => Stage::Show(at),
                    false => Stage::Probe(at + 1).settle(count),
                };
                settled.set(None);
                stage.set(next);
            }
            Stage::Show(at) => match settled.get_untracked() {
                None => settled.set(Some(width)),
                Some(held) => {
                    let narrower = width < held - SLACK && at + 1 < count;
                    let wider = width > held + SLACK && at > 0;
                    if narrower || wider {
                        settled.set(None);
                        stage.set(Stage::Probe(0).settle(count));
                    } else {
                        settled.set(Some(width));
                    }
                }
            },
        }
    });
    view! {
        <Layers>
            <ForEach keys={keys}>|(stage, node): (Stage, NodeId)| {
                measured.set(Measured::default());
                let shown = view! {
                    <Portal node={Some(node)} />
                };
                let held = measure(shown);
                match stage {
                    Stage::Show(_) => {
                        create_effect(move || {
                            measured.set(Measured {
                                width: held.get().map(|rect| rect.width()),
                                natural: None,
                            });
                        });
                        shown
                    }
                    Stage::Probe(_) => {
                        let probe = view! {
                            <Offset direction=Direction::Horizontal fit=true>{shown}</Offset>
                        };
                        let room = measure(probe);
                        create_effect(move || {
                            measured.set(Measured {
                                width: room.get().map(|rect| rect.width()),
                                natural: held.get().map(|rect| rect.width()),
                            });
                        });
                        probe
                    }
                }
            }</ForEach>
        </Layers>
    }
}
