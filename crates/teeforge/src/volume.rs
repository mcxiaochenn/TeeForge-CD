use crate::error::{Result, TfError};
#[cfg(target_os = "android")]
use std::ffi::CString;
#[cfg(target_os = "android")]
use std::fs;
#[cfg(target_os = "android")]
use std::mem::MaybeUninit;
#[cfg(target_os = "android")]
use std::os::fd::RawFd;
#[cfg(target_os = "android")]
use std::time::{Duration, Instant};

#[cfg(target_os = "android")]
#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    event_type: u16,
    code: u16,
    value: i32,
}

#[cfg(target_os = "android")]
const EV_KEY: u16 = 1;
#[cfg(target_os = "android")]
const KEY_VOLUMEUP: u16 = 115;
#[cfg(target_os = "android")]
const KEY_VOLUMEDOWN: u16 = 114;

#[cfg(target_os = "android")]
fn open_events() -> Result<Vec<RawFd>> {
    let mut entries = fs::read_dir("/dev/input")?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("event"))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut fds = Vec::new();
    for entry in entries {
        let path = entry.path();
        let Ok(c_path) = CString::new(path.as_os_str().as_encoded_bytes()) else {
            continue;
        };
        // SAFETY: c_path is NUL-terminated and the returned descriptor is owned here.
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd >= 0 {
            fds.push(fd);
        }
    }
    if fds.is_empty() {
        Err(TfError::new("无法打开输入设备 [Cannot open input devices]"))
    } else {
        Ok(fds)
    }
}

#[cfg(target_os = "android")]
pub(crate) fn listen(timeout_seconds: u64) -> Result<i32> {
    let fds = open_events()?;
    let mut poll_fds = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let result = 'waiting: loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Ok(-1);
        }
        let timeout = remaining.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: poll_fds points to initialized pollfd values for the supplied length.
        let ready = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, timeout) };
        if ready < 0 {
            break Err(TfError::from(std::io::Error::last_os_error()));
        }
        for poll_fd in &poll_fds {
            if poll_fd.revents & libc::POLLIN == 0 {
                continue;
            }
            let mut event = MaybeUninit::<InputEvent>::uninit();
            // SAFETY: event points to writable storage of exactly InputEvent size.
            let read = unsafe {
                libc::read(
                    poll_fd.fd,
                    event.as_mut_ptr().cast(),
                    std::mem::size_of::<InputEvent>(),
                )
            };
            if read as usize != std::mem::size_of::<InputEvent>() {
                continue;
            }
            // SAFETY: read initialized every byte of event after the exact-size check.
            let event = unsafe { event.assume_init() };
            if event.event_type == EV_KEY && event.value == 1 {
                if event.code == KEY_VOLUMEUP {
                    break 'waiting Ok(1);
                }
                if event.code == KEY_VOLUMEDOWN {
                    break 'waiting Ok(0);
                }
            }
        }
    };
    for fd in fds {
        // SAFETY: every descriptor was returned by open_events and is closed exactly once.
        unsafe { libc::close(fd) };
    }
    result
}

#[cfg(not(target_os = "android"))]
pub(crate) fn listen(_timeout_seconds: u64) -> Result<i32> {
    Err(TfError::new(
        "音量监听仅支持 Android [Volume listening is Android-only]",
    ))
}
