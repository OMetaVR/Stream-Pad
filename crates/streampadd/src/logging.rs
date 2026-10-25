//! stderr logging that reads well both in a terminal and in the journal. `RUST_LOG` overrides the level.

use std::io::Write;

pub fn init() {
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if stderr_is_journal() {
        // journald adds timestamps itself and understands sd-daemon "<N>" priority prefixes.
        builder.format(|buf, record| {
            let priority = match record.level() {
                log::Level::Error => 3,
                log::Level::Warn => 4,
                log::Level::Info => 6,
                log::Level::Debug | log::Level::Trace => 7,
            };
            writeln!(buf, "<{priority}>{}: {}", record.target(), record.args())
        });
    } else {
        builder.format_timestamp_millis();
    }
    builder.init();
}

/// `$JOURNAL_STREAM` ("<device>:<inode>") names stderr's journal stream; it is inherited by children whose
/// stderr may point elsewhere, so compare it with what stderr actually is.
fn stderr_is_journal() -> bool {
    let Some(stream) = std::env::var_os("JOURNAL_STREAM") else { return false };
    let Some((dev, ino)) = stream.to_str().and_then(|s| s.split_once(':')) else { return false };
    let (Ok(dev), Ok(ino)) = (dev.parse::<u64>(), ino.parse::<u64>()) else { return false };
    // SAFETY: fstat on fd 2 into a zeroed, properly sized stat buffer.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(2, &mut st) } != 0 {
        return false;
    }
    st.st_dev == dev && st.st_ino == ino
}
