use std::time::Duration;

pub static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub static RECEIVED_SIGNAL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x5DEECE66D),
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn gen_u64(&mut self) -> u64 {
        self.next()
    }

    fn gen_f64(&mut self) -> f64 {
        let mantissa_bits = self.next() & ((1u64 << 52) - 1);
        let bits = (0x3FFu64 << 52) | mantissa_bits;
        f64::from_bits(bits) - 1.0
    }
}

pub struct Args {
    pub hours: u32,
    pub threads: usize,
    pub memory_mb: usize,
    pub verbose: bool,
}

impl Args {
    pub fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut hours = 36u32;
        let mut threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let mut memory_mb = 512usize;
        let mut verbose = false;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--hours" => {
                    if let Some(v) = args.next() {
                        hours = v.parse().unwrap_or(36);
                    }
                }
                "--threads" => {
                    if let Some(v) = args.next() {
                        threads = v.parse().unwrap_or(4);
                    }
                }
                "--memory-mb" => {
                    if let Some(v) = args.next() {
                        memory_mb = v.parse().unwrap_or(512);
                    }
                }
                "--verbose" => {
                    verbose = true;
                }
                _ => {}
            }
        }

        Self {
            hours,
            threads,
            memory_mb,
            verbose,
        }
    }
}

pub fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn get_cpu_temperature() -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("sh")
            .args([
                "-c",
                "sysctl -n cpu0.local_temperature 2>/dev/null || echo \"\"",
            ])
            .output()
            .ok()?;

        let temp_str = String::from_utf8_lossy(&output.stdout);
        temp_str.trim().parse::<f32>().ok()
    }

    #[cfg(target_os = "linux")]
    {
        let temp_files = [
            "/sys/class/thermal/thermal_zone0/temp",
            "/sys/class/hwmon/hwmon0/temp1_input",
        ];

        for path in temp_files {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(temp_millidegrees) = content.trim().parse::<i32>() {
                    return Some(temp_millidegrees as f32 / 1000.0);
                }
            }
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

pub fn setup_signal_handler() {
    #[cfg(unix)]
    {
        extern "C" fn handler(sig: libc::c_int) {
            RECEIVED_SIGNAL.store(sig, std::sync::atomic::Ordering::SeqCst);
            RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
            let msg = b"\nReceived signal, shutting down...\n";
            unsafe {
                libc::write(
                    libc::STDERR_FILENO,
                    msg.as_ptr() as *const libc::c_void,
                    msg.len(),
                );
            }
        }

        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handler as *const () as usize;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);

            libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
            libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        }
    }

    #[cfg(windows)]
    {
        println!("Windows signal handling not implemented");
    }
}

pub fn cpu_stress_worker(worker_id: usize, verbose: bool) {
    let mut rng = SimpleRng::new(worker_id as u64 ^ 0x5DEECE66D);
    let mut data = vec![0u64; 1024];
    let mut checksum: u64 = 0;

    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        for v in data.iter_mut() {
            *v = rng.gen_u64();
        }

        for _ in 0..100 {
            let mut a = vec![0.0; 256];
            let mut b = vec![0.0; 256];
            let mut c = vec![0.0; 256];

            for i in 0..256 {
                a[i] = rng.gen_f64() * 1000.0;
                b[i] = rng.gen_f64() * 1000.0;
            }

            for i in 0..256 {
                c[i] = a[i] * b[i] + (a[i].sin() * b[i].cos());
            }

            checksum =
                checksum.wrapping_add(c.iter().fold(0u64, |acc, &x| acc.wrapping_add(x.to_bits())));
        }

        let mut matrix_a = vec![0.0; 64 * 64];
        let mut matrix_b = vec![0.0; 64 * 64];
        let mut matrix_c = vec![0.0; 64 * 64];

        for i in 0..64 * 64 {
            matrix_a[i] = rng.gen_f64();
            matrix_b[i] = rng.gen_f64();
        }

        for i in 0..64 {
            for j in 0..64 {
                let mut sum = 0.0;
                for k in 0..64 {
                    sum += matrix_a[i * 64 + k] * matrix_b[k * 64 + j];
                }
                matrix_c[i * 64 + j] = sum;
            }
        }

        for v in matrix_c.iter() {
            checksum = checksum.wrapping_add(v.to_bits());
        }

        if verbose && worker_id == 0 {
            println!("CPU worker {} checksum: {:016x}", worker_id, checksum);
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn memory_stress_worker(mb: usize, verbose: bool) {
    let size = mb * 1024 * 1024;
    let chunk_size = 1024 * 1024;
    let num_chunks = size / chunk_size;
    let mut rng = SimpleRng::new(0xABCD1234 ^ (mb as u64));
    let mut total_errors = 0u64;

    let mut iteration = 0;
    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        iteration += 1;

        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(num_chunks);

        for chunk_idx in 0..num_chunks {
            if !RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let mut block = vec![0u8; chunk_size];
            let pattern = match chunk_idx % 4 {
                0 => 0xAA,
                1 => 0x55,
                2 => 0xFF,
                _ => 0x00,
            };

            for (i, byte) in block.iter_mut().enumerate() {
                if pattern == 0xAA || pattern == 0x55 {
                    *byte = if (i % 2) == 0 { pattern } else { !pattern };
                } else {
                    *byte = pattern;
                }
            }

            for byte in block.iter_mut() {
                *byte = rng.gen_u64() as u8;
            }

            blocks.push(block);
        }

        for (block_idx, block) in blocks.iter().enumerate() {
            if !RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let mut check: u64 = 0;
            for (i, &byte) in block.iter().enumerate() {
                check = check.wrapping_add((byte as u64).wrapping_mul((i as u64).wrapping_add(1)));
            }

            for (i, &byte) in block.iter().enumerate() {
                let expected = block[i];
                if byte != expected {
                    total_errors += 1;
                    if verbose {
                        println!(
                            "Memory error at block {}, offset {}: expected {:02x}, got {:02x}",
                            block_idx, i, expected, byte
                        );
                    }
                }
            }
        }

        for block in blocks.iter_mut() {
            if !RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            for byte in block.iter_mut() {
                *byte = !*byte;
            }
        }

        for block in blocks.iter_mut() {
            if !RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            for byte in block.iter_mut() {
                *byte = 0;
            }
        }

        blocks.clear();

        if verbose && iteration % 10 == 0 {
            println!(
                "Memory worker: iteration {}, total errors: {}",
                iteration, total_errors
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "00:00:00");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(45), "00:00:45");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(125), "00:02:05");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3661), "01:01:01");
    }

    #[test]
    fn test_format_duration_full_day() {
        assert_eq!(format_duration(86400), "24:00:00");
    }

    #[test]
    fn test_format_duration_large() {
        assert_eq!(format_duration(90061), "25:01:01");
    }

    #[test]
    fn test_args_default_values() {
        let args = Args {
            hours: 36,
            threads: 4,
            memory_mb: 512,
            verbose: false,
        };
        assert_eq!(args.hours, 36);
        assert_eq!(args.memory_mb, 512);
        assert!(!args.verbose);
    }

    #[test]
    fn test_args_custom_values() {
        let args = Args {
            hours: 12,
            threads: 8,
            memory_mb: 1024,
            verbose: true,
        };
        assert_eq!(args.hours, 12);
        assert_eq!(args.threads, 8);
        assert_eq!(args.memory_mb, 1024);
        assert!(args.verbose);
    }

    #[test]
    fn test_received_signal_initial_value() {
        assert_eq!(RECEIVED_SIGNAL.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_received_signal_stores_value() {
        RECEIVED_SIGNAL.store(libc::SIGTERM, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            RECEIVED_SIGNAL.load(std::sync::atomic::Ordering::SeqCst),
            libc::SIGTERM
        );

        RECEIVED_SIGNAL.store(libc::SIGINT, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            RECEIVED_SIGNAL.load(std::sync::atomic::Ordering::SeqCst),
            libc::SIGINT
        );

        RECEIVED_SIGNAL.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn test_running_atomic() {
        RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(RUNNING.load(std::sync::atomic::Ordering::SeqCst));

        RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!RUNNING.load(std::sync::atomic::Ordering::SeqCst));

        // Reset to true for other tests
        RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
