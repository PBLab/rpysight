// Remember to $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.
use std::fs::File;

#[macro_use]
extern crate log;

use iced::{Application, Result};

use librpysight::gui::MainAppGui;
use librpysight::{load_app_settings, setup_logger, reload_cfg_or_use_default};

fn main() -> Result {
    setup_logger();
    info!("Logger initialized successfully, starting rPySight from the GUI");
    let cfg = reload_cfg_or_use_default(None);
    let settings = load_app_settings(cfg);
    MainAppGui::run(settings)
}
