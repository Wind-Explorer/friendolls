//! Compatibility shim for macOS Screenshot's screencapture toolbar process.
//! `-U` is documented by screencapture(1); `keyboard.interactive` is an
//! implementation detail observed in the system keyboard shortcut invocation.
//! This follows process lifetime, including any recording started by the toolbar.
use std::{io, mem::size_of};

pub(super) const ENABLED: bool = true;

// sys/proc_info.h, included by libproc.h; not currently exposed by libc.
const PROC_ALL_PIDS: u32 = 1;

pub(super) fn is_active() -> io::Result<bool> {
    // proc_listpids returns bytes, not a PID count. Retry a full buffer because
    // the process table can grow between the sizing and filling calls.
    let bytes = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Err(io::Error::last_os_error());
    }
    let mut pids = vec![0i32; bytes as usize / size_of::<i32>() + 64];
    loop {
        let capacity = i32::try_from(pids.len() * size_of::<i32>())
            .map_err(|_| io::Error::other("process table too large"))?;
        // SAFETY: pids is an aligned, initialized buffer of capacity bytes.
        let bytes =
            unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), capacity) };
        if bytes < 0 {
            return Err(io::Error::last_os_error());
        }
        if bytes < capacity {
            pids.truncate(bytes as usize / size_of::<i32>());
            break;
        }
        pids.resize(pids.len() * 2, 0);
    }

    for pid in pids.into_iter().filter(|pid| *pid > 0) {
        let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: path is writable for the advertised size. Processes which
        // exit or are inaccessible between these calls are retried next scan.
        let length =
            unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
        if length <= 0 || path.split(|byte| *byte == 0).next() != Some(b"/usr/sbin/screencapture") {
            continue;
        }
        if let Ok(arguments) = process_arguments(pid)
            && toolbar_arguments(&arguments)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn process_arguments(pid: i32) -> io::Result<Vec<u8>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut length = 0;
    // SAFETY: sysctl writes only the size when oldp is null; mib has three ints.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0u8; length];
    // SAFETY: buffer has the size requested by sysctl. No new value is supplied.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(length);
    Ok(buffer)
}

fn toolbar_arguments(buffer: &[u8]) -> bool {
    let Some(count) = buffer
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_ne_bytes)
    else {
        return false;
    };
    if count <= 0 {
        return false;
    }
    // KERN_PROCARGS2: argc, executable path, NUL padding, argv, environment.
    let rest = &buffer[4..];
    let Some(path_end) = rest.iter().position(|byte| *byte == 0) else {
        return false;
    };
    let rest = &rest[path_end..];
    let Some(start) = rest.iter().position(|byte| *byte != 0) else {
        return false;
    };
    let mut rest = &rest[start..];
    let mut toolbar = false;
    for index in 0..count {
        let Some(end) = rest.iter().position(|byte| *byte == 0) else {
            return false;
        };
        let arg = &rest[..end];
        rest = &rest[end + 1..];
        if index == 0 {
            continue;
        }
        if arg == b"--" {
            break;
        }
        toolbar |= arg == b"keyboard.interactive"
            || (arg.starts_with(b"-") && !arg.starts_with(b"--") && arg[1..].contains(&b'U'));
    }
    toolbar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(args: &[&str]) -> Vec<u8> {
        let mut buffer = (args.len() as i32).to_ne_bytes().to_vec();
        buffer.extend_from_slice(b"/usr/sbin/screencapture\0\0");
        for arg in args {
            buffer.extend_from_slice(arg.as_bytes());
            buffer.push(0);
        }
        buffer
    }

    #[test]
    fn distinguishes_toolbar_from_quick_capture() {
        for flags in ["-pdiU", "-U"] {
            assert!(toolbar_arguments(&arguments(&["screencapture", flags])));
        }
        assert!(toolbar_arguments(&arguments(&[
            "screencapture",
            "-z",
            "keyboard.interactive"
        ])));
        for mode in ["keyboard.selection", "keyboard.screen"] {
            assert!(!toolbar_arguments(&arguments(&[
                "screencapture",
                "-pdi",
                "-z",
                mode
            ])));
        }
        assert!(!toolbar_arguments(&arguments(&["screencapture", "-u"])));
        assert!(!toolbar_arguments(&arguments(&[
            "screencapture",
            "--",
            "-U"
        ])));
    }

    #[test]
    fn excludes_environment_and_malformed_arguments() {
        let mut buffer = arguments(&["screencapture", "-pd"]);
        buffer.extend_from_slice(b"keyboard.interactive\0-U\0");
        assert!(!toolbar_arguments(&buffer));
        assert!(!toolbar_arguments(&[0, 1]));
        let mut buffer = arguments(&["screencapture", "-U"]);
        buffer.pop();
        assert!(!toolbar_arguments(&buffer));
    }

    #[test]
    fn scans_native_process_table() {
        is_active().unwrap();
    }

    #[test]
    fn reads_native_process_arguments_without_capture_permissions() {
        let buffer = process_arguments(std::process::id() as i32).unwrap();
        assert!(!toolbar_arguments(&buffer));
    }
}
