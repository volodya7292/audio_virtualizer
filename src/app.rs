use crate::{
    backend,
    config::{self, AppConfig, EqualizerProfile},
};
use std::io::Cursor;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{self, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
};
use winit::application::ApplicationHandler;

const ICON: &'static [u8] = include_bytes!("../res/icon.png");

pub enum AppUserEvent {
    MenuEvent(tray_icon::menu::MenuEvent),
}

pub struct App {
    _tray_icon: TrayIcon,
    quit_menu_item: MenuItem,
    eq_none_item: CheckMenuItem,
    eq_earpods_item: CheckMenuItem,
    eq_k702_item: CheckMenuItem,
    eq_dt770pro_item: CheckMenuItem,
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

        let tray_menu = Menu::new();
        tray_menu.append(&eq_submenu).unwrap();
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

    pub fn update_from_config(&mut self, config: &AppConfig) {
        self.select_eq_item(config.equalizer_profile);
    }
}

impl ApplicationHandler<AppUserEvent> for App {
    fn resumed(&mut self, _: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::MenuEvent(menu_event) => {
                if menu_event.id() == self.quit_menu_item.id() {
                    event_loop.exit();
                } else if menu_event.id() == self.eq_none_item.id() {
                    self.select_eq_item(EqualizerProfile::None);
                } else if menu_event.id() == self.eq_earpods_item.id() {
                    self.select_eq_item(EqualizerProfile::Earpods);
                } else if menu_event.id() == self.eq_k702_item.id() {
                    self.select_eq_item(EqualizerProfile::K702);
                } else if menu_event.id() == self.eq_dt770pro_item.id() {
                    self.select_eq_item(EqualizerProfile::DT770Pro);
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
