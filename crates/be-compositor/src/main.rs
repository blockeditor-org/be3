use be_compositor::app::Compositor;
use be_compositor::server::Server;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut launches: Vec<String> = std::env::args().skip(1).collect();
    if launches
        .first()
        .is_some_and(|argument| argument == "--session")
    {
        launches.remove(0);
        return be_compositor::session::run(launches);
    }
    let server = Server::new()?;
    let mut options = beui::RunOptions::new("BE Compositor");
    options.app_id = Some("be-compositor".to_owned());
    options.open_device = Some(std::sync::Arc::new(be_compositor::gpu::open_device));
    beui::run_with(options, Compositor::new(server, launches))
}
