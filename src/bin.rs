// Remember to  $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.

// TODO: Labels should be added to the left of the entries in the GUI
// TODO: I saw some peculiar photons that needed logging
use std::fs::File;

#[macro_use]
extern crate log;
extern crate simplelog;

use iced::{Application, Result};
use simplelog::*;

use librpysight::gui::MainAppGui;
use librpysight::{load_app_settings, reload_cfg_or_use_default};

fn main() -> Result {
    let _ = WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("target/rpysight.log").unwrap(),
    )
    .unwrap();
    info!("Logger initialized successfully, starting RPySight");
    let cfg = reload_cfg_or_use_default();
    let settings = load_app_settings(cfg);
    MainAppGui::run(settings)
}
