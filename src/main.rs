mod app;
mod audio_buffer_queue;
mod backend;
mod block_convolver;
mod surround_virtualizer;

use crate::app::{App, AppUserEvent};
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::<AppUserEvent>::with_user_event()
        .build()
        .unwrap();

    let proxy = event_loop.create_proxy();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(AppUserEvent::MenuEvent(event));
    }));

    std::thread::spawn(|| {
        backend::run();
    });

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
