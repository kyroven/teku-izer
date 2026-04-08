// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::io;
use std::io::BufReader;
use std::fs;
use std::fs::File;
use std::time::Duration;
use std::fmt;

use rfd::{AsyncFileDialog, FileHandle};
use rodio::source;
use rodio::{Decoder, decoder::DecoderBuilder, Source};
use slint::{Timer, TimerMode, Image, Model};
use slint;

slint::include_modules!();

const SUPPORTED_FILE_TYPES: [&str; 4] = ["ogg", "wav", "mp3", "flac"];

struct Media {
    path: PathBuf,
    title: Option<String>,
    artist: Option<String>,
    _source: Option<Decoder<BufReader<File>>>,
}
impl Media {
    pub fn new(path: &Path) -> Self {
        let (title, artist, _) = read_metadata(path);
        Self {
            path: path.to_path_buf(),
            title,
            artist,
            _source: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn title(&self) -> Option<&str> {
        if let Some(s) = &self.title {
            Some(s)
        } else {
            None
        }
    }

    pub fn artist(&self) -> Option<&str> {
        if let Some(s) = &self.artist {
            Some(s)
        } else {
            None
        }
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
impl fmt::Debug for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Media")
         .field("path", &self.path)
         .field("title", &self.title)
         .field("artist", &self.artist)
         .finish()
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

    // TODO actually i don't think any of these need to be Arcs
    // I thought slint::spawn_local spawned a new thread but it actually doesn't
    let current_file: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));
    let current_folder: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));
    // TODO do we actually need to save the current media like this? or can we just pass the info
    // to the ui and then be done with it?``
    let _current_media: Arc<Mutex<Option<Media>>> = Arc::new(Mutex::new(None));
    let current_time_update = Arc::new(Timer::default());
    let current_queue: Rc<RefCell<Vec<Media>>> = Rc::new(RefCell::new(Vec::new()));
    let mut current_idx: i32 = 0;

    let audio_sink = rodio::DeviceSinkBuilder::open_default_sink()
        .expect("open default audio stream");
    let audio_player = Arc::new(rodio::Player::connect_new(&audio_sink.mixer()));
    
    let ui = MainWindow::new()?;

    let queue: Vec<MediaData> = ui.get_media_list().iter().collect();
    let queue_model = Rc::new(slint::VecModel::from(queue));
    ui.set_media_list(queue_model.clone().into());

    let mainpage_background = Image::load_from_path(&Path::new("ui/images/Background Teku.png")).unwrap();
    ui.set_mainpage_background(mainpage_background);

    ui.on_toggle_play({
        let ui_handle = ui.as_weak();
        let current_file_handle = Arc::clone(&current_file);
        let player_handle = Arc::clone(&audio_player);
        move || {
            let ui = ui_handle.unwrap();

            if !player_handle.empty() {
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
    let current_file_handle = Arc::clone(&current_file);
    ui.on_media_file_select(move || {
        let current_file_handle = Arc::clone(&current_file_handle);
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
                let new_media = Media::new(handle.path());
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

    let queue_model_handle = queue_model.clone();
    let current_queue_handle = current_queue.clone();
    let ui_handle = ui.as_weak();
    let player_handle = Arc::clone(&audio_player);
    // TODO will need to change this to a box and probably use refcell. it's just an int but it still
    // is quite important to not run into a race condition with this value
    let mut current_idx_handle = &mut current_idx;
    ui.on_play_media(move |idx| {
        let ui = ui_handle.unwrap();
        *current_idx_handle = idx;

        let media_list: Vec<MediaData> = queue_model_handle.iter().collect();
        let mut target_model = media_list[idx as usize].clone();
        target_model.playing = true;
        queue_model_handle.set_row_data(idx as usize, target_model);
        let target = &current_queue_handle.borrow()[idx as usize];
        
        start_new_playback(&ui, target, player_handle.clone());
        // load_next_media(player_handle.clone(), current_queue_handle.clone(), idx as usize, false);
    });

    let ui_handle = ui.as_weak();
    let player_handle = Arc::clone(&audio_player);
    let current_folder_handle = Arc::clone(&current_folder);
    let queue_model_handle = queue_model.clone();
    let current_queue_handle = current_queue.clone();
    ui.on_media_folder_select(move || {
        let ui = ui_handle.unwrap();
        let player_handle = Arc::clone(&player_handle);
        let current_folder_handle = Arc::clone(&current_folder_handle);
        // let current_queue_handle = Arc::clone(&current_queue_handle);
        let queue_model = queue_model_handle.clone();
        let current_queue_handle = current_queue_handle.clone();
        slint::spawn_local(async move {
            let folder = AsyncFileDialog::new()
                // .add_filter("folder", &["ogg", "wav", "flac", "mp3"])
                .set_directory("/")
                .pick_folder()
                .await;
            if let Some(handle) = folder {
                let mut current_folder = current_folder_handle.lock().unwrap();
                *current_folder = Some(handle.clone());
                let queue = build_queue(handle.path()).unwrap();
                let mut media_list: Vec<MediaData> = Vec::new();
                for media in &queue {
                    let mut model = MediaData::default();
                    model.title = slint::SharedString::from(media.title().unwrap_or("Unknown Title"));
                    model.artist = slint::SharedString::from(media.artist().unwrap_or("Unknown Artist"));
                    println!("title: {}", model.title);
                    println!("artist: {}", model.artist);
                    media_list.push(model);
                }
                *current_queue_handle.borrow_mut() = queue;
                queue_model.set_vec(media_list);
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


    let player_handle = Arc::clone(&audio_player);
    let current_queue_handle = current_queue.clone();
    let ui_handle = ui.as_weak();
    ui.on_load_next_media(move || {
        load_next_media(&ui_handle.clone().unwrap(), player_handle.clone(), current_queue_handle.clone(), current_idx as usize, false);
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

fn build_queue(dir: &Path) -> io::Result<Vec<Media>> {
    let mut queue = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            queue.append(&mut build_queue(&path)?);
        } else {
            if let Some(os_ext) = path.extension() {
                if let Some(ext) = os_ext.to_str() {
                    if SUPPORTED_FILE_TYPES.contains(&ext) {
                        queue.push(Media::new(&path));
                    }
                }
            }
        }
    }

    Ok(queue)
}

fn start_new_playback(ui: &MainWindow, media: &Media, player: Arc<rodio::Player>) {
    set_metadata(&ui, media);
    let source = media.create_source().unwrap();
    if let Some(duration) = source.total_duration() {
        ui.set_total_duration(duration.as_millis() as i32);
    };
    player.clear();
    player.append(source);
    let ui_handle = ui.as_weak();
    let track_finished_callback = source::EmptyCallback::new(Box::new(move || {
        let ui_handle = ui_handle.clone();
        slint::invoke_from_event_loop(move || {
            ui_handle.unwrap().invoke_load_next_media();
        }).unwrap();
    }));
    player.append(track_finished_callback);
    ui.set_current_time(player.get_pos().as_millis() as i32);
    ui.set_file_selected(true);
    player.play();
    ui.set_media_playing(true);
}

fn load_next_media(ui: &MainWindow, player: Arc<rodio::Player>, queue_handle: Rc<RefCell<Vec<Media>>>, current_idx: usize, repeat: bool) {
    let queue = queue_handle.borrow();
    if current_idx == (queue.len() - 1) && repeat {
        let target_idx = 0;
        let target = &queue[target_idx];
        // player.append(target.create_source().unwrap());
        start_new_playback(ui, target, player);
    } else if current_idx < (queue.len() - 1) {
        let target_idx = current_idx + 1;
        let target = &queue[target_idx];
        start_new_playback(ui, target, player);
    }
}