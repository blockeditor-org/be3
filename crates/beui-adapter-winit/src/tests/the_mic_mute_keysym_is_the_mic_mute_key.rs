use super::*;
use winit::keyboard::{Key as Winit, NamedKey, NativeKey};

#[test]
fn the_mic_mute_keysym_is_the_mic_mute_key() {
    let mic_mute = Winit::Unidentified(NativeKey::Xkb(XKB_AUDIO_MIC_MUTE));
    assert_eq!(logical_key(&mic_mute, None), Some(Key::MicMute));
    let volume_mute = Winit::Named(NamedKey::AudioVolumeMute);
    assert_eq!(logical_key(&volume_mute, None), Some(Key::VolumeMute));
}
