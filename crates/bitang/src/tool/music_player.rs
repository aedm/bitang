use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use tracing::{debug, info, warn};

struct MusicDevice {
    sink: Sink,
    _stream_handle: OutputStreamHandle,
    _stream: OutputStream,
}

pub struct MusicPlayer {
    device: Option<MusicDevice>,
    is_playing: bool,
    music_file_path: Option<String>,
}

impl MusicPlayer {
    pub fn new() -> Self {
        let Ok((stream, stream_handle)) = OutputStream::try_default() else {
            warn!("No audio device found");
            return Self {
                device: None,
                is_playing: false,
                music_file_path: None,
            };
        };
        let sink = Sink::try_new(&stream_handle).unwrap();

        Self {
            device: Some(MusicDevice {
                sink,
                _stream_handle: stream_handle,
                _stream: stream,
            }),
            is_playing: false,
            music_file_path: None,
        }
    }

    pub fn set_root_path(&mut self, root_path: &str) {
        self.music_file_path = Self::find_music_file(root_path).ok();
    }

    pub fn play_from(&mut self, time: f32) {
        if self.device.is_none() {
            return;
        }
        if time < 0. {
            info!("Music can't be played from negative time");
            return;
        }
        let Some(path) = &self.music_file_path else {
            info!("Music file not found");
            return;
        };
        let Ok(file) = File::open(path) else {
            info!("Music file '{path}' not found");
            return;
        };
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).unwrap();
        self.stop();
        self.device
            .as_mut()
            .unwrap()
            .sink
            .append(source.skip_duration(Duration::from_secs_f32(time)));
        self.is_playing = true;
    }

    pub fn stop(&mut self) {
        if let Some(device) = &mut self.device {
            if self.is_playing {
                device.sink.stop();
                self.is_playing = false;
            }
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
