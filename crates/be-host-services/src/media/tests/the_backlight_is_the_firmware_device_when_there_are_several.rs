use super::*;
use backlight::{Device, find};

fn device(root: &std::path::Path, name: &str, kind: &str, brightness: u32, max: u32) {
    let directory = root.join(name);
    std::fs::create_dir_all(&directory).expect("the device directory is made");
    for (file, value) in [
        ("type", kind.to_owned()),
        ("brightness", format!("{brightness}\n")),
        ("max_brightness", format!("{max}\n")),
    ] {
        std::fs::write(directory.join(file), value).expect("the device file is written");
    }
}

#[test]
fn the_backlight_is_the_firmware_device_when_there_are_several() {
    let root = std::env::temp_dir().join(format!("block-app-backlight-{}", uuid::Uuid::new_v4()));
    assert_eq!(find(&root), None, "no directory has no backlight");

    device(&root, "intel_backlight", "raw\n", 19200, 96000);
    assert_eq!(
        find(&root),
        Some(Device {
            name: "intel_backlight".to_owned(),
            brightness: 19200,
            max: 96000,
        })
    );

    device(&root, "acpi_video0", "firmware\n", 4, 15);
    device(&root, "broken", "platform\n", 0, 0);
    assert_eq!(
        find(&root).map(|device| device.name),
        Some("acpi_video0".to_owned())
    );
    std::fs::remove_dir_all(&root).expect("the devices are removed");
}
