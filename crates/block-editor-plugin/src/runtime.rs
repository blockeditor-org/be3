use block_plugin_api::Message;

use crate::{
    Waker,
    screens::Screens,
    session::{ClientSession, State},
    wasm::Surfaces,
    wire::Delivery,
};

pub(crate) struct Step {
    pub(crate) outbound: Vec<Message>,
    pub(crate) closed: bool,
}

pub(crate) struct Runtime {
    session: ClientSession,
    screens: Screens,
    surfaces: Option<Surfaces>,
    asked: bool,
    told: bool,
}

impl Runtime {
    pub(crate) fn new(
        apps: Vec<(uuid::Uuid, crate::screens::Opener)>,
        id: &str,
        name: &str,
        version: &str,
        waker: Waker,
    ) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new(apps, waker),
            surfaces: None,
            asked: false,
            told: false,
        }
    }

    pub(crate) fn hello(&self) -> Message {
        self.session.hello()
    }

    pub(crate) fn step(&mut self, batch: Vec<Delivery>, woken: bool) -> Result<Step, String> {
        let mut changed = woken;
        let mut draw = false;
        let mut painting = false;
        let mut outbound = Vec::new();
        for delivery in batch {
            let message = match delivery {
                Delivery::Message(message) => message,
                Delivery::Editor(instance, call) => {
                    changed |= self.screens.call(instance, call);
                    continue;
                }
                Delivery::Screens(call) => {
                    call(&mut self.screens);
                    changed = true;
                    continue;
                }
            };
            if matches!(message, Message::DrawFrame { .. }) {
                draw = true;
            }
            painting |= matches!(
                &message,
                Message::Extension { key, .. } if *key == paint_format::PAINTING_EXTENSION
            );
            changed |= self.screens.receive(&message);
            outbound.extend(self.session.receive(message));
            if matches!(self.session.state(), State::Closed) {
                return Ok(Step {
                    outbound,
                    closed: true,
                });
            }
        }
        self.told |= draw;
        let replaced = self.place_surfaces(&mut outbound)?;
        if let Some(surfaces) = &mut self.surfaces {
            if draw || (replaced && self.told) {
                self.asked = false;
                outbound.extend(surfaces.render(&mut self.screens)?);
            } else if (changed || replaced) && !self.asked {
                self.asked = true;
                outbound.push(Message::FrameNeeded);
            }
        }
        if painting {
            outbound.push(self.screens.paintings());
        }
        outbound.extend(self.screens.outbound());
        Ok(Step {
            outbound,
            closed: false,
        })
    }

    fn place_surfaces(&mut self, outbound: &mut Vec<Message>) -> Result<bool, String> {
        let Some(spec) = self.screens.surface() else {
            return Ok(false);
        };
        if self.surfaces.is_none() {
            self.surfaces = Some(Surfaces::new(spec)?);
        }
        let surfaces = self.surfaces.as_mut().expect("the surfaces were just made");
        let layout = self.screens.layout().clone();
        if surfaces.layout().same_placements(&layout) {
            return Ok(false);
        }
        surfaces.place(layout.clone());
        outbound.push(Message::Layout(layout));
        Ok(true)
    }
}
