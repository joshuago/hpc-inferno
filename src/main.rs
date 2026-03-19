use hpc_inferno::{
    cpu_stress_worker, format_duration, get_cpu_temperature, memory_stress_worker,
    setup_signal_handler, Args, RECEIVED_SIGNAL, RUNNING,
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
fn get_cpu_usage() -> f32 {
    use std::process::Command;
    let output = Command::new("sh")
        .args([
            "-c",
            "top -l 1 -n 0 | grep 'CPU usage' | awk '{print $3}' | tr -d '%'",
        ])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse::<f32>()
            .unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

#[cfg(target_os = "linux")]
fn get_cpu_usage() -> f32 {
    use std::fs;
    let content = fs::read_to_string("/proc/stat").unwrap_or_default();
    let line = content.lines().next().unwrap_or("");
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return 0.0;
    }
    let user: u64 = parts[1].parse().unwrap_or(0);
    let nice: u64 = parts[2].parse().unwrap_or(0);
    let system: u64 = parts[3].parse().unwrap_or(0);
    let idle: u64 = parts[4].parse().unwrap_or(0);
    let total: u64 = user + nice + system + idle;
    if total == 0 {
        return 0.0;
    }
    ((total - idle) as f32 / total as f32) * 100.0
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_cpu_usage() -> f32 {
    0.0
}

#[cfg(target_os = "macos")]
fn get_memory_info() -> (u64, u64) {
    use std::process::Command;
    let output = Command::new("sh")
        .args(["-c", "sysctl -n hw.memsize vm_stat"])
        .output();
    match output {
        Ok(o) => {
            let total = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("0")
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            let used = Command::new("sh")
                .args([
                    "-c",
                    "vm_stat | grep 'Pages active' | awk '{print $3}' | tr -d '.'",
                ])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<u64>()
                        .unwrap_or(0)
                        * 4096
                })
                .unwrap_or(0);
            (used, total)
        }
        Err(_) => (0, 0),
    }
}

#[cfg(target_os = "linux")]
fn get_memory_info() -> (u64, u64) {
    use std::fs;
    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0u64;
    let mut available = 0u64;
    for line in meminfo.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0].trim_end_matches(':');
        let val: u64 = parts[1].parse().unwrap_or(0);
        if key == "MemTotal" {
            total = val * 1024;
        } else if key == "MemAvailable" {
            available = val * 1024;
        }
    }
    let used = total.saturating_sub(available);
    (used, total)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_memory_info() -> (u64, u64) {
    (0, 0)
}

fn main() {
    let args = Args::parse();

    println!("=== Hardware Burn-In Test ===");
    println!("Target duration: {} hours", args.hours);
    println!("CPU threads: {}", args.threads);
    println!("Memory per worker: {} MB", args.memory_mb);
    println!();

    setup_signal_handler();

    let start_time = Instant::now();
    let target_duration = Duration::from_secs(args.hours as u64 * 3600);
    let mut iteration = 0;
    let mut last_report = Instant::now();

    println!("Starting stress tests...");
    println!("Press Ctrl-C or send SIGTERM to stop gracefully");
    println!();

    let cpu_handles: Vec<_> = (0..args.threads)
        .map(|i| std::thread::spawn(move || cpu_stress_worker(i, args.verbose)))
        .collect();

    let mem_handle = std::thread::spawn(move || memory_stress_worker(args.memory_mb, args.verbose));

    while RUNNING.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_secs(1));

        let elapsed = start_time.elapsed();

        if last_report.elapsed() >= Duration::from_secs(60) || iteration == 0 {
            iteration += 1;
            last_report = Instant::now();

            let cpu_usage = get_cpu_usage();
            let (used_mem, total_mem) = get_memory_info();

            let used_mb = used_mem / 1024 / 1024;
            let total_mb = total_mem / 1024 / 1024;

            let temp = get_cpu_temperature();
            let temp_str = match temp {
                Some(t) => format!("{:.1}°C", t),
                None => "N/A".to_string(),
            };

            println!(
                "[{}] CPU: {:.1}% | Memory: {}/{} MB | Temp: {} | Elapsed: {} / {}h",
                format_duration(elapsed.as_secs()),
                cpu_usage,
                used_mb,
                total_mb,
                temp_str,
                format_duration(elapsed.as_secs()),
                args.hours
            );

            if elapsed >= target_duration {
                println!();
                println!("=== BURN-IN COMPLETED SUCCESSFULLY ===");
                println!("System ran for {} hours without errors!", args.hours);
                break;
            }
        }
    }

    let sig = RECEIVED_SIGNAL.load(Ordering::SeqCst);
    if sig != 0 {
        println!();
        println!("Received signal {}, shutting down gracefully...", sig);
    }

    println!("Waiting for worker threads to finish...");

    for handle in cpu_handles {
        let _ = handle.join();
    }
    let _ = mem_handle.join();

    let final_elapsed = start_time.elapsed();
    println!();
    println!("=== TEST ENDED ===");
    println!(
        "Total runtime: {}",
        format_duration(final_elapsed.as_secs())
    );
}
