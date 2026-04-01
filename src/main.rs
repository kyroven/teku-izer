// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::io::BufReader;
use std::fs::File;
use std::time::Duration;

use rfd::{AsyncFileDialog, FileHandle};
use rodio::{Decoder, Source};

slint::include_modules!();

struct Media {
    file: FileHandle,
    duration: Option<f64>,
    title: Option<String>,
    artist: Option<String>,
    source: Option<Decoder<BufReader<File>>>,
}
impl Media {
    pub fn new(file: FileHandle) -> Self {
        let (title, artist, duration) = read_metadata(file.path());
        Self {
            file,
            duration,
            title,
            artist,
            source: None,
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Moves `source` out of self if it already exists, otherwise creates and returns new `source`
    pub fn source(&mut self) -> Result<Decoder<BufReader<File>>, <Decoder<BufReader<File>> as TryFrom<BufReader<File>>>::Error> {
        if let Some(_) = &mut self.source {
            let res = std::mem::replace(&mut self.source, None);
            return Ok(res.unwrap())
        } else {
            return self.create_source()
        }
    }

    fn create_source(&self) -> Result<Decoder<BufReader<File>>, <Decoder<BufReader<File>> as TryFrom<BufReader<File>>>::Error> {
        let file = File::open(self.path()).unwrap();
        let buf = BufReader::new(file);
        Decoder::try_from(buf)
    }
}

fn read_metadata(path: &Path) -> (Option<String>, Option<String>, Option<f64>) {
    if let Some(ext) = path.extension() {
        if ext == "mp3" || ext == "mp4" || ext == "flac" {
            let tag = audiotags::Tag::new().read_from_path(path).unwrap();
            println!("duration: {:?}", tag.duration());
            return (tag.title().map(String::from), tag.artist().map(String::from), tag.duration())
        } else {
            return (None, None, None)
        }
    }
    // TODO i don't think we need this because i don't think path.extension() will ever return none
    // or at least if it would, we would've hit an error before this point
    return (None, None, None)
}

fn main() -> Result<(), Box<dyn Error>> {
    // use slint::Model;

    let current_file: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));
    let current_media: Arc<Mutex<Option<Media>>> = Arc::new(Mutex::new(None));

    let current_length: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));

    let audio_sink_handle = rodio::DeviceSinkBuilder::open_default_sink()
        .expect("open default audio stream");
    // let test_file = BufReader::new(File::open("examples/funky.wav").unwrap());
    // let audio_player = Arc::new(rodio::play(&audio_sink_handle.mixer(), test_file).unwrap());
    let audio_player = Arc::new(rodio::Player::connect_new(&audio_sink_handle.mixer()));
    
    let ui = MainWindow::new()?;

    ui.on_play_button({
        let ui_handle = ui.as_weak();
        let current_file_handle = Arc::clone(&current_file);
        let audio_player_handle = Arc::clone(&audio_player);
        move || {
            let ui = ui_handle.unwrap();

            let val = current_file_handle.lock().unwrap();
            if let Some(_) = *val {
                if !audio_player_handle.is_paused() {
                    audio_player_handle.pause();
                    ui.set_media_playing(false);
                    
                } else {
                    audio_player_handle.play();
                    ui.set_media_playing(true);
                }
            }
        }
    });

    ui.on_file_button({
        let ui_handle = ui.as_weak();
        let audio_player = Arc::clone(&audio_player);
        move || {
            let current_file_handle = Arc::clone(&current_file);
            let current_media_handle = Arc::clone(&current_media);
            let ui = ui_handle.unwrap();
            let audio_player_handle = Arc::clone(&audio_player);
            slint::spawn_local(async move {
                let file = AsyncFileDialog::new()
                    .add_filter("audio", &["ogg", "wav", "flac", "mp3"])
                    .set_directory("/")
                    .pick_file()
                    .await;
                if let Some(handle) = file {
                    let mut current_file = current_file_handle.lock().unwrap();
                    let mut current_media = current_media_handle.lock().unwrap();
                    *current_file = Some(handle.clone());
                    let new_media = Media::new(handle);
                    set_metadata(&ui, &new_media);
                    let source = new_media.create_source().unwrap();
                    if let Some(duration) = source.total_duration() {
                        ui.set_total_duration(duration.as_millis() as i32);
                    };
                    audio_player_handle.clear();
                    audio_player_handle.append(source);
                    ui.set_current_time(audio_player_handle.get_pos().as_millis() as i32);
                    // ui.set_media_artist()
                    ui.set_file_selected(true);
                }
            }).unwrap();
    }});

    ui.on_change_volume({
        let ui_handle = ui.as_weak();
        let audio_player_handle = Arc::clone(&audio_player);
        move || {
            let ui = ui_handle.unwrap();
            audio_player_handle.set_volume(ui.get_volume_level());
        }
    });
    
    ui.run()?;
    

    Ok(())
}

fn set_metadata(ui: &MainWindow, media: &Media) {
    if let Some(title) = &media.title {
        ui.set_media_title(title.into());
    } else {
        ui.set_media_title(String::from("UNKNOWN TITLE").into());
    }
    if let Some(artist) = &media.artist {
        ui.set_media_artist(artist.into());
    } else {
        ui.set_media_artist(String::from("UNKNOWN ARTIST").into());
    }
}