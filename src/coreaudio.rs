use objc2_core_audio::{
    AudioObjectAddPropertyListener, AudioObjectPropertyAddress, kAudioHardwareNoError,
    kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
};
use std::ptr::NonNull;
use std::sync::Mutex;

static LISTENERS: Mutex<Vec<Box<dyn Fn() + Send>>> = Mutex::new(Vec::new());

unsafe extern "C-unwind" fn on_devices_changed(
    _id: u32,
    _count: u32,
    _addrs: NonNull<AudioObjectPropertyAddress>,
    _data: *mut std::ffi::c_void,
) -> i32 {
    for listener in LISTENERS.lock().unwrap().iter() {
        listener();
    }
    kAudioHardwareNoError
}

pub fn on_devices_change(listener: impl Fn() + Send + 'static) {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let addr = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };
        unsafe {
            AudioObjectAddPropertyListener(
                kAudioObjectSystemObject as u32,
                NonNull::from(&addr),
                Some(on_devices_changed),
                std::ptr::null_mut(),
            );
        }
    });
    LISTENERS.lock().unwrap().push(Box::new(listener));
}
