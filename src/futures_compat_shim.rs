pub use futures_preview;

#[macro_export]
macro_rules! compat01to03 {
    ($x:expr) => {
        futures_preview::compat::Future01CompatExt::compat($x)
    };
}

#[macro_export]
macro_rules! compat03to01 {
    ($x:expr) => {
        futures_preview::future::TryFutureExt::compat(futures_preview::future::FutureExt::boxed(
            futures_preview::future::FutureExt::unit_error($x),
        ))
    };
}

#[macro_export]
macro_rules! compat01to03executor {
    ($x:expr) => {
        futures_preview::compat::Executor01CompatExt::compat($x)
    };
}

#[macro_export]
macro_rules! compat01to03stream {
    ($x:expr) => {
        futures_preview::compat::Stream01CompatExt::compat($x)
    };
}
