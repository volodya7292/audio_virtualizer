mod app;
mod audio_data;
mod audio_swapchain;
mod backend;
mod block_convolver;
mod config;
mod surround_virtualizer;
mod macros;

use crate::app::{App, AppUserEvent};
use crate::config::get_cache_path;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use log::error;
use winit::event_loop::EventLoop;

fn setup_logging() {
    let cache_dir = get_cache_path();
    let _ = std::fs::create_dir_all(&cache_dir).ok();

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .directory(cache_dir)
                .basename("audio_virtualizer"),
        )
        .format(flexi_logger::detailed_format)
        .rotate(
            Criterion::Size(1_000_000),
            Naming::Numbers,
            Cleanup::KeepLogFiles(3),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()
        .unwrap();

    log_panics::init();
}

fn main() {
    setup_logging();
    config::load();

    let mut event_loop_builder = EventLoop::<AppUserEvent>::with_user_event();

    #[cfg(target_os = "macos")]
    {
        // hide the app from the dock
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        event_loop_builder
            .with_activation_policy(winit::platform::macos::ActivationPolicy::Prohibited);
    }

    let event_loop = event_loop_builder.build().unwrap();

    let ev_proxy = event_loop.create_proxy();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        if let Err(e) = ev_proxy.send_event(AppUserEvent::MenuEvent(event)) {
            error!("Failed to send menu event: {}", e);
        }
    }));

    let ev_proxy = event_loop.create_proxy();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        if let Err(e) = ev_proxy.send_event(AppUserEvent::TrayIconEvent(event)) {
            error!("Failed to send tray icon event: {}", e);
        }
    }));

    let mut app = App::new();
    app.update_from_config(&config::get_snapshot());

    std::thread::spawn(|| {
        backend::run();
    });

    event_loop.run_app(&mut app).unwrap();
}
