use super::*;
use winit::keyboard::{Key as Winit, NativeKey};

#[test]
fn the_mic_mute_keysym_is_the_mic_mute_key() {
    let mic_mute = Winit::Unidentified(NativeKey::Xkb(XKB_AUDIO_MIC_MUTE));
    assert!(matches!(logical_key(&mic_mute), Logical::Key(Key::MicMute)));
    let volume_mute = Winit::Named(NamedKey::AudioVolumeMute);
    assert!(matches!(
        logical_key(&volume_mute),
        Logical::Key(Key::VolumeMute)
    ));
}
