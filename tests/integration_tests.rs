use hpc_inferno::{format_duration, get_cpu_temperature, Args, RUNNING};
use std::sync::atomic::Ordering;

#[cfg(unix)]
use std::process::Command;

#[test]
fn test_integration_args_parsing() {
    let args = Args {
        hours: 36,
        threads: 4,
        memory_mb: 512,
        verbose: false,
    };
    assert_eq!(args.hours, 36);
    assert_eq!(args.threads, 4);
    assert_eq!(args.memory_mb, 512);
    assert!(!args.verbose);
}

#[test]
fn test_integration_custom_args() {
    let args = Args {
        hours: 6,
        threads: 2,
        memory_mb: 256,
        verbose: true,
    };
    assert_eq!(args.hours, 6);
    assert_eq!(args.threads, 2);
    assert_eq!(args.memory_mb, 256);
    assert!(args.verbose);
}

#[test]
fn test_integration_format_duration() {
    assert_eq!(format_duration(0), "00:00:00");
    assert_eq!(format_duration(59), "00:00:59");
    assert_eq!(format_duration(60), "00:01:00");
    assert_eq!(format_duration(3600), "01:00:00");
    assert_eq!(format_duration(3661), "01:01:01");
    assert_eq!(format_duration(86400), "24:00:00");
}

#[test]
fn test_integration_running_flag() {
    RUNNING.store(false, Ordering::SeqCst);
    assert!(!RUNNING.load(Ordering::SeqCst));

    RUNNING.store(true, Ordering::SeqCst);
    assert!(RUNNING.load(Ordering::SeqCst));
}

#[test]
fn test_integration_get_cpu_temperature() {
    let temp = get_cpu_temperature();
    match temp {
        Some(t) => {
            assert!(
                t > -50.0 && t < 150.0,
                "Temperature {}°C is out of reasonable range",
                t
            );
        }
        None => {
            println!("CPU temperature not available on this system");
        }
    }
}

#[test]
fn test_integration_edge_cases() {
    assert_eq!(format_duration(1), "00:00:01");
    assert_eq!(format_duration(59), "00:00:59");
    assert_eq!(format_duration(60), "00:01:00");
    assert_eq!(format_duration(61), "00:01:01");
    assert_eq!(format_duration(3599), "00:59:59");
    assert_eq!(format_duration(3600), "01:00:00");
}

#[cfg(unix)]
#[test]
fn test_sigterm_causes_graceful_shutdown() {
    use std::thread;
    use std::time::Duration;

    let bin = std::env::var("CARGO_BIN_EXE_hpc-inferno")
        .expect("CARGO_BIN_EXE_hpc-inferno environment variable not set");
    let mut child = Command::new(bin)
        .arg("--hours")
        .arg("1")
        .arg("--memory-mb")
        .arg("16")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start hpc-inferno");

    // Give the process time to set up signal handlers
    thread::sleep(Duration::from_millis(500));

    // Send SIGTERM
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }

    // Process must exit within 10 seconds
    let timeout = Duration::from_secs(10);
    let start = std::time::Instant::now();
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    // Cleanup: kill the hung process
                    let _ = child.kill();
                    panic!(
                        "Process did not exit after SIGTERM within {:?} — signal handler is broken",
                        timeout
                    );
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                let _ = child.kill();
                panic!("Failed to wait on child process: {}", e);
            }
        }
    };

    assert!(
        exit_status.success(),
        "Process exited with signal-derived error: {:?}",
        exit_status
    );
}
