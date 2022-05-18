#![allow(unused_macros)]
#![allow(unused_imports)]

macro_rules! error {
    ($($arg:tt)+) => (
        #[cfg(feature = "log")]
        ex_log::error!($($arg)+)
    )
}

macro_rules! warning {
    ($($arg:tt)+) => (
        #[cfg(feature = "log")]
        ex_log::warn!($($arg)+)
    )
}

macro_rules! info {
    ($($arg:tt)+) => (
        #[cfg(feature = "log")]
        ex_log::info!($($arg)+)
    )
}

macro_rules! debug {
    ($($arg:tt)+) => (
        #[cfg(feature = "log")]
        ex_log::debug!($($arg)+)
    )
}

macro_rules! trace {
    ($($arg:tt)+) => (
        #[cfg(feature = "log")]
        ex_log::trace!($($arg)+)
    )
}

pub(crate) use debug;
pub(crate) use error;
pub(crate) use info;
pub(crate) use trace;
pub(crate) use warning;
