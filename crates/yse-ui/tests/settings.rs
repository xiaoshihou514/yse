use yse_ui::{Application, Settings};

#[test]
fn settings_round_trip() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    // Isolate the test from the real user configuration.
    let config_dir = std::env::temp_dir().join(format!("yse-settings-test-{}", std::process::id()));
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &config_dir) };

    let _app = Application::init();

    let settings = Settings::new("YseTests", "RoundTrip");
    assert!(!settings.contains("name"));
    settings.set("name", "Ada");
    settings.sync();

    // A fresh handle reads the persisted value back.
    let reloaded = Settings::new("YseTests", "RoundTrip");
    assert_eq!(reloaded.value("name").as_deref(), Some("Ada"));

    settings.remove("name");
    assert!(!settings.contains("name"));
    assert_eq!(settings.value("name"), None);
}
