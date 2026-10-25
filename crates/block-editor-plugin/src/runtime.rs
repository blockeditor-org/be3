use crate::{
    Waker,
    screens::Screens,
    session::{ClientSession, State},
    wasm::Surfaces,
    wire::{Delivery, Shared},
};

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

    pub(crate) fn hello(&self) -> block_plugin_api::Hello {
        self.session.hello()
    }

    /// Takes what the host asked for and answers it through the world's
    /// imports, inside the executor. True once the plugin was shut down.
    pub(crate) fn step(
        &mut self,
        batch: Vec<Delivery>,
        woken: bool,
        wire: &Shared,
    ) -> Result<bool, String> {
        let mut changed = woken;
        let mut draw = false;
        let mut paintings = Vec::new();
        for delivery in batch {
            self.session.receive(&delivery)?;
            match delivery {
                Delivery::Paintings(painted) => paintings.push(painted),
                Delivery::Shutdown => {
                    wire.shut_down();
                    return Ok(true);
                }
                delivery => {
                    draw |= matches!(delivery, Delivery::Draw { .. });
                    changed |= self.screens.deliver(delivery);
                }
            }
            if self.session.state() == State::Closed {
                return Ok(true);
            }
        }
        self.told |= draw;
        let replaced = self.place_surfaces()?;
        if let Some(surfaces) = &mut self.surfaces {
            if draw || (replaced && self.told && wire.drawing()) {
                self.asked = false;
                if let Some(commit) = surfaces.render(&mut self.screens)? {
                    wire.drawn(commit);
                }
            } else if (changed || replaced) && !self.asked {
                self.asked = true;
                block_plugin_api::guest::host::frame_needed();
            }
        }
        for painted in paintings {
            painted.send(self.screens.paintings());
        }
        self.screens.outbound();
        Ok(false)
    }

    fn place_surfaces(&mut self) -> Result<bool, String> {
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
        block_plugin_api::guest::host::layout(layout);
        Ok(true)
    }
}
