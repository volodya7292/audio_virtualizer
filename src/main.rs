mod app;
mod audio_buffer_queue;
mod backend;
mod block_convolver;
mod surround_virtualizer;

use crate::app::{App, AppUserEvent};
use winit::event_loop::EventLoop;

fn main() {
    let mut event_loop_builder = EventLoop::<AppUserEvent>::with_user_event();

    #[cfg(target_os = "macos")]
    {
        // hide the app from the dock
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        event_loop_builder
            .with_activation_policy(winit::platform::macos::ActivationPolicy::Accessory);
    }

    let event_loop = event_loop_builder.build().unwrap();

    let proxy = event_loop.create_proxy();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        if let Err(e) = proxy.send_event(AppUserEvent::MenuEvent(event)) {
            eprintln!("Failed to send menu event: {}", e);
        }
    }));

    std::thread::spawn(|| {
        backend::run();
    });

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
