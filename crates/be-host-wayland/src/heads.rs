use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::utils::{Logical, Rectangle, Transform};
use smithay::wayland::compositor::send_surface_state;

use crate::state::{State, WindowId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadSpec {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) rect: Rectangle<i32, Logical>,
    pub(crate) scale: i32,
}

pub(crate) struct Head {
    pub(crate) monitor: HeadSpec,
    pub(crate) output: Output,
    global: GlobalId,
}

impl Head {
    fn new(state: &State, monitor: HeadSpec) -> Self {
        let output = Output::new(
            monitor.id.clone(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "BE3".to_owned(),
                model: monitor.name.clone(),
            },
        );
        let global = output.create_global::<State>(&state.handle);
        let head = Self {
            monitor,
            output,
            global,
        };
        head.show();
        head
    }

    fn show(&self) {
        let HeadSpec { rect, scale, .. } = self.monitor;
        let mode = Mode {
            size: (rect.size.w.max(1) * scale, rect.size.h.max(1) * scale).into(),
            refresh: 60_000,
        };
        self.output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Integer(scale)),
            Some(rect.loc),
        );
        self.output.set_preferred(mode);
    }
}

fn overlap(rect: Rectangle<i32, Logical>, window: Rectangle<i32, Logical>) -> i32 {
    rect.intersection(window)
        .map_or(0, |shared| shared.size.w * shared.size.h)
}

impl State {
    pub(crate) fn set_heads(&mut self, monitors: Vec<HeadSpec>) {
        let gone: Vec<usize> = (0..self.heads.len())
            .rev()
            .filter(|index| {
                !monitors
                    .iter()
                    .any(|monitor| monitor.id == self.heads[*index].monitor.id)
            })
            .collect();
        for index in gone {
            let head = self.heads.remove(index);
            self.handle.remove_global::<State>(head.global);
        }
        for (index, monitor) in monitors.into_iter().enumerate() {
            match self
                .heads
                .iter()
                .position(|head| head.monitor.id == monitor.id)
            {
                Some(at) => {
                    if self.heads[at].monitor != monitor {
                        self.heads[at].monitor = monitor;
                        self.heads[at].show();
                    }
                    let head = self.heads.remove(at);
                    self.heads.insert(index.min(self.heads.len()), head);
                }
                None => {
                    let head = Head::new(self, monitor);
                    self.heads.insert(index.min(self.heads.len()), head);
                }
            }
        }
        self.follow_heads();
    }

    pub(crate) fn screen_of(&self, window: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        self.head_of(window)
            .map_or_else(Rectangle::default, |head| head.monitor.rect)
    }

    pub(crate) fn first_screen(&self) -> Rectangle<i32, Logical> {
        self.heads
            .first()
            .map_or_else(Rectangle::default, |head| head.monitor.rect)
    }

    pub(crate) fn head_of(&self, window: Rectangle<i32, Logical>) -> Option<&Head> {
        self.heads
            .iter()
            .filter(|head| overlap(head.monitor.rect, window) > 0)
            .max_by_key(|head| overlap(head.monitor.rect, window))
            .or_else(|| self.heads.first())
    }

    pub(crate) fn head_of_window(&self, id: WindowId) -> Option<&Head> {
        let window = self.find(id)?;
        self.head_of(Rectangle::new(
            window.position,
            window.window.geometry().size,
        ))
    }

    pub(crate) fn output_of(&self, id: WindowId) -> Option<Output> {
        self.head_of_window(id).map(|head| head.output.clone())
    }

    pub(crate) fn scale_of(&self, id: WindowId) -> i32 {
        self.head_of_window(id).map_or(1, |head| head.monitor.scale)
    }

    pub(crate) fn head_named(&self, output: &WlOutput) -> Option<Rectangle<i32, Logical>> {
        self.heads
            .iter()
            .find(|head| head.output.owns(output))
            .map(|head| head.monitor.rect)
    }

    pub(crate) fn follow_heads(&mut self) {
        let moved: Vec<(WindowId, Option<String>)> = self
            .windows
            .iter()
            .map(|window| {
                let id = self
                    .head_of(Rectangle::new(
                        window.position,
                        window.window.geometry().size,
                    ))
                    .map(|head| head.monitor.id.clone());
                (window.id, id)
            })
            .filter(|(id, head)| {
                self.find(*id)
                    .is_some_and(|window| window.entered.as_ref() != head.as_ref())
            })
            .collect();
        for (id, head) in moved {
            self.enter(id, head);
        }
    }

    fn enter(&mut self, id: WindowId, head: Option<String>) {
        let outputs: Vec<(String, Output, i32)> = self
            .heads
            .iter()
            .map(|head| {
                (
                    head.monitor.id.clone(),
                    head.output.clone(),
                    head.monitor.scale,
                )
            })
            .collect();
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return;
        };
        let old = std::mem::replace(&mut window.entered, head.clone());
        let left = outputs
            .iter()
            .find(|(held, _, _)| Some(held) == old.as_ref())
            .map(|(_, output, _)| output.clone());
        let entered = outputs
            .iter()
            .find(|(held, _, _)| Some(held) == head.as_ref())
            .map(|(_, output, scale)| (output.clone(), *scale));
        window.window.with_surfaces(|surface, data| {
            if let Some(left) = &left {
                left.leave(surface);
            }
            if let Some((output, scale)) = &entered {
                output.enter(surface);
                send_surface_state(surface, data, *scale, Transform::Normal);
            }
        });
    }
}
