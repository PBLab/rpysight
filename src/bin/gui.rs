// Remember to $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.
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
        ConfigBuilder::default().set_time_to_local(true).build(),
        File::create("target/rpysight.log")
            .map_err(|e| iced::Error::WindowCreationFailed(Box::new(e)))?,
    )
    .map_err(|e| iced::Error::WindowCreationFailed(Box::new(e)))?;
    info!("Logger initialized successfully, starting rPySight from the GUI");
    let cfg = reload_cfg_or_use_default(None);
    let settings = load_app_settings(cfg);
    MainAppGui::run(settings)
}
