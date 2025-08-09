use crate::{
    backend,
    config::{self, AppConfig, EqualizerProfile, AudioSourceMode},
};
use std::collections::HashMap;
use std::io::Cursor;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{self, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};
use winit::application::ApplicationHandler;

const ICON: &'static [u8] = include_bytes!("../res/icon.png");

pub enum AppUserEvent {
    MenuEvent(tray_icon::menu::MenuEvent),
    TrayIconEvent(tray_icon::TrayIconEvent),
}

pub struct App {
    _tray_icon: TrayIcon,
    quit_menu_item: MenuItem,
    eq_none_item: CheckMenuItem,
    eq_earpods_item: CheckMenuItem,
    eq_k702_item: CheckMenuItem,
    eq_dt770pro_item: CheckMenuItem,
    source_universal_item: CheckMenuItem,
    source_stereo_item: CheckMenuItem,
    source_mono_item: CheckMenuItem,
    input_device_submenu: Submenu,
    output_device_submenu: Submenu,
    input_device_items: HashMap<String, CheckMenuItem>,
    output_device_items: HashMap<String, CheckMenuItem>,
}

impl App {
    pub fn new() -> Self {
        let quit_menu_item = menu::MenuItem::new("Quit", true, None);

        let eq_none_item = menu::CheckMenuItem::new("None", true, true, None);
        let eq_earpods_item = menu::CheckMenuItem::new("EarPods", true, false, None);
        let eq_k702_item = menu::CheckMenuItem::new("K702", true, false, None);
        let eq_dt770pro_item = menu::CheckMenuItem::new("DT 770 Pro", true, false, None);

        let eq_submenu = menu::Submenu::new("Equalizer Profile", true);
        eq_submenu.append(&eq_none_item).unwrap();
        eq_submenu.append(&eq_earpods_item).unwrap();
        eq_submenu.append(&eq_k702_item).unwrap();
        eq_submenu.append(&eq_dt770pro_item).unwrap();

        let source_universal_item = menu::CheckMenuItem::new("Universal", true, true, None);
        let source_stereo_item = menu::CheckMenuItem::new("Stereo", true, false, None);
        let source_mono_item = menu::CheckMenuItem::new("Mono", true, false, None);

        let source_submenu = menu::Submenu::new("Audio Source Mode", true);
        source_submenu.append(&source_universal_item).unwrap();
        source_submenu.append(&source_stereo_item).unwrap();
        source_submenu.append(&source_mono_item).unwrap();

        let input_device_submenu = menu::Submenu::new("Surround Audio Source", true);
        let output_device_submenu = menu::Submenu::new("Stereo Output Device", true);

        let tray_menu = Menu::new();
        tray_menu.append(&eq_submenu).unwrap();
        tray_menu.append(&source_submenu).unwrap();
        tray_menu.append(&PredefinedMenuItem::separator()).unwrap();
        tray_menu.append(&input_device_submenu).unwrap();
        tray_menu.append(&output_device_submenu).unwrap();
        tray_menu.append(&PredefinedMenuItem::separator()).unwrap();
        tray_menu.append(&quit_menu_item).unwrap();

        let mut icon_reader = png::Decoder::new(Cursor::new(ICON)).read_info().unwrap();
        let mut icon_buf = vec![0; icon_reader.output_buffer_size()];
        icon_reader.next_frame(&mut icon_buf).unwrap();
        icon_reader.finish().unwrap();

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Audio Virtualizer")
            .with_menu(Box::new(tray_menu))
            .with_icon(
                Icon::from_rgba(
                    icon_buf,
                    icon_reader.info().width,
                    icon_reader.info().height,
                )
                .unwrap(),
            )
            .build()
            .unwrap();

        Self {
            _tray_icon: tray_icon,
            quit_menu_item,
            eq_none_item,
            eq_earpods_item,
            eq_k702_item,
            eq_dt770pro_item,
            source_universal_item,
            source_stereo_item,
            source_mono_item,
            input_device_submenu,
            output_device_submenu,
            input_device_items: HashMap::new(),
            output_device_items: HashMap::new(),
        }
    }

    fn select_eq_item(&mut self, profile: EqualizerProfile) {
        self.eq_none_item.set_checked(false);
        self.eq_earpods_item.set_checked(false);
        self.eq_k702_item.set_checked(false);
        self.eq_dt770pro_item.set_checked(false);
        match profile {
            EqualizerProfile::None => self.eq_none_item.set_checked(true),
            EqualizerProfile::Earpods => self.eq_earpods_item.set_checked(true),
            EqualizerProfile::K702 => self.eq_k702_item.set_checked(true),
            EqualizerProfile::DT770Pro => self.eq_dt770pro_item.set_checked(true),
        }
        backend::set_equalizer_profile(profile);
        config::update(|cfg| {
            cfg.equalizer_profile = profile;
        });
    }

    fn select_source_mode(&mut self, mode: AudioSourceMode) {
        self.source_universal_item.set_checked(false);
        self.source_stereo_item.set_checked(false);
        self.source_mono_item.set_checked(false);
        match mode {
            AudioSourceMode::Universal => self.source_universal_item.set_checked(true),
            AudioSourceMode::Stereo => self.source_stereo_item.set_checked(true),
            AudioSourceMode::Mono => self.source_mono_item.set_checked(true),
        }
        config::update(|cfg| {
            cfg.audio_source_mode = mode;
        });
        backend::reload_backend();
    }

    fn refresh_audio_device_lists(&mut self, config: &AppConfig) {
        for item in self.input_device_items.values() {
            self.input_device_submenu.remove(item).unwrap_or_default();
        }
        for item in self.output_device_items.values() {
            self.output_device_submenu.remove(item).unwrap_or_default();
        }

        self.input_device_items.clear();
        self.output_device_items.clear();

        let input_devices = backend::get_input_devices();
        let selected_input_def = config
            .input_device_name
            .as_deref()
            .unwrap_or(backend::DEFAULT_INPUT_DEVICE_NAME);

        for device_name in input_devices {
            let is_selected = device_name == selected_input_def;
            let item = menu::CheckMenuItem::new(&device_name, true, is_selected, None);
            self.input_device_submenu.append(&item).unwrap();
            self.input_device_items.insert(device_name, item);
        }

        let output_devices = backend::get_output_devices();
        let selected_output_def = config
            .output_device_name
            .as_deref()
            .unwrap_or(backend::DEFAULT_OUTPUT_DEVICE_NAME);

        for device_name in output_devices {
            let is_selected = device_name == selected_output_def;
            let item = menu::CheckMenuItem::new(&device_name, true, is_selected, None);
            self.output_device_submenu.append(&item).unwrap();
            self.output_device_items.insert(device_name, item);
        }
    }

    fn select_input_device(&mut self, device_name: &str) {
        for item in self.input_device_items.values() {
            item.set_checked(false);
        }
        if let Some(item) = self.input_device_items.get(device_name) {
            item.set_checked(true);
        }

        config::update(|cfg| {
            cfg.input_device_name = Some(device_name.to_string());
        });
        backend::reload_backend();
    }

    fn select_output_device(&mut self, device_name: &str) {
        for item in self.output_device_items.values() {
            item.set_checked(false);
        }
        if let Some(item) = self.output_device_items.get(device_name) {
            item.set_checked(true);
        }

        config::update(|cfg| {
            cfg.output_device_name = Some(device_name.to_string());
        });
        backend::reload_backend();
    }

    pub fn update_from_config(&mut self, config: &AppConfig) {
        self.refresh_audio_device_lists(config);
        self.select_eq_item(config.equalizer_profile);
        self.select_source_mode(config.audio_source_mode);
        self.select_input_device(
            config
                .input_device_name
                .as_deref()
                .unwrap_or(backend::DEFAULT_INPUT_DEVICE_NAME),
        );
        self.select_output_device(
            config
                .output_device_name
                .as_deref()
                .unwrap_or(backend::DEFAULT_OUTPUT_DEVICE_NAME),
        );
    }
}

impl ApplicationHandler<AppUserEvent> for App {
    fn resumed(&mut self, _: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::MenuEvent(menu_event) => {
                let menu_id = menu_event.id();

                if menu_id == self.quit_menu_item.id() {
                    event_loop.exit();
                } else if menu_id == self.eq_none_item.id() {
                    self.select_eq_item(EqualizerProfile::None);
                } else if menu_id == self.eq_earpods_item.id() {
                    self.select_eq_item(EqualizerProfile::Earpods);
                } else if menu_id == self.eq_k702_item.id() {
                    self.select_eq_item(EqualizerProfile::K702);
                } else if menu_id == self.eq_dt770pro_item.id() {
                    self.select_eq_item(EqualizerProfile::DT770Pro);
                } else if menu_id == self.source_universal_item.id() {
                    self.select_source_mode(AudioSourceMode::Universal);
                } else if menu_id == self.source_stereo_item.id() {
                    self.select_source_mode(AudioSourceMode::Stereo);
                } else if menu_id == self.source_mono_item.id() {
                    self.select_source_mode(AudioSourceMode::Mono);
                } else {
                    let mut selected_input_device = None;
                    for (device_name, item) in &self.input_device_items {
                        if menu_id == item.id() {
                            selected_input_device = Some(device_name.clone());
                            break;
                        }
                    }
                    if let Some(device_name) = selected_input_device {
                        self.select_input_device(&device_name);
                        return;
                    }

                    let mut selected_output_device = None;
                    for (device_name, item) in &self.output_device_items {
                        if menu_id == item.id() {
                            selected_output_device = Some(device_name.clone());
                            break;
                        }
                    }
                    if let Some(device_name) = selected_output_device {
                        self.select_output_device(&device_name);
                    }
                }
            }
            AppUserEvent::TrayIconEvent(tray_icon_event) => {
                if let TrayIconEvent::Click { .. } = tray_icon_event {
                    let config = config::get_snapshot();
                    self.refresh_audio_device_lists(&config);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let winit::event::WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}
