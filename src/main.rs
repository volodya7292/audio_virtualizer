mod app;
mod audio_swapchain;
mod backend;
mod block_convolver;
mod config;
mod surround_virtualizer;

use crate::app::{App, AppUserEvent};
use winit::event_loop::EventLoop;

fn main() {
    config::load();

    let mut event_loop_builder = EventLoop::<AppUserEvent>::with_user_event();

    #[cfg(target_os = "macos")]
    {
        // hide the app from the dock
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        event_loop_builder
            .with_activation_policy(winit::platform::macos::ActivationPolicy::Accessory);
    }

    let event_loop = event_loop_builder.build().unwrap();

    let ev_proxy = event_loop.create_proxy();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        if let Err(e) = ev_proxy.send_event(AppUserEvent::MenuEvent(event)) {
            eprintln!("Failed to send menu event: {}", e);
        }
    }));

    let ev_proxy = event_loop.create_proxy();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        if let Err(e) = ev_proxy.send_event(AppUserEvent::TrayIconEvent(event)) {
            eprintln!("Failed to send tray icon event: {}", e);
        }
    }));

    let mut app = App::new();
    app.update_from_config(&config::get_snapshot());

    std::thread::spawn(|| {
        backend::run();
    });

    event_loop.run_app(&mut app).unwrap();
}
