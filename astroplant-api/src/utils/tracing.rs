use tracing_subscriber::{filter::LevelFilter, EnvFilter};

/// Init tracing with default level of `INFO`.
pub fn init() {
    init_with_default_level(LevelFilter::INFO);
}

fn init_with_default_level(level: LevelFilter) {
    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let _ = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(env_filter)
        .with_test_writer()
        .try_init();
}
