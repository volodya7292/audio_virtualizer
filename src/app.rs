use crate::{
    backend,
    config::{self, AppConfig, AudioSourceMode, EqualizerProfile},
};
use std::collections::HashMap;
use std::io::Cursor;
use strum::IntoEnumIterator;
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
    eq_items: Vec<(EqualizerProfile, CheckMenuItem)>,
    source_items: Vec<(AudioSourceMode, CheckMenuItem)>,
    input_device_submenu: Submenu,
    output_device_submenu: Submenu,
    input_device_items: HashMap<String, CheckMenuItem>,
    output_device_items: HashMap<String, CheckMenuItem>,
}

impl App {
    pub fn new() -> Self {
        let quit_menu_item = menu::MenuItem::new("Quit", true, None);

        let mut eq_items = Vec::new();
        let eq_submenu = menu::Submenu::new("Equalizer Profile", true);
        for profile in EqualizerProfile::iter() {
            let checked = profile == EqualizerProfile::None;
            let item = menu::CheckMenuItem::new(profile.label(), true, checked, None);
            eq_submenu.append(&item).unwrap();
            eq_items.push((profile, item));
        }

        let mut source_items = Vec::new();
        let source_submenu = menu::Submenu::new("Audio Source Mode", true);
        for source in AudioSourceMode::iter() {
            let checked = source == AudioSourceMode::Universal;
            let label: &str = source.into();
            let item = menu::CheckMenuItem::new(label, true, checked, None);
            source_submenu.append(&item).unwrap();
            source_items.push((source, item));
        }

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
            eq_items,
            source_items,
            input_device_submenu,
            output_device_submenu,
            input_device_items: HashMap::new(),
            output_device_items: HashMap::new(),
        }
    }

    fn select_eq_item(&mut self, profile: EqualizerProfile) {
        for (p, item) in &self.eq_items {
            item.set_checked(*p == profile);
        }
        backend::set_equalizer_profile(profile);
        config::update(|cfg| {
            cfg.equalizer_profile = profile;
        });
    }

    fn select_source_mode(&mut self, mode: AudioSourceMode) {
        for (s, item) in &self.source_items {
            item.set_checked(*s == mode);
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
        for (name, item) in &mut self.input_device_items {
            item.set_checked(name == device_name);
        }
        config::update(|cfg| {
            cfg.input_device_name = Some(device_name.to_string());
        });
        backend::reload_backend();
    }

    fn select_output_device(&mut self, device_name: &str) {
        for (name, item) in &mut self.output_device_items {
            item.set_checked(name == device_name);
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
                } else if let Some((profile, _)) =
                    self.eq_items.iter().find(|(_, item)| item.id() == menu_id)
                {
                    self.select_eq_item(*profile);
                } else if let Some((source, _)) = self
                    .source_items
                    .iter()
                    .find(|(_, item)| item.id() == menu_id)
                {
                    self.select_source_mode(*source);
                } else if let Some((device_name, _)) = self
                    .input_device_items
                    .iter()
                    .find(|(_, item)| item.id() == menu_id)
                {
                    self.select_input_device(&device_name.clone());
                } else if let Some((device_name, _)) = self
                    .output_device_items
                    .iter()
                    .find(|(_, item)| item.id() == menu_id)
                {
                    self.select_output_device(&device_name.clone());
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
