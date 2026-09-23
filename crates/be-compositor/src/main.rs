use be_compositor::app::Compositor;
use be_compositor::server::Server;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let launches: Vec<String> = std::env::args().skip(1).collect();
    let server = Server::new()?;
    let mut options = beui::RunOptions::new("BE Compositor");
    options.app_id = Some("be-compositor".to_owned());
    beui::run_with(options, Compositor::new(server, launches))
}
