use iced::Application;
use librpysight::{self, gui::MainAppGui, rendering_helpers::{AppConfig, AppConfigBuilder}};
use toml;

#[test]
fn config_ser_deser_returns_identical() {
    let cfg = AppConfigBuilder::default().build();
    let stringified = toml::to_string(&cfg).unwrap();
    let ret: AppConfig = toml::from_str(&stringified).unwrap();
    assert_eq!(ret, cfg);
}

#[test]
fn test_app_to_cfg() {
    let cfg = AppConfigBuilder::default().build();
    let app = MainAppGui::new(cfg.clone());
    let serialized_cfg = AppConfig::from_user_input(&app.0).unwrap();
    assert_eq!(cfg, serialized_cfg);
}
