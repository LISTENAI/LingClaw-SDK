use lingclaw_sdk::{
    audio::{Audio, samples},
    runtime::Tone,
};
#[test]
fn square_wave_has_device_sample_count_amplitude_and_frequency() {
    let values = samples(Tone { hz: 1000, ms: 100 });
    assert_eq!(values.len(), 1600);
    let transitions = values
        .windows(2)
        .filter(|pair| pair[0].is_sign_positive() != pair[1].is_sign_positive())
        .count();
    assert_eq!(transitions, 200);
    assert!(values.iter().all(|value| value.abs() == 12000.0 / 32768.0));
}
#[test]
#[ignore = "requires a real system audio output and plays a short tone"]
fn system_audio_outputs_tone() {
    let mut audio = Audio::new().unwrap();
    audio.play(Tone { hz: 880, ms: 160 });
    assert!(!audio.idle());
    let start = std::time::Instant::now();
    while !audio.idle() && start.elapsed() < std::time::Duration::from_secs(3) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(audio.idle(), "audio device did not consume the tone");
    audio.play(Tone { hz: 440, ms: 3000 });
    audio.stop();
    assert!(audio.idle());
}
