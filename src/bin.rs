// Remember to  $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.
use std::fs::File;

#[macro_use]
extern crate log;
extern crate simplelog;

use iced::{Application, Result, Settings};
use simplelog::*;

use librpysight::gui::ConfigGui;

fn main() -> Result {
    let _ = WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("target/rpysight.log").unwrap(),
    )
    .unwrap();
    info!("Logger initialized successfully, starting RPySight");
    ConfigGui::run(Settings::default())
}
