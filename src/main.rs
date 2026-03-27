// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::io::BufReader;

use rfd::{AsyncFileDialog, FileHandle};

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    // use slint::Model;

    let current_file: Option<FileHandle> = None;
    
    let main_window = MainWindow::new()?;

    // let file = Option<

    
    main_window.on_play_button(move || {
        println!("play song!");
    });
    
    main_window.on_file_button(move || {
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
        
    }).unwrap();
    
    });
    
    main_window.run()?;
    

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