use std::sync::atomic::{AtomicU32, Ordering};

use objc2_core_motion::CMHeadphoneMotionManager;

static HEAD_YAW: AtomicU32 = AtomicU32::new(0);
static HEAD_PITCH: AtomicU32 = AtomicU32::new(0);

pub fn get_head_yaw() -> f32 {
    f32::from_bits(HEAD_YAW.load(Ordering::Relaxed))
}

pub fn get_head_pitch() -> f32 {
    f32::from_bits(HEAD_PITCH.load(Ordering::Relaxed))
}

pub unsafe fn start_head_tracking() {
    unsafe {
        let manager = CMHeadphoneMotionManager::new();
        if manager.isDeviceMotionAvailable() {
            println!("Device motion available");
        } else {
            println!("Device motion NOT available");
            return;
        }

        manager.startDeviceMotionUpdates();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(1));
            if let Some(motion) = manager.deviceMotion() {
                let yaw = motion.attitude().yaw() as f32;
                let pitch = motion.attitude().pitch() as f32;
                HEAD_YAW.store(yaw.to_bits(), Ordering::Relaxed);
                HEAD_PITCH.store(pitch.to_bits(), Ordering::Relaxed);
            }
        }
    }
}
