use lazy_static::lazy_static;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
use std::{fs::File, path::PathBuf, sync::Mutex};

lazy_static! {
    static ref APP_CONFIG: Mutex<AppConfig> = Mutex::new(AppConfig {
        equalizer_profile: EqualizerProfile::None,
    });
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub equalizer_profile: EqualizerProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, FromPrimitive, Serialize, Deserialize)]
pub enum EqualizerProfile {
    None,
    Earpods,
    K702,
    DT770Pro,
}

fn get_config_path() -> PathBuf {
    let path = directories::ProjectDirs::from("", "", "audio_virtualizer").unwrap();
    path.config_dir().join("config.json")
}

pub fn load() {
    let config_path = get_config_path();

    let Ok(config_file) = File::open(config_path) else {
        eprintln!("Failed to open config file, using default configuration.");
        return;
    };
    let Ok(config) = serde_json::from_reader::<_, AppConfig>(config_file) else {
        eprintln!("Failed to parse config file, using default configuration.");
        return;
    };

    let mut app_config = APP_CONFIG.lock().unwrap();
    *app_config = config;
}

pub fn get_snapshot() -> AppConfig {
    APP_CONFIG.lock().unwrap().clone()
}

fn save() {
    let config_path = get_config_path();
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let app_config = APP_CONFIG.lock().unwrap();
    let config_file = File::create(config_path).unwrap();
    serde_json::to_writer_pretty(config_file, &*app_config).unwrap();
}

pub fn update<F: FnOnce(&mut AppConfig)>(f: F) {
    let mut config = APP_CONFIG.lock().unwrap();
    f(&mut config);
    drop(config);
    save();
}
