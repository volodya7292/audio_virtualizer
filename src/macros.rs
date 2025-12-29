/// Execute a block if the per-callsite cooldown has elapsed.
#[macro_export]
macro_rules! execute_sampled {
    ($dur:expr, $block:block) => {{
        static __LAST_INVOCATION_TIME: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);

        let __now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let __last = __LAST_INVOCATION_TIME.load(std::sync::atomic::Ordering::Relaxed);

        if __now.saturating_sub(__last) >= $dur.as_millis() as u64 {
            __LAST_INVOCATION_TIME.store(__now, std::sync::atomic::Ordering::Relaxed);
            $block
        }
    }};
}
