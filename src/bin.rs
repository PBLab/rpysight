// Remember to  $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.
use iced::{Settings, Result, Application};

use librpysight::gui::ConfigGui;

fn main() -> Result {
    ConfigGui::run(Settings::default())
}
