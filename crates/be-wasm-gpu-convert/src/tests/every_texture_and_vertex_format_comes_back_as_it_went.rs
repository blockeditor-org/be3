use super::*;

fn round_trips<A, W>(minimum: usize)
where
    A: serde::de::DeserializeOwned + Convert<W> + Copy + PartialEq + std::fmt::Debug,
    W: TryConvert<A> + std::fmt::Debug,
{
    let cases = every::<A>();
    assert!(
        cases.len() >= minimum,
        "only {} cases of {}",
        cases.len(),
        std::any::type_name::<A>()
    );
    for case in cases {
        let converted: W = case.convert();
        let back = converted
            .try_convert()
            .expect("what the ABI names, wgpu names too");
        assert_eq!(back, case);
    }
}

#[test]
fn every_texture_and_vertex_format_comes_back_as_it_went() {
    round_trips::<abi::TextureFormat, wgpu::TextureFormat>(40);
    round_trips::<abi::VertexFormat, wgpu::VertexFormat>(30);
}
