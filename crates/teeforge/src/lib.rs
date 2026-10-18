mod atomic_file;
mod blhide;
mod cli;
mod config;
mod description;
mod error;
mod info;
mod json;
mod keybox;
mod keybox_format;
mod logging;
mod omk;
mod process;
mod rkp;
mod rootdetect;
mod target;
mod update;
mod volume;

pub use cli::run;

pub(crate) const VERSION: &str = match option_env!("TEEFORGE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
