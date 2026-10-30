use geometry::{Pos2, Rect, Region, vec2};

use super::*;
use crate::{EditorInstanceId, FrameSpec, View};

mod a_framework_that_stops_stops_every_shown_region;
mod a_region_with_damage_has_changed_and_keeps_its_surface_while_shown;
mod a_window_ask_goes_to_the_region_of_the_instance_that_made_it;

#[derive(Default)]
struct Plain {
    stopped: Option<Option<Stopped>>,
    asks: Vec<(ViewRef, WindowRequest)>,
    damage: Option<Damage>,
}

impl Regions for Plain {
    type Painter = ();

    fn input(
        &mut self,
        _services: &mut HostServices,
        _region: &ShownView,
        _event: &TimedInput,
        _over: Option<ViewRef>,
    ) {
    }

    fn frame(
        &mut self,
        _services: &mut HostServices,
        _active: Option<ViewRef>,
        _regions: &[ShownView],
    ) -> Result<Ran, Option<Stopped>> {
        if let Some(stopped) = self.stopped.clone() {
            return Err(stopped);
        }
        Ok(Ran {
            asks: std::mem::take(&mut self.asks),
            ..Ran::default()
        })
    }

    fn framed(
        &mut self,
        surfaces: &mut Surfaces<()>,
        region: &ShownView,
        _keyboard: Option<ViewRef>,
    ) -> Frame {
        if let Some(damage) = self.damage.take() {
            surfaces.of(region.id()).damage(damage);
        }
        Frame {
            painted: vec![region.rect],
            ..Frame::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(
        _painter: &mut (),
        _gpu: &SurfaceGpu,
        target: &PaintTarget<'_>,
        _size: (u32, u32),
        _repaint: Damage,
    ) -> Option<Vec<SurfaceRect>> {
        Some(target.whole())
    }
}

fn shown(instance: u64) -> ShownView {
    ShownView {
        instance: EditorInstanceId(instance),
        view: View::MAIN,
        block: None,
        rect: Rect::from_min_size(Pos2::ZERO, vec2(40.0, 30.0)),
        visible: vec2(0.0, 0.0),
        scale_factor: 1.0,
        raster: 1.0,
        pixels: [40, 30],
        age: 0,
        spec: FrameSpec::default(),
        describe: None,
        seen: true,
        windows: true,
        docked: false,
        host_place: None,
        anchor_space: None,
    }
}

fn driven(plain: Plain) -> Driver<Plain> {
    Driver::new(EditorHost::default(), plain)
}

fn damaged() -> Damage {
    Damage::Region(Region::from(Rect::from_min_size(
        Pos2::ZERO,
        vec2(4.0, 4.0),
    )))
}
