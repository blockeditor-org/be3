//! What a root's frame shows natively: each surface its layer list names drawn on the window's
//! device (`Surfaces`), the root started on that device (`start`).

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
};

use be_host_desktop::desktop_host::{DesktopHost, Host, PictureSurface, WindowPicture};
use be_lib_layers::{Layer, LayerList, Surface};
use be_sdk_api::HostWindowId;
use be_wasm_root_host::Root;
use beui::native::painting::{Painting, Paints, Pixels, Placement, ScreenPaintings};
use beui::native::texture_quad::{Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt, Drawing as GpuDrawing, compose, draw_gpu};
use beui::reactive::*;
use beui::styled::*;

use super::SHOWN;
use super::serving::fonts;

/// What the window shows of the root's frames: the surfaces of the last, drawn into the document
/// as one drawing.
pub(super) struct Surfaces {
    /// What the window shows of the root's last frame.
    shown: Signal<Option<GpuDrawing>>,
    /// What the last frame showed, so an unchanged frame keeps its drawing.
    drawn: Vec<(Layer, Shown)>,
    /// What the root's own surfaces painted when last asked (drive's `paint`), by their ids.
    paintings: Rc<RefCell<BTreeMap<u64, Painting>>>,
}

impl Surfaces {
    /// The document the window shows, which draws what the root drew, with the surfaces it
    /// shows them by. Under `--desktop` the platform's desktop host is made while the document
    /// is.
    pub(super) fn document(desktop: bool) -> (beui::native::Document, Self) {
        let mut shown = None;
        let document = beui_host::native::build(|| {
            // Under `--desktop` the platform's desktop host runs here as under be-host's own UI
            // (the Wayland compositor on Linux), so programs run and their windows reach the
            // desktop app (plans/host_as_platform_adapter.md, 4g7e3a).
            if desktop {
                Host::create();
                // Its windows reach the desktop app the root shows.
                be_host_desktop::desktop_host::publish_windows();
            }
            let drawing = create_signal(None::<GpuDrawing>);
            shown = Some(drawing);
            view! {
                <beui::styled::Root title="be-host">
                    <Drawing @test_id={SHOWN} draw={move || draw_gpu(drawing.get())} />
                </beui::styled::Root>
            }
        });
        let surfaces = Self {
            shown: shown.expect("the drawing's signal is made while the document is built"),
            drawn: Vec::new(),
            paintings: Rc::default(),
        };
        (document, surfaces)
    }

    /// Keeps what the root's surfaces painted, as drive asked (`take_paintings`), for the
    /// window's paintings.
    pub(super) fn painted(&mut self, app: &str, painted: Vec<u8>) {
        // A painting that cannot be read paints nothing: it is for the tests, not the user.
        let painted = match painted.is_empty() {
            true => ScreenPaintings::default(),
            false => ScreenPaintings::decode(&painted).unwrap_or_else(|why| {
                eprintln!("be-host: the root {app}'s paintings cannot be read: {why}");
                ScreenPaintings::default()
            }),
        };
        *self.paintings.borrow_mut() = painted.screens.into_iter().collect();
    }

    /// Shows the layers of `list` in the host's one window, which the list names as none: each
    /// texture the root or an app it embeds drew, and under `--desktop` each program window it
    /// placed, which this places.
    pub(super) fn show(
        &mut self,
        desktop: bool,
        root: &mut Root,
        list: LayerList,
        document: &mut beui::native::Document,
    ) {
        // What a surface of an app the root embeds showed stays while the list names it, shown
        // or not: the last frame the root keeps of a view hidden while its app draws it anew
        // (be-sdk-root's `Embedded::kept_layers`), as be-host keeps its own views'.
        let named: HashSet<(u64, u32)> = list
            .windows
            .iter()
            .flat_map(|window| &window.layers)
            .filter_map(|layer| match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    Some((instance, surface))
                }
                Surface::Root(_) | Surface::Host(_) => None,
            })
            .collect();
        root.retain(|instance, surface| named.contains(&(instance, surface)));
        let mut layers = Vec::new();
        let mut placed = Vec::new();
        for layer in list
            .windows
            .into_iter()
            .filter(|window| window.window.is_none())
            .flat_map(|window| window.layers)
            .filter(|layer| layer.shown)
        {
            let shown = match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    root.surface(instance, surface)
                        .map(|shown| Shown::Texture(shown.generation, shown.texture.clone()))
                }
                Surface::Root(surface) => root
                    .own_surface(surface)
                    .map(|shown| Shown::Texture(shown.generation, shown.texture.clone())),
                // A program window the root placed, which the desktop host configures to its
                // layer's size (its pixels are the window's logical pixels) and draws once it
                // drew something (`DesktopHost::picture`).
                // Where it is placed is its layer's origin and its size, upright and unscaled:
                // a window placed zoomed or turned is configured to its place's size and hears
                // its pointer in its own pixels (the root's router maps it), but where the
                // compositor thinks it is, which its popups follow, does not follow the zoom or
                // the turn.
                Surface::Host(window) if desktop => {
                    let window = HostWindowId(window);
                    let origin = layer.transform.apply(beui::pos2(0.0, 0.0));
                    let size = beui::vec2(layer.size.0 as f32, layer.size.1 as f32);
                    placed.push((window, beui::Rect::from_min_size(origin, size)));
                    Host::picture(window).map(Shown::Window)
                }
                Surface::Host(_) => None,
            };
            if let Some(shown) = shown {
                layers.push((layer, shown));
            }
        }
        if desktop {
            Host::place_windows(&placed);
        }
        let unchanged = layers.len() == self.drawn.len()
            && layers
                .iter()
                .zip(&self.drawn)
                .all(|((layer, shown), (was, drawn))| layer == was && shown.same(drawn));
        if unchanged {
            return;
        }
        self.drawn = layers.clone();
        let drawing = (!layers.is_empty()).then(|| {
            beui::native::painted_drawing(ShownLayers {
                layers,
                quads: Quads::default(),
                paintings: Rc::clone(&self.paintings),
            })
        });
        let shown = self.shown;
        beui::native::enter(document, || shown.set(drawing));
    }
}

/// Starts `app`'s entry point as the root on the window's device, which it and its staged apps,
/// found as be-host finds every app's, draw on, waking the host whenever it has something to
/// step for.
pub(super) fn start(app: &str, setup: &beui::native::Setup) -> Result<Root, String> {
    let gpu = setup
        .get::<beui_renderer_wgpu::present::GpuSetup>()
        .ok_or("the window has no GPU renderer, and the root's apps draw on its device")?;
    let manifest = super::root_manifest(app)?;
    // Loaded as an app is, linked against the libraries it names, which every app on this
    // device shares one compile of; but stepped inline on the window's thread, never on a
    // thread of its own.
    let host = be_wasm_host::Host::new(gpu.device.clone(), gpu.queue.clone())?;
    let module = be_host_runtime::staged_app(app, &manifest.entry_point)?.load(&host)?;
    let root = Root::start(
        module,
        Box::new(be_host_runtime::staged_app),
        gpu.device.clone(),
        gpu.queue.clone(),
    )?
    .with_fonts(fonts());
    let waker = setup.waker.clone();
    root.on_wake(move || waker.wake());
    Ok(root)
}

/// What a layer of the root's frame shows: a texture the root or an app drew, with its
/// generation, or a program window's picture.
#[derive(Clone)]
enum Shown {
    Texture(u64, wgpu::Texture),
    Window(WindowPicture),
}

impl Shown {
    /// Whether it shows what `other` showed: the same generation of a texture, or the same
    /// picture of a window.
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Texture(generation, _), Self::Texture(was, _)) => generation == was,
            (Self::Window(picture), Self::Window(was)) => picture.same(was),
            _ => false,
        }
    }
}

/// The layer showing `surface`, one of a program window's surfaces, where the window's layer
/// shows the window: its texture's pixels mapped onto its rect in the window's logical pixels,
/// which are the window's layer's pixels, and clipped to what the window's layer shows. None
/// for a surface that shows nothing.
fn surface_layer(window: &Layer, surface: &PictureSurface) -> Option<Layer> {
    let size = surface.texture.size();
    let (width, height) = (size.width as f32, size.height as f32);
    let source = beui::Rect::from_min_max(
        beui::pos2(surface.uv.min.x * width, surface.uv.min.y * height),
        beui::pos2(surface.uv.max.x * width, surface.uv.max.y * height),
    );
    if source.width() <= 0.0 || source.height() <= 0.0 {
        return None;
    }
    let (across, down) = (
        surface.rect.width() / source.width(),
        surface.rect.height() / source.height(),
    );
    let into_window = be_lib_layers::Affine {
        xx: across,
        xy: 0.0,
        yx: 0.0,
        yy: down,
        x: surface.rect.min.x - source.min.x * across,
        y: surface.rect.min.y - source.min.y * down,
    };
    let transform = into_window.then(&window.transform);
    // What the window's layer shows, in the window's logical pixels, in the texture's: exact
    // however the window's layer is turned, as the surface is upright in the window.
    let shows = into_window.inverse()?.bounds(window.clip);
    Some(Layer {
        size: (size.width, size.height),
        transform,
        clip: source.intersect(shows),
        corner_radius: 0.0,
        opacity: window.opacity * surface.alpha.clamp(0.0, 1.0),
        ..window.clone()
    })
}

/// The surfaces of the root's frame the window shows, bottom first, each with what it shows.
struct ShownLayers {
    layers: Vec<(Layer, Shown)>,
    quads: Quads,
    /// What the root's own surfaces painted when drive last asked, by their ids.
    paintings: Rc<RefCell<BTreeMap<u64, Painting>>>,
}

impl Paints for ShownLayers {
    /// Each of the root's surfaces' paintings where its layer shows it in the window, bottom
    /// first: its own by their surfaces' ids, and its apps' screens and anchored views, which their paintings
    /// come through the root for (plans/host_as_platform_adapter.md, Other moves), by their
    /// layers' ids.
    fn painting(&self, _rect: beui::Rect) -> Option<Painting> {
        let paintings = self.paintings.borrow();
        let mut primitives = Vec::new();
        let mut textures = BTreeMap::new();
        // Each layer with what it painted: the root's and its apps' surfaces as they painted
        // when drive asked, and each of a program window's surfaces as its texture holds it.
        let mut painted: Vec<(Layer, std::borrow::Cow<'_, Painting>)> = Vec::new();
        for (layer, shown) in &self.layers {
            let key = match (layer.surface, shown) {
                (_, Shown::Window(picture)) => {
                    for (index, surface) in picture.surfaces.iter().enumerate() {
                        let (Some(placed), Some(pixels)) = (
                            surface_layer(layer, surface),
                            picture.pixels(&surface.texture),
                        ) else {
                            continue;
                        };
                        let key = std::hash::BuildHasher::hash_one(
                            &std::hash::BuildHasherDefault::<std::hash::DefaultHasher>::default(),
                            (layer.id.0, index),
                        );
                        painted.push((
                            placed,
                            std::borrow::Cow::Owned(texture_painting(key, pixels)),
                        ));
                    }
                    continue;
                }
                (Surface::Root(surface), _) => u64::from(surface),
                (Surface::Screen { .. } | Surface::Anchored { .. }, _) => layer.id.0,
                (Surface::Host(_), _) => continue,
            };
            if let Some(painting) = paintings.get(&key) {
                painted.push((layer.clone(), std::borrow::Cow::Borrowed(painting)));
            }
        }
        for (layer, painting) in &painted {
            // The painting's own pixels, which are its surface's: a screen asked to fit its
            // content paints the room it used, at the top left of its surface.
            let (width, height) = (painting.frame.size[0] as f32, painting.frame.size[1] as f32);
            let corners =
                [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)].map(|(x, y)| {
                    let at = layer.transform.apply(beui::pos2(x, y));
                    [at.x, at.y]
                });
            // What of it shows: its layer's clip.
            let shown = layer.transform.bounds(layer.clip);
            let clip = [shown.min.x, shown.min.y, shown.max.x, shown.max.y];
            primitives.extend(painting.placed(Placement {
                source: [0.0, 0.0, 1.0, 1.0],
                corners,
                clip,
                corner_radius: layer.corner_radius,
            }));
            for (key, pixels) in &painting.textures {
                textures.entry(*key).or_insert_with(|| pixels.clone());
            }
        }
        Some(Painting {
            frame: beui::native::painting::Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives,
            },
            textures,
        })
    }
}

/// A painting of `pixels` filling its frame, one pixel a pixel, as the texture `key` names.
fn texture_painting(key: u64, pixels: Pixels) -> Painting {
    use beui::native::painting::{Content, Fade, Primitive, Triangle, Vertex};
    let [width, height] = pixels.size.map(|side| side as f32);
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color: [255; 4],
    };
    let (a, b, c, d) = (
        corner(0.0, 0.0, 0.0, 0.0),
        corner(width, 0.0, 1.0, 0.0),
        corner(width, height, 1.0, 1.0),
        corner(0.0, height, 0.0, 1.0),
    );
    Painting {
        frame: beui::native::painting::Frame {
            size: pixels.size,
            pixels_per_point: 1.0,
            background: [0; 4],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, width, height],
                fade: Fade::NONE,
                rounds: Vec::new(),
                content: Content::Mesh(vec![
                    Triangle {
                        texture: key,
                        corners: [a, b, c],
                    },
                    Triangle {
                        texture: key,
                        corners: [a, c, d],
                    },
                ]),
            }],
        },
        textures: BTreeMap::from([(key, pixels)]),
    }
}

impl Draw for ShownLayers {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipelines = Pipelines::of(device, &at);
        let mut bound = Vec::new();
        for (layer, shown) in &self.layers {
            match shown {
                Shown::Texture(_, texture) => {
                    let quad = Quad {
                        clip: at.clip,
                        // An app presents sRGB-encoded values, as it does to the host's own UI.
                        srgb_encoded: true,
                        ..compose::quad(layer, at.pixels_per_point, [0.0; 2])
                    };
                    bound.push((pipelines.bind(texture), quad));
                }
                // Each of a program window's surfaces, as its view's drawing draws them
                // (be-host-wayland's `WindowDraw`).
                Shown::Window(picture) => {
                    for surface in &picture.surfaces {
                        let Some(placed) = surface_layer(layer, surface) else {
                            continue;
                        };
                        let mut quad = Quad {
                            clip: at.clip,
                            opaque: surface.opaque,
                            ..compose::quad(&placed, at.pixels_per_point, [0.0; 2])
                        };
                        // On whole pixels when upright, as its view's drawing places it, so a
                        // buffer drawn one texel a pixel stays sharp.
                        if quad.rotation == 0.0 {
                            let [left, top, right, bottom] = quad.rect;
                            let (left, top) = (left.round(), top.round());
                            quad.rect = [
                                left,
                                top,
                                left + (right - quad.rect[0]).round(),
                                top + (bottom - quad.rect[1]).round(),
                            ];
                        }
                        bound.push((pipelines.bind(&surface.texture), quad));
                    }
                }
            }
        }
        self.quads.prepare(
            &pipelines,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            bound.iter().map(|(texture, quad)| (texture, *quad)),
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
        // Each program window shown hears so, as when its view draws it.
        for (_, shown) in &self.layers {
            if let Shown::Window(picture) = shown {
                picture.painted();
            }
        }
    }
}
