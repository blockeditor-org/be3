use super::*;

#[test]
fn products_are_named_as_they_are_parsed() {
    let mut products = vec![Product::Android, Product::LauncherAndroid];
    for platform in PLATFORMS {
        products.extend([Product::Native(platform), Product::BeuiDemo(platform)]);
    }
    for product in products {
        assert_eq!(Product::parse(&product.name()), Some(product));
    }
    assert!(PLATFORMS.contains(&platform()));
    for unknown in ["ios", "native-plan9_x86_64", "native-", "beui-demo"] {
        assert_eq!(Product::parse(unknown), None);
    }
}
