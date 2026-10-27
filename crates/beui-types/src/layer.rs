use geometry::{Rect, Vec2, pos2};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LayerPolicy {
    Inert,
    Pass,
    BlocksParent,
    #[default]
    BlocksAll,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LayerLevel {
    #[default]
    Normal,
    Top,
    Lock,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dismiss {
    Escape,
    Back,
    OutsidePress,
    WithParent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    At,
    Above,
    Over(u16),
    Around,
    BelowStart,
    RightStart,
    Center,
    Fill,
    InsideTop,
    InsideTopEnd,
    InsideBottom,
    InsideBottomEnd,
    CenterOn,
    HangFrom(u16),
    Popover { gap: u16, margin: u16 },
}

pub fn resolve_rect(
    viewport: Rect,
    anchor_rect: Rect,
    placement: Placement,
    content_size: Vec2,
) -> Rect {
    if placement == Placement::At {
        return Rect::from_min_size(anchor_rect.min, content_size);
    }
    if let Placement::Over(inset) = placement {
        let inset = f32::from(inset);
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let origin = pos2(
            (anchor_rect.left() - inset).min(right).max(viewport.left()),
            (anchor_rect.top() - inset).min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::Around {
        return Rect::from_center_size(anchor_rect.center(), content_size);
    }
    if placement == Placement::Above {
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let y = match anchor_rect.top() - content_size.y >= viewport.top() {
            true => anchor_rect.top() - content_size.y,
            false => anchor_rect.bottom(),
        };
        let origin = pos2(
            (anchor_rect.center().x - content_size.x / 2.0)
                .min(right)
                .max(viewport.left()),
            y.min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::CenterOn {
        let size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let centred = Rect::from_center_size(anchor_rect.center(), size);
        let x = centred
            .left()
            .min(viewport.right() - size.x)
            .max(viewport.left());
        let y = centred
            .top()
            .min(viewport.bottom() - size.y)
            .max(viewport.top());
        return Rect::from_min_size(pos2(x, y), size);
    }
    if let Placement::HangFrom(gap) = placement {
        let gap = f32::from(gap);
        let room = viewport.shrink(gap);
        let size = content_size.min(room.size()).max(Vec2::ZERO);
        let x = (anchor_rect.center().x - size.x / 2.0)
            .min(room.right() - size.x)
            .max(room.left());
        let y = (anchor_rect.top() + gap)
            .min(room.bottom() - size.y)
            .max(room.top());
        return Rect::from_min_size(pos2(x, y), size);
    }
    if let Placement::Popover { gap, margin } = placement {
        let gap = f32::from(gap);
        let centre = anchor_rect.center().x;
        let reach = Rect::from_min_max(
            pos2(centre - content_size.x / 2.0, anchor_rect.top() - gap),
            pos2(centre + content_size.x / 2.0, anchor_rect.bottom() + gap),
        );
        let room = viewport.shrink(f32::from(margin));
        return resolve_rect(room, reach, Placement::BelowStart, content_size);
    }
    if placement == Placement::Fill {
        return viewport;
    }
    if placement == Placement::Center {
        let content_size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let origin = pos2(
            viewport.left() + (viewport.width() - content_size.x) / 2.0,
            viewport.top() + (viewport.height() - content_size.y) / 2.0,
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTopEnd | Placement::InsideBottomEnd = placement {
        let y = match placement {
            Placement::InsideTopEnd => anchor_rect.top(),
            _ => (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
        };
        let origin = pos2(
            (anchor_rect.right() - content_size.x).max(anchor_rect.left()),
            y,
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTop | Placement::InsideBottom = placement {
        let y = match placement {
            Placement::InsideTop => anchor_rect.top(),
            _ => anchor_rect.bottom() - content_size.y,
        };
        let origin = pos2(
            anchor_rect.center().x - content_size.x / 2.0,
            y.clamp(
                anchor_rect.top(),
                (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
            ),
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    let mut content_size = content_size.min(viewport.size());
    if placement == Placement::BelowStart {
        let below = viewport.bottom() - anchor_rect.bottom();
        let above = anchor_rect.top() - viewport.top();
        if content_size.y > below && content_size.y > above && below.max(above) > 0.0 {
            content_size.y = below.max(above);
            let top = match below >= above {
                true => anchor_rect.bottom(),
                false => anchor_rect.top() - content_size.y,
            };
            let left = anchor_rect
                .left()
                .min(viewport.right() - content_size.x)
                .max(viewport.left());
            return Rect::from_min_size(pos2(left, top), content_size);
        }
    }
    let mut origin = match placement {
        Placement::BelowStart => pos2(anchor_rect.left(), anchor_rect.bottom()),
        Placement::RightStart => pos2(anchor_rect.right(), anchor_rect.top()),
        Placement::At
        | Placement::Above
        | Placement::Over(_)
        | Placement::Around
        | Placement::Center
        | Placement::Fill
        | Placement::InsideTop
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomEnd
        | Placement::CenterOn
        | Placement::HangFrom(_)
        | Placement::Popover { .. } => {
            unreachable!("a centred or pinned overlay is placed above")
        }
    };
    if origin.x + content_size.x > viewport.right() {
        origin.x = match placement {
            Placement::RightStart => anchor_rect.left() - content_size.x,
            _ => (viewport.right() - content_size.x).max(viewport.left()),
        };
    }
    if origin.y + content_size.y > viewport.bottom() {
        origin.y = match placement {
            Placement::BelowStart => anchor_rect.top() - content_size.y,
            _ => (viewport.bottom() - content_size.y).max(viewport.top()),
        };
    }
    origin.x = origin.x.max(viewport.left());
    origin.y = origin.y.max(viewport.top());
    Rect::from_min_size(origin, content_size)
}
