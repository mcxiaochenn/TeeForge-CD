mod atomic_file;
mod blhide;
mod cli;
mod config;
mod description;
mod error;
mod keybox;
mod logging;
mod process;
mod rootdetect;
mod target;
mod volume;

pub use cli::run;

pub(crate) const VERSION: &str = match option_env!("TEEFORGE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
