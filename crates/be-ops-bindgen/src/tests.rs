use super::*;

mod a_network_world_refuses_a_sync_func;
mod bytes_are_encoded_whole_in_every_position;

fn generated(wit: &str, network: bool) -> Result<String, String> {
    let mut resolve = Resolve::new();
    let package = resolve
        .push_str("test.wit", wit)
        .map_err(|error| format!("{error:#}"))?;
    let options = Options {
        world: "test".to_owned(),
        network,
        ..Options::default()
    };
    Generator::generate(options, resolve, package).map(|tokens| tokens.to_string())
}
