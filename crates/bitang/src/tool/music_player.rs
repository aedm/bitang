use crate::file::ROOT_FOLDER;
use anyhow::{anyhow, Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use tracing::{debug, info};

pub struct MusicPlayer {
    sink: Sink,
    _stream_handle: OutputStreamHandle,
    _stream: OutputStream,
    is_playing: bool,
}

impl MusicPlayer {
    pub fn new() -> Self {
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        Self {
            sink,
            _stream_handle: stream_handle,
            _stream: stream,
            is_playing: false,
        }
    }

    pub fn play_from(&mut self, time: f32) {
        if time < 0. {
            info!("Music can't be played from negative time");
            return;
        }
        let Ok(path) = Self::find_music_file() else {
            info!("Music file not found");
            return;
        };
        let Ok(file) = File::open(&path) else {
            info!("Music file '{path}' not found");
            return;
        };
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).unwrap();
        self.stop();
        self.sink
            .append(source.skip_duration(Duration::from_secs_f32(time)));
        self.is_playing = true;
    }

    pub fn stop(&mut self) {
        if self.is_playing {
            self.sink.stop();
            self.is_playing = false;
        }
    }

    fn find_music_file() -> Result<String> {
        let entries = std::fs::read_dir(ROOT_FOLDER)
            .with_context(|| format!("Failed to read directory: {}", ROOT_FOLDER))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(path) = path.to_str() {
                    if path.ends_with(".mp3") {
                        debug!("Music file found: {}", path);
                        return Ok(path.to_string());
                    }
                }
            }
        }
        Err(anyhow!("No music file found in {}", ROOT_FOLDER))
    }
}
