// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};

use rfd::{AsyncFileDialog, FileHandle};

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    // use slint::Model;

    let current_file: Arc<Mutex<Option<FileHandle>>> = Arc::new(Mutex::new(None));
    
    let ui = MainWindow::new()?;

    // let file = Option<

    
    ui.on_play_button({
        let current_file_handle = Arc::clone(&current_file);
        move || {
            println!("play song!");

            let val = current_file_handle.lock().unwrap();
            println!("hmm: {val:?}",);
        }
    });

    
    ui.on_file_button({
        let ui_handle = ui.as_weak();
        move || {
            let current_file_handle = Arc::clone(&current_file);
            let ui = ui_handle.unwrap();
            println!("file button!");
            slint::Timer::single_shot(std::time::Duration::from_secs(1), move || {
                println!("test");
            });
            slint::spawn_local(async move {
                let file = AsyncFileDialog::new()
                    .add_filter("text", &["txt", "rs"])
                    .add_filter("rust", &["rs", "toml"])
                    .set_directory("/")
                    .pick_file()
                    .await;
                let prev = ui.get_test_counter();
                println!("{prev}");
                ui.set_test_counter(prev + 1);
                if let Some(f) = file {
                    let mut current_file = current_file_handle.lock().unwrap();
                    // tx.send(f).unwrap();
                    *current_file = Some(f);
                }
            }).unwrap();
    }});
    
    ui.run()?;
    

    Ok(())
}

// Fetch the tiles from the model
// let mut tiles: Vec<TileData> = main_window.get_memory_tiles().iter().collect();
// Duplicate them to ensure that we have pairs
// tiles.extend(tiles.clone());

// Randomly mix the tiles
// use rand::seq::SliceRandom;
// let mut rng = rand::rng();
// tiles.shuffle(&mut rng);

// // Assign the shuffled Vec to the model property
// let tiles_model = std::rc::Rc::new(slint::VecModel::from(tiles));
// main_window.set_memory_tiles(tiles_model.clone().into());

// let main_window_weak = main_window.as_weak();
// main_window.on_check_if_pair_solved(move || {
//     let mut flipped_tiles = 
//         tiles_model.iter().enumerate().filter(|(_, tile)| tile.image_visible && !tile.solved);

//     if let (Some((t1_idx, mut t1)), Some((t2_idx, mut t2))) =
//         (flipped_tiles.next(), flipped_tiles.next())
//     {
//         let is_pair_solved = t1 == t2;
//         if is_pair_solved {
//             t1.solved = true;
//             tiles_model.set_row_data(t1_idx, t1);
//             t2.solved = true;
//             tiles_model.set_row_data(t2_idx, t2);
//         } else {
//             let main_window = main_window_weak.unwrap();
//             main_window.set_disable_tiles(true);
//             let tiles_model = tiles_model.clone();
//             slint::Timer::single_shot(std::time::Duration::from_secs(1), move || {
//                 main_window.set_disable_tiles(false);
//                 t1.image_visible = false;
//                 tiles_model.set_row_data(t1_idx, t1);
//                 t2.image_visible = false;
//                 tiles_model.set_row_data(t2_idx, t2);
//             });
//         }
//     }
// });