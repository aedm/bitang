use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use rodio::{source, Decoder, OutputStream, Sink, Source};
use tracing::{debug, error, info, warn};

struct MusicDevice {
    sink: Sink,
    _stream: OutputStream,
    music_file_path: String,
}

pub struct MusicPlayer {
    device: Option<MusicDevice>,
    is_playing: bool,
}

impl MusicPlayer {
    pub fn new() -> Self {
        Self {
            device: None,
            is_playing: false,
        }
    }

    pub fn set_root_path(&mut self, root_path: &str) {
        let Ok(stream) = rodio::OutputStreamBuilder::open_default_stream() else {
            warn!("No audio device found");
            return;
        };
        let Ok(music_file_path) = Self::find_music_file(root_path) else {
            warn!("Music file not found");
            return;
        };
        let sink = rodio::Sink::connect_new(stream.mixer());
        self.device = Some(MusicDevice {
            sink,
            _stream: stream,
            music_file_path,
        });
    }

    pub fn play_from(&mut self, time: f32) {
        if time < 0. {
            warn!("Music can't be played from negative time");
            return;
        }
        self.stop();
        let Some(device) = self.device.as_mut() else {
            warn!("Music device not found");
            return;
        };
        let Ok(file) = File::open(&device.music_file_path) else {
            error!("Failed to open music file: {}.", device.music_file_path);
            return;
        };
        let Ok(source) = Decoder::try_from(file) else {
            error!("Failed to decode music file: {}.", device.music_file_path);
            return;
        };
        device.sink.append(source);
        if let Err(err) = device.sink.try_seek(Duration::from_secs_f32(time)) {
            error!("Failed to seek music: {err}");
        }
        device.sink.play();
        self.is_playing = true;
    }

    pub fn stop(&mut self) {
        if let Some(device) = &mut self.device {
            device.sink.stop();
            self.is_playing = false;
        }
    }

    fn find_music_file(root_path: &str) -> Result<String> {
        let entries = std::fs::read_dir(root_path)
            .with_context(|| format!("Failed to read directory: {root_path}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(path) = path.to_str() {
                    if path.ends_with(".mp3") {
                        debug!("Music file found: {path}");
                        return Ok(path.to_string());
                    }
                }
            }
        }
        bail!("No music file found in '{root_path}'")
    }
}
