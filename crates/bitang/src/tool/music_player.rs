use crate::file::ROOT_FOLDER;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use tracing::{info};

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
        let path = format! {"{ROOT_FOLDER}/music.mp3"};
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
}
