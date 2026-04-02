// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::io::BufReader;
use std::fs::File;
use std::time::Duration;

use rfd::{AsyncFileDialog, FileHandle};
use rodio::{Decoder, decoder::DecoderBuilder, Source};
use slint::{Timer, TimerMode, Image};

slint::include_modules!();

struct Media {
    file: FileHandle,
    title: Option<String>,
    artist: Option<String>,
    _source: Option<Decoder<BufReader<File>>>,
}
impl Media {
    pub fn new(file: FileHandle) -> Self {
        let (title, artist, _) = read_metadata(file.path());
        Self {
            file,
            title,
            artist,
            _source: None,
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    // TODO do we need to save the source in the struct? we probably will only need it once
    /// Moves `source` out of self if it already exists, otherwise creates and returns new `source`
    pub fn _source(&mut self) -> Result<Decoder<BufReader<File>>, <Decoder<BufReader<File>> as TryFrom<BufReader<File>>>::Error> {
        if let Some(_) = &mut self._source {
            let res = std::mem::replace(&mut self._source, None);
            return Ok(res.unwrap())
        } else {
            return self.create_source()
        }
    }

    fn create_source(&self) -> Result<Decoder<BufReader<File>>, <Decoder<BufReader<File>> as TryFrom<BufReader<File>>>::Error> {
        let file = File::open(self.path()).unwrap();
        let len = file.metadata().unwrap().len();
        let buf = BufReader::new(file);
        let decoder = DecoderBuilder::new()
            .with_data(buf)
            .with_byte_len(len)
            .build()?;
        Ok(decoder)
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

    let current_file: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));
    // TODO do we actually need to save the current media like this? or can we just pass the info
    // to the ui and then be done with it?
    let _current_media: Arc<Mutex<Option<Media>>> = Arc::new(Mutex::new(None));
    let current_time_update = Arc::new(Timer::default());
    let current_queue: Vec<Box<Path>> = Vec::new();


    let audio_sink = rodio::DeviceSinkBuilder::open_default_sink()
        .expect("open default audio stream");
    let audio_player = Arc::new(rodio::Player::connect_new(&audio_sink.mixer()));
    
    let ui = MainWindow::new()?;

    let mainpage_background = Image::load_from_path(&Path::new("ui/images/Background Teku.png")).unwrap();
    ui.set_mainpage_background(mainpage_background);

    ui.on_play_button({
        let ui_handle = ui.as_weak();
        let current_file_handle = Arc::clone(&current_file);
        let player_handle = Arc::clone(&audio_player);
        move || {
            let ui = ui_handle.unwrap();

            if current_file_handle.lock().unwrap().is_some() {
                if !player_handle.is_paused() {
                    player_handle.pause();
                    ui.set_media_playing(false);
                    
                } else {
                    player_handle.play();
                    ui.set_media_playing(true);
                }
            }
        }
    });

    // This timer is responsible for continuously reading the current song position from the audio
    // player, and then updating the ui seek bar such that it follows the current position of the
    // song
    let ui_handle = ui.as_weak();
    let player_handle = Arc::clone(&audio_player);
    current_time_update.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let ui = ui_handle.unwrap();
        if ui.get_media_playing() {
            // println!("--------------");
            // println!("current time: {}", ui.get_current_time());
            ui.set_current_time(player_handle.get_pos().as_millis() as i32);
            // println!("current_time_update: {}", player_handle.get_pos().as_millis() as i32);
        }
    });
    
    
    let ui_handle = ui.as_weak();
    let player_handle = Arc::clone(&audio_player);
    ui.on_file_button(move || {
        let current_file_handle = Arc::clone(&current_file);
        let ui = ui_handle.unwrap();
        let player_handle = Arc::clone(&player_handle);
        slint::spawn_local(async move {
            let file = AsyncFileDialog::new()
                .add_filter("audio", &["ogg", "wav", "flac", "mp3"])
                .set_directory("/")
                .pick_file()
                .await;
            if let Some(handle) = file {
                let mut current_file = current_file_handle.lock().unwrap();
                *current_file = Some(handle.clone());
                let new_media = Media::new(handle);
                set_metadata(&ui, &new_media);
                let source = new_media.create_source().unwrap();
                if let Some(duration) = source.total_duration() {
                    ui.set_total_duration(duration.as_millis() as i32);
                };
                player_handle.clear();
                player_handle.append(source);
                ui.set_current_time(player_handle.get_pos().as_millis() as i32);
                // ui.set_media_artist()
                ui.set_file_selected(true);
            }
        }).unwrap();
    });

    let ui_handle = ui.as_weak();
    let audio_player_handle = Arc::clone(&audio_player);
    ui.on_change_volume(move || {
        let ui = ui_handle.unwrap();
        audio_player_handle.set_volume(ui.get_volume_level());
    });

    // Pauses the timer that continuously updates the seek bar with current song position whenever
    // the user starts dragging the seek bar. The timer must be restarted later, after the user
    // finishes dragging the bar and we seek to position
    let time_update_handle = Arc::clone(&current_time_update);
    ui.on_seek_press(move || {
        time_update_handle.stop();
    });

    // Call try_seek from the audio player, then restart the timer that continuously updates the seek
    // bar with current position. This is required because when the user starts dragging the seek bar
    // we pause the update timer so that it doesn't keep trying to update the slider value while the
    // user is holding it in place
    let time_update_handle = Arc::clone(&current_time_update);
    let player_handle = Arc::clone(&audio_player);
    let ui_handle = ui.as_weak();
    ui.on_seek(move |time| {
        let ui = ui_handle.unwrap();
        player_handle.try_seek(Duration::from_millis(time.try_into().unwrap())).unwrap();
        time_update_handle.restart();
        let last = ui.get_resume_update_flag();
        ui.set_resume_update_flag(last + 1);
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

fn build_queue(folder_path: &Path) {

}