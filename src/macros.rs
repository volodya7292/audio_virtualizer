pub(crate) static GLOBAL_START_INSTANT: std::sync::OnceLock<std::time::Instant> =
    std::sync::OnceLock::new();

pub fn now_monotonic_millis() -> u64 {
    let global_start = *GLOBAL_START_INSTANT.get_or_init(std::time::Instant::now);
    std::time::Instant::now()
        .duration_since(global_start)
        .as_millis() as u64
}

/// Execute a block if the per-callsite cooldown has elapsed.
#[macro_export]
macro_rules! execute_sampled {
    ($dur:expr, $block:block) => {{
        static __LAST_INVOCATION_TIME: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);

        let __now = crate::macros::now_monotonic_millis();
        let __last = __LAST_INVOCATION_TIME.load(std::sync::atomic::Ordering::Relaxed);

        if __now.saturating_sub(__last) >= $dur.as_millis() as u64 {
            __LAST_INVOCATION_TIME.store(__now, std::sync::atomic::Ordering::Relaxed);
            $block
        }
    }};
}
