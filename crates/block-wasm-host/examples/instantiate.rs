use block_plugin_api::{
    EditorInstanceId, EditorRegion, FrameChrome, FrameSpec, HelloAccepted, OpenBlock, Opening,
    ScreenId, ScreenRequest, ViewportMetrics,
    embed::{
        exports::plugin::{self, Editor},
        host::Call,
    },
    ops::{Connection, Incoming, OpId, Outcome, Side},
};
use block_wasm_host::{Host, Plugin};

const SCREENS_SURFACE: u32 = 1;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(path), Some(editor)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: instantiate PLUGIN.wasm EDITOR_ID [CACHE_DIRECTORY]");
        std::process::exit(2);
    };
    let editor = match uuid::Uuid::parse_str(&editor) {
        Ok(editor) => editor.into_bytes(),
        Err(error) => {
            eprintln!("{editor} is not an editor id: {error}");
            std::process::exit(2);
        }
    };
    let cache = arguments.next().map(std::path::PathBuf::from);
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let host = match Host::new(device, queue, cache.as_deref()) {
        Ok(host) => host.with_direct(block_plugin_api::embed::IMPORTS),
        Err(error) => {
            eprintln!("host failed: {error}");
            std::process::exit(1);
        }
    };
    let mut plugin = match host.load(path.as_ref()) {
        Ok(plugin) => plugin,
        Err(error) => {
            eprintln!("load failed: {error}");
            std::process::exit(1);
        }
    };
    println!("instantiated");
    let mut session = Session {
        connection: Connection::new(Side::Host),
        draw: None,
        drawn: false,
    };
    check(plugin.start());
    session.hear("start", &mut plugin);
    let _editor = Editor::new(&mut session.connection, opening(editor));
    plugin::screens(&mut session.connection, screens());
    session.step("open", &mut plugin);
    session.draw = Some(plugin::draw(&mut session.connection, 0, true).op());
    session.step("draw", &mut plugin);
    match plugin.surface(SCREENS_SURFACE) {
        Some((texture, generation)) => println!(
            "surface {SCREENS_SURFACE} is {}x{} at generation {generation}",
            texture.width(),
            texture.height()
        ),
        None => {
            eprintln!("the plugin never asked for a surface");
            std::process::exit(1);
        }
    }
    if !session.drawn {
        eprintln!("the plugin never answered the draw with a frame");
        std::process::exit(1);
    }
    if !plugin.take_presented().contains(&SCREENS_SURFACE) {
        eprintln!("the plugin never presented its surface");
        std::process::exit(1);
    }
    println!("presented");
    plugin.stop();
}

struct Session {
    connection: Connection,
    draw: Option<OpId>,
    drawn: bool,
}

impl Session {
    fn step(&mut self, stage: &str, plugin: &mut Plugin) {
        check(plugin.deliver(self.connection.take_outbound()));
        check(plugin.step());
        self.hear(stage, plugin);
    }

    fn hear(&mut self, stage: &str, plugin: &mut Plugin) {
        let outbound = plugin.take_outbound();
        println!("{stage} sent {} item(s)", outbound.len());
        for item in outbound {
            if let Err(error) = self.connection.receive(&item) {
                println!("  undecodable: {error}");
            }
        }
        let mut answered = false;
        while let Some(event) = self.connection.next_event() {
            match event {
                Incoming::Started(started) if Call::accepts(&started) => {
                    match Call::accept(&mut self.connection, started) {
                        Some(Call::Connect { reply, hello }) => {
                            println!("  connect from {}", hello.plugin.id);
                            reply.send(&mut self.connection, &accepted());
                            answered = true;
                        }
                        Some(call) => println!("  {call:?}"),
                        None => {}
                    }
                }
                Incoming::Completed { op, outcome } if Some(op) == self.draw => {
                    self.drawn = matches!(outcome, Outcome::Ok(_));
                    println!(
                        "  a frame, {}",
                        if self.drawn { "drawn" } else { "refused" }
                    );
                }
                event => println!("  {event:?}"),
            }
        }
        if answered {
            self.step(stage, plugin);
        }
    }
}

fn check(outcome: Result<(), String>) {
    if let Err(error) = outcome {
        eprintln!("the plugin failed: {error}");
        std::process::exit(1);
    }
}

fn accepted() -> HelloAccepted {
    HelloAccepted {
        host_name: "instantiate".to_owned(),
        surface: Some(block_plugin_api::SurfaceSpec {
            format: block_plugin_api::SurfaceFormat::Rgba8Unorm,
            render: true,
        }),
        preferences: block_plugin_api::Preferences::default(),
    }
}

fn opening(editor: [u8; 16]) -> Opening {
    Opening::Block(OpenBlock {
        instance: EditorInstanceId(1),
        editor,
        block_id: [1; 16],
        block_type: [2; 16],
        view_block: None,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    })
}

fn metrics() -> ViewportMetrics {
    ViewportMetrics {
        logical: block_plugin_api::Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        },
        visible: block_plugin_api::Pos2::ZERO,
        pixel_width: WIDTH,
        pixel_height: HEIGHT,
        scale_factor: 1.0,
        raster: 1.0,
        monitors: Vec::new(),
    }
}

fn screens() -> Vec<ScreenRequest> {
    vec![
        ScreenRequest {
            screen: ScreenId(1),
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content: None,
                top_bar: block_plugin_api::TopBar::Hidden,
            }),
            opens_windows: false,
            metrics: metrics(),
        },
        ScreenRequest {
            screen: ScreenId(2),
            instance: EditorInstanceId(1),
            region: EditorRegion::Preview,
            frame: None,
            opens_windows: false,
            metrics: metrics(),
        },
    ]
}
