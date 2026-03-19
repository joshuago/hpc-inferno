# hpc-inferno - Hardware Burn-In Tool

A system burn-in program written in Rust that stress tests CPU and memory to
verify hardware stability. Designed to run continuously until SIGTERM is caught
(e.g., Ctrl-C).

## Purpose

This tool is designed for validating hardware quality, particularly CPU and
memory. A hardware configuration is considered to have passed if the system can
run for 36 hours without any errors.

## Features

- **CPU Stress Testing**: Multi-threaded workloads using matrix multiplication,
  floating-point operations, and checksum calculations
- **Memory Stress Testing**: Allocates memory, writes various patterns (0xAA,
  0x55, 0xFF, random), verifies data integrity, and performs bit-flip tests
- **Temperature Monitoring**: Attempts to read CPU temperature
  (platform-dependent)
- **Progress Reporting**: Displays CPU usage, memory usage, temperature, and
  elapsed time every 60 seconds

## Building

```bash
cd hpc-inferno
cargo build --release
```

## Usage

```bash
./target/release/hpc-inferno [OPTIONS]
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `--hours HOURS` | Test duration in hours | 36 |
| `--threads N` | Number of CPU worker threads | 4 |
| `--memory-mb MB` | Memory per worker in MB | 512 |
| `--verbose` | Enable verbose output | false |

### Examples

Run default 36-hour test with 4 threads and 512MB per worker:
```bash
./target/release/hpc-inferno
```

Quick 1-hour test:
```bash
./target/release/hpc-inferno --hours 1
```

Aggressive testing with more threads and memory:
```bash
./target/release/hpc-inferno --threads 8 --memory-mb 1024
```

Verbose mode for detailed diagnostics:
```bash
./target/release/hpc-inferno --verbose
```

## Output Example

```
=== Hardware Burn-In Test ===
Target duration: 36 hours
CPU threads: 4
Memory per worker: 512 MB

Starting stress tests...
Press Ctrl-C or send SIGTERM to stop gracefully

[00:01:00] CPU: 95.2% | Memory: 4096/16384 MB | Temp: 72.5°C | Elapsed: 00:01:00 / 36h
[00:02:00] CPU: 97.1% | Memory: 4096/16384 MB | Temp: 74.2°C | Elapsed: 00:02:00 / 36h
...

=== BURN-IN COMPLETED SUCCESSFULLY ===
System ran for 36 hours without errors!
```

## Temperature Monitoring

Temperature reading is platform-dependent:

- **macOS**: Attempts `sysctl -n cpu0.local_temperature` (may require specific
  hardware support)
- **Linux**: Reads from `/sys/class/thermal/thermal_zone0/temp` or
  `/sys/class/hwmon/hwmon0/temp1_input`

If temperature cannot be read, it will display as "N/A".

## Exit Codes

- `0`: Test completed successfully (reached target duration)
- `1`: Test interrupted or failed

## Requirements

- Rust 2024 edition
- Supported platforms: macOS, Linux

## License

MIT
