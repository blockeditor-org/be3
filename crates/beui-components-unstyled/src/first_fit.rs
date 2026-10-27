use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Direction, Extent, ItemSize};

const SLACK: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Stage {
    Probe(usize),
    Show(usize),
}

impl Stage {
    fn settle(self, count: usize) -> Stage {
        let last = count.saturating_sub(1);
        match self {
            Stage::Probe(at) if at >= last => Stage::Show(last),
            Stage::Show(at) => Stage::Show(at.min(last)),
            probe => probe,
        }
    }

    fn next(
        self,
        count: usize,
        (room, stretched): (f32, Option<f32>),
        width: f32,
        naturals: &[Option<f32>],
    ) -> Stage {
        let fits = |width: f32| width <= room + SLACK;
        let stale = stretched
            .is_some_and(|stretched| stretched > room + SLACK && (width - stretched).abs() <= SLACK);
        let last = count.saturating_sub(1);
        let earlier = |before: usize| {
            naturals
                .iter()
                .take(before)
                .position(|natural| natural.is_none_or(fits))
                .map(Stage::Probe)
        };
        let stage = self.settle(count);
        match stage {
            Stage::Probe(at) if fits(width) => Stage::Show(at),
            Stage::Probe(at) => Stage::Probe(at + 1).settle(count),
            Stage::Show(at) if at < last && !fits(width) => {
                let known = naturals.get(at).copied().flatten();
                match stale && known.is_some_and(fits) {
                    true => stage,
                    false => Stage::Probe(at + 1).settle(count),
                }
            }
            Stage::Show(at) => earlier(at).unwrap_or(stage),
        }
    }
}

#[component]
pub fn FirstFit(children: Children<NodeId>) -> NodeId {
    let items = children.into_run();
    let nodes = create_memo(move || items.items());
    let count = create_memo(move || nodes.get().len());
    let stage = create_signal(Stage::Probe(0));
    let naturals = create_signal(Vec::<Option<f32>>::new());
    let width = create_signal(None::<f32>);
    create_effect(move || {
        let count = nodes.get().len();
        naturals.set(vec![None; count]);
        stage.set(Stage::Probe(0).settle(count));
    });
    let staged = create_memo(move || {
        let stage = stage.get().settle(count.get());
        let at = match stage {
            Stage::Probe(at) | Stage::Show(at) => at,
        };
        nodes.get().get(at).map(|node| (stage, *node))
    });
    let keys = create_memo(move || staged.get().into_iter().collect::<Vec<_>>());
    let claim = create_memo(move || {
        let first = naturals.with(|naturals| naturals.first().copied().flatten());
        Extent::at_most(first.unwrap_or(0.0))
    });
    let sizer = view! {
        <Frame width={claim} height={Extent::px(0.0)} />
    };
    let held = measure(sizer);
    let room = create_memo(move || held.get().map(|held| held.size.x));
    let stretched = std::cell::Cell::new(None::<f32>);
    create_effect(move || {
        let (Some(room), Some(width)) = (room.get(), width.get()) else {
            return;
        };
        let stretched = stretched.replace(Some(room));
        let count = count.get_untracked();
        let current = stage.get_untracked().settle(count);
        if let Stage::Probe(at) = current {
            naturals.update(|naturals| {
                if let Some(natural) = naturals.get_mut(at) {
                    *natural = Some(width);
                }
            });
        }
        let next = naturals.with_untracked(|naturals| current.next(count, (room, stretched), width, naturals));
        if next != current {
            stage.set(next);
        }
    });
    view! {
        <Layers>
            {sizer}
            <ForEach keys={keys}>|(stage, node): (Stage, NodeId)| {
                let shown = view! {
                    <Portal node={Some(node)} />
                };
                if stage == Stage::Show(count.get_untracked().saturating_sub(1)) {
                    width.set(Some(0.0));
                    return shown;
                }
                width.set(None);
                let natural = naturals.with_untracked(|naturals| match stage {
                    Stage::Show(at) => naturals.get(at).copied().flatten(),
                    Stage::Probe(_) => None,
                });
                let stretch = create_memo(move || match stage {
                    Stage::Show(_) => ItemSize::Intrinsic.min(room.get().unwrap_or(0.0)),
                    Stage::Probe(_) => ItemSize::Intrinsic.min(0.0),
                });
                let measured = measure(shown);
                create_effect(move || {
                    if let Some(measured) = measured.get() {
                        width.set(Some(measured.size.x));
                    }
                });
                let box_width = match natural {
                    Some(natural) => Extent::fraction(1.0).max(natural),
                    None => Extent::fraction(1.0),
                };
                view! {
                    <Frame width={box_width}>
                        <List direction=Direction::Horizontal spacing=0.0>
                            {shown} @sizing={stretch}
                        </List>
                    </Frame>
                }
            }</ForEach>
        </Layers>
    }
}
