use std::io::Cursor;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{self, Menu, MenuItem},
};
use winit::application::ApplicationHandler;

const ICON: &'static [u8] = include_bytes!("../res/icon.png");

pub enum AppUserEvent {
    MenuEvent(tray_icon::menu::MenuEvent),
}

pub struct App {
    _tray_icon: TrayIcon,
    quit_menu_item: MenuItem,
}

impl App {
    pub fn new() -> Self {
        let quit_menu_item = menu::MenuItem::new("Quit", true, None);

        let tray_menu = Menu::new();
        tray_menu.append(&quit_menu_item).unwrap();

        let mut icon_reader = png::Decoder::new(Cursor::new(ICON)).read_info().unwrap();
        let mut icon_buf = vec![0; icon_reader.output_buffer_size()];
        icon_reader.next_frame(&mut icon_buf).unwrap();
        icon_reader.finish().unwrap();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("system-tray - tray icon library!")
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
        }
    }
}

impl ApplicationHandler<AppUserEvent> for App {
    fn resumed(&mut self, _: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::MenuEvent(menu_event) => {
                if menu_event.id() == self.quit_menu_item.id() {
                    event_loop.exit();
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
