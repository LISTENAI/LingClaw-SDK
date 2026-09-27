use crate::runtime::Tone;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, buffer::SamplesBuffer};

/// Reproduce the device's 16 kHz square-wave PCM, at a modest desktop volume.
pub fn samples(tone: Tone) -> Vec<f32> {
    let mut phase = 0;
    (0..tone.ms * 16)
        .map(|_| {
            phase = (phase + tone.hz) % 16000;
            if phase < 8000 {
                12000.0 / 32768.0
            } else {
                -12000.0 / 32768.0
            }
        })
        .collect()
}
pub struct Audio {
    player: Player,
    stream: MixerDeviceSink,
}
impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        let mut stream = DeviceSinkBuilder::open_default_sink()?;
        stream.log_on_drop(false);
        let player = Player::connect_new(stream.mixer());
        player.set_volume(0.4);
        Ok(Self { player, stream })
    }
    pub fn play(&self, tone: Tone) {
        // One playing tone and at most four queued tones.
        if self.player.len() < 5 {
            self.player.append(SamplesBuffer::new(
                rodio::nz!(1),
                rodio::nz!(16000),
                samples(tone),
            ));
        }
    }
    pub fn idle(&self) -> bool {
        self.player.empty()
    }
    pub fn stop(&mut self) {
        if self.player.empty() {
            return;
        }
        self.player.stop();
        self.player = Player::connect_new(self.stream.mixer());
        self.player.set_volume(0.4);
    }
}
