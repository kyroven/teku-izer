// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::io::BufReader;
use std::fs::File;

use rfd::{AsyncFileDialog, FileHandle};
use rodio::Decoder;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    // use slint::Model;

    let current_file: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));

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
                    *current_file = Some(handle.clone());
                    read_metadata(&ui, handle.path());
                    let buf = BufReader::new(File::open(handle.path()).unwrap());
                    audio_player_handle.clear();
                    audio_player_handle.append(Decoder::try_from(buf).unwrap());
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

fn read_metadata(ui: &MainWindow, path: &Path) {
    if let Some(ext) = path.extension() {
        if ext == "mp3" || ext == "mp4" || ext == "flac" {
            let tag = audiotags::Tag::new().read_from_path(path).unwrap();
            if let Some(title) = tag.title() {
                ui.set_media_title(String::from(title).into());
            } else {
                ui.set_media_title(String::from("UNKNOWN TRACK").into());
            }
            
            if let Some(artist) = tag.artist() {
                ui.set_media_artist(String::from(artist).into());
            } else {
                ui.set_media_artist(String::from("UNKNOWN ARTIST").into());
            }
        } else {
            ui.set_media_title(String::from("UNKNOWN TRACK").into());
            ui.set_media_artist(String::from("UNKNOWN ARTIST").into());
        }
    }
}