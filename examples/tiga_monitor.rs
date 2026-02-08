//! Real-time system monitor for Zoom75 TIGA display.
//!
//! Reads CPU/GPU temperature from sysinfo (PMU die sensors), fan speed from
//! macOS SMC, and network throughput from sysinfo. Pushes updates to the TIGA
//! display via HID and optionally cycles through display screens.
//!
//! On exit (Ctrl+C), navigates back to home and re-syncs time + weather so the
//! display doesn't freeze.
//!
//! Usage:
//!   cargo run --example tiga_monitor -- [options]
//!
//! Options:
//!   --interval <SEC>           Sensor send interval in seconds (default: 2)
//!   --cycle <SEC>              Screen cycle interval in seconds (default: 3)
//!   --no-cycle                 Disable automatic screen cycling
//!   --no-weather               Disable weather sync
//!   --weather-interval <MIN>   Weather refresh interval in minutes (default: 30)
//!   --verbose / -v             Print sensor values to terminal each tick

use std::env;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Timelike};
use hidapi::HidDevice;
use macsmc::Smc;
use sysinfo::{Components, Networks};
use zoom75_tiga::consts::*;
use zoom75_tiga::protocol::build_packet;
use zoom75_tiga::types::{WeatherIcon, encode_temperature};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

struct Args {
    interval_secs: u64,
    cycle_secs: u64,
    no_cycle: bool,
    no_weather: bool,
    weather_interval_mins: u64,
    verbose: bool,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();
    let mut a = Args {
        interval_secs: 2,
        cycle_secs: 3,
        no_cycle: false,
        no_weather: false,
        weather_interval_mins: 30,
        verbose: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--interval" => {
                i += 1;
                a.interval_secs = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(2);
            }
            "--cycle" => {
                i += 1;
                a.cycle_secs = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(5);
            }
            "--no-cycle" => a.no_cycle = true,
            "--no-weather" => a.no_weather = true,
            "--weather-interval" => {
                i += 1;
                a.weather_interval_mins = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(30);
            }
            "--verbose" | "-v" => a.verbose = true,
            "--help" | "-h" => {
                println!("tiga_monitor — real-time system monitor for Zoom75 TIGA");
                println!();
                println!("Options:");
                println!("  --interval <SEC>           Sensor send interval (default: 2)");
                println!("  --cycle <SEC>              Screen cycle interval (default: 3)");
                println!("  --no-cycle                 Disable automatic screen cycling");
                println!("  --no-weather               Disable weather sync");
                println!("  --weather-interval <MIN>   Weather refresh interval (default: 30)");
                println!("  --verbose / -v             Print values to terminal");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown option: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    a
}

// ---------------------------------------------------------------------------
// Cached weather data
// ---------------------------------------------------------------------------

struct CachedWeather {
    wmo: u8,
    is_day: bool,
    current: f32,
    min: f32,
    max: f32,
}

/// Fetch weather using ipinfo geolocation + open-meteo API (blocking).
fn fetch_weather() -> Option<CachedWeather> {
    let rt = tokio::runtime::Runtime::new().ok()?;
    rt.block_on(async {
        // Geolocation
        let mut ipinfo = ipinfo::IpInfo::new(ipinfo::IpInfoConfig {
            token: None,
            ..Default::default()
        })
        .ok()?;
        let info = ipinfo.lookup_self_v4().await.ok()?;
        let (lat_s, lon_s) = info.loc.split_once(',')?;
        let lat: f32 = lat_s.parse().ok()?;
        let lon: f32 = lon_s.parse().ok()?;

        // Weather
        let res = open_meteo_api::query::OpenMeteo::new()
            .coordinates(lat, lon)
            .ok()?
            .current_weather()
            .ok()?
            .time_zone(open_meteo_api::models::TimeZone::Auto)
            .ok()?
            .daily()
            .ok()?
            .query()
            .await
            .ok()?;

        let current = res.current_weather?;
        let daily = res.daily?;
        Some(CachedWeather {
            wmo: current.weathercode as u8,
            is_day: current.is_day == 1.0,
            current: current.temperature,
            min: *daily.temperature_2m_min.first()?.as_ref()?,
            max: *daily.temperature_2m_max.first()?.as_ref()?,
        })
    })
}

/// Send weather data to the TIGA display.
fn send_weather(dev: &HidDevice, w: &CachedWeather) {
    let icon = WeatherIcon::from_wmo(w.wmo, w.is_day).unwrap_or(WeatherIcon::Cloudy);
    let curr = encode_temperature(w.current);
    let max_t = encode_temperature(w.max);
    let min_t = encode_temperature(w.min);
    let pkt = build_packet(
        0xFE,
        &[0x00, icon as u8, curr[0], curr[1], max_t[0], max_t[1], min_t[0], min_t[1]],
    );
    send(dev, &pkt);
}

// ---------------------------------------------------------------------------
// Sensor state
// ---------------------------------------------------------------------------

struct SensorState {
    smc: Smc,
    components: Components,
    networks: Networks,
    // Current values
    cpu_temp: f32,
    gpu_temp: f32,
    fan_rpm: f32,
    net_mbps: f64,
    last_net_refresh: Instant,
}

impl SensorState {
    fn new() -> Self {
        let smc = Smc::connect().expect("Failed to connect to SMC (Apple Silicon required)");
        let components = Components::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        Self {
            smc,
            components,
            networks,
            cpu_temp: 0.0,
            gpu_temp: 0.0,
            fan_rpm: 0.0,
            net_mbps: 0.0,
            last_net_refresh: Instant::now(),
        }
    }

    fn refresh(&mut self) {
        // Temperatures from sysinfo Components (PMU die sensors on Apple Silicon)
        self.components.refresh(true);
        let mut die_temps: Vec<f32> = Vec::new();
        for c in self.components.iter() {
            let label = c.label();
            if label.starts_with("PMU tdie") {
                if let Some(t) = c.temperature() {
                    if t > 0.0 {
                        die_temps.push(t);
                    }
                }
            }
        }
        if !die_temps.is_empty() {
            die_temps.sort_by(|a, b| b.partial_cmp(a).unwrap());
            // CPU = max die temp (hottest cluster), GPU = average of remaining
            self.cpu_temp = die_temps[0];
            if die_temps.len() > 1 {
                let sum: f32 = die_temps[1..].iter().sum();
                self.gpu_temp = sum / (die_temps.len() - 1) as f32;
            } else {
                self.gpu_temp = self.cpu_temp;
            }
        }

        // Fan speed from SMC (first fan)
        match self.smc.fans() {
            Ok(mut fans) => {
                if let Some(Ok(fan)) = fans.next() {
                    self.fan_rpm = fan.actual.0;
                }
            }
            Err(e) => eprintln!("  warn: fan read failed: {}", e),
        }

        // Network throughput: received()/transmitted() return bytes since last refresh()
        let net_elapsed = self.last_net_refresh.elapsed().as_secs_f64();
        self.networks.refresh(true);
        self.last_net_refresh = Instant::now();
        let delta_bytes: u64 = self
            .networks
            .iter()
            .map(|(_, data)| data.received() + data.transmitted())
            .sum();
        if net_elapsed > 0.0 {
            self.net_mbps = (delta_bytes as f64 * 8.0) / (net_elapsed * 1_000_000.0);
        }
    }

    /// Encode sensor values into the TIGA 0xFF system data packet.
    fn to_sysdata_packet(&self) -> [u8; 32] {
        let cpu = (self.cpu_temp.round() as u8).min(99);
        let gpu = (self.gpu_temp.round() as u8).min(99);
        let ssd: u8 = 0;
        // Fan RPM: u16 BE, displayed as-is by firmware
        let fan = (self.fan_rpm.round() as u16).min(65535);
        // Net speed: u16 BE, firmware divides by 10 to get Mbps
        let net_val = (self.net_mbps * 10.0).round().min(65535.0) as u16;

        build_packet(
            0xFF,
            &[
                0x00, 0x00, cpu, 0x00, gpu, 0x00, ssd,
                (fan >> 8) as u8, (fan & 0xFF) as u8,
                (net_val >> 8) as u8, (net_val & 0xFF) as u8,
            ],
        )
    }
}

// ---------------------------------------------------------------------------
// HID helpers
// ---------------------------------------------------------------------------

fn open_device() -> HidDevice {
    let api = hidapi::HidApi::new().expect("hidapi init failed");
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == TIGA_VENDOR_ID
                && d.product_id() == TIGA_PRODUCT_ID
                && d.usage_page() == TIGA_USAGE_PAGE
                && d.usage() == TIGA_USAGE
        })
        .expect("Zoom75 TIGA not found — is it connected via USB?");
    info.open_device(&api)
        .expect("Failed to open TIGA HID device")
}

fn send(dev: &HidDevice, pkt: &[u8; 32]) {
    if let Err(e) = dev.write(pkt) {
        eprintln!("  warn: HID write failed: {}", e);
    }
    let mut buf = [0u8; 32];
    let _ = dev.read_timeout(&mut buf, 200);
}

fn sync_time(dev: &HidDevice) {
    let now = Local::now();
    let pkt = build_packet(
        0x38,
        &[
            0x00, 0x01,
            (now.year() as u16 >> 8) as u8, (now.year() as u16 & 0xFF) as u8,
            now.month() as u8, now.day() as u8,
            now.hour() as u8, now.minute() as u8, now.second() as u8,
            now.weekday().num_days_from_sunday() as u8,
        ],
    );
    send(dev, &pkt);
}

fn cycle_screen(dev: &HidDevice) {
    // Nav Down cycles: CPU → mbps → RPM → GPU → CPU
    let pkt = build_packet(0x39, &[0x00, 0x02]);
    send(dev, &pkt);
}

/// Navigate back to home reliably from any position.
/// Uses two strategies combined:
/// 1. Repeated "return" to exit sysinfo hierarchy (individual → selection → menu → home)
/// 2. Display reset (0x34 + 0xFB) as final safety to ensure true home state
fn navigate_to_home(dev: &HidDevice) {
    let ret = build_packet(0x39, &[0x00, 0x04]);
    // Exit sysinfo hierarchy step by step (max 4 levels)
    for _ in 0..4 {
        send(dev, &ret);
        thread::sleep(Duration::from_millis(150));
    }
    thread::sleep(Duration::from_millis(200));
    // Display reset as safety net
    send(dev, &build_packet(0x34, &[0x00, 0x01]));
    thread::sleep(Duration::from_millis(150));
    send(dev, &build_packet(0xFB, &[0x00]));
    thread::sleep(Duration::from_millis(300));
}

/// Navigate to the sysinfo CPU screen from any starting position.
/// Uses display reset to reach home reliably, then navigates:
/// home → (down) → selection menu → (switch) → highlighted → (switch) → CPU
fn navigate_to_sysinfo(dev: &HidDevice) {
    let down = build_packet(0x39, &[0x00, 0x02]);
    let switch = build_packet(0x39, &[0x00, 0x03]);

    navigate_to_home(dev);

    // Navigate forward: home → selection → highlight → enter CPU
    send(dev, &down);
    thread::sleep(Duration::from_millis(300));
    send(dev, &switch);
    thread::sleep(Duration::from_millis(300));
    send(dev, &switch);
    thread::sleep(Duration::from_millis(300));
}

// ---------------------------------------------------------------------------
// Graceful shutdown
// ---------------------------------------------------------------------------

fn shutdown(dev: &HidDevice, cached_weather: &Option<CachedWeather>, verbose: bool) {
    println!("\nShutting down gracefully...");

    // Brief pause to let the display finish processing the last data packet
    thread::sleep(Duration::from_millis(500));

    // 1. Navigate back to home (return x4 + display reset)
    if verbose {
        println!("  Navigating to home...");
    }
    navigate_to_home(dev);

    // 2. Re-sync time
    if verbose {
        println!("  Syncing time...");
    }
    sync_time(dev);
    thread::sleep(Duration::from_millis(300));

    // 3. Re-send weather (if cached)
    if let Some(w) = cached_weather {
        if verbose {
            println!(
                "  Sending weather (WMO:{}, {:.0}/{:.0}/{:.0}°C)...",
                w.wmo, w.current, w.min, w.max
            );
        }
        send_weather(dev, w);
        thread::sleep(Duration::from_millis(300));
    }

    println!("Shutdown complete. Display restored to home with time/weather.");
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

/// Spawn `caffeinate -di` to prevent system and display sleep while the monitor runs.
/// Returns the child process handle — must be killed on shutdown.
fn start_caffeinate() -> Option<Child> {
    match Command::new("caffeinate").args(["-di"]).spawn() {
        Ok(child) => {
            println!("Sleep prevention active (caffeinate pid={}).", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("Warning: could not start caffeinate: {} — Mac may sleep.", e);
            None
        }
    }
}

fn main() {
    let args = parse_args();

    // Prevent Mac from sleeping while the monitor is running
    let mut caffeinate = start_caffeinate();

    println!("Connecting to Zoom75 TIGA...");
    let dev = open_device();
    println!("Connected.");

    println!("Initializing sensors...");
    let mut sensors = SensorState::new();
    println!("Sensors ready (SMC + sysinfo).");

    // Initial time sync
    sync_time(&dev);
    println!("Time synced.");

    // Initial weather sync
    let mut cached_weather: Option<CachedWeather> = None;
    if !args.no_weather {
        print!("Fetching weather...");
        match fetch_weather() {
            Some(w) => {
                println!(
                    " OK (WMO:{}, day:{}, {:.0}/{:.0}/{:.0}°C)",
                    w.wmo, w.is_day, w.current, w.min, w.max
                );
                send_weather(&dev, &w);
                thread::sleep(Duration::from_millis(300));
                cached_weather = Some(w);
            }
            None => println!(" failed (will retry later)"),
        }
    }

    // Navigate into sysinfo screen (home → selection → CPU)
    navigate_to_sysinfo(&dev);
    println!("Navigated to sysinfo.");

    // Prime sensor baseline
    sensors.refresh();

    println!();
    println!(
        "Monitor running: interval={}s, cycle={}, weather={}{}",
        args.interval_secs,
        if args.no_cycle {
            "off".to_string()
        } else {
            format!("{}s", args.cycle_secs)
        },
        if args.no_weather {
            "off".to_string()
        } else {
            format!("every {}min", args.weather_interval_mins)
        },
        if args.verbose { ", verbose" } else { "" }
    );
    println!("Press Ctrl+C to stop.");
    println!();

    // Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    let mut last_data = Instant::now() - Duration::from_secs(args.interval_secs);
    let mut last_cycle = Instant::now();
    let mut last_time_sync = Instant::now();
    let mut last_weather_sync = Instant::now();

    while running.load(Ordering::SeqCst) {
        // Send sensor data
        if last_data.elapsed() >= Duration::from_secs(args.interval_secs) {
            sensors.refresh();
            let pkt = sensors.to_sysdata_packet();
            send(&dev, &pkt);
            last_data = Instant::now();

            if args.verbose {
                let now = Local::now();
                println!(
                    "[{}] CPU:{:.0}°C GPU:{:.0}°C Fan:{:.0}RPM Net:{:.1}Mbps",
                    now.format("%H:%M:%S"),
                    sensors.cpu_temp,
                    sensors.gpu_temp,
                    sensors.fan_rpm,
                    sensors.net_mbps,
                );
            }
        }

        // Cycle screen (with small delay to avoid colliding with data send)
        if !args.no_cycle && last_cycle.elapsed() >= Duration::from_secs(args.cycle_secs) {
            thread::sleep(Duration::from_millis(300));
            cycle_screen(&dev);
            last_cycle = Instant::now();
            if args.verbose {
                println!("  [screen cycle]");
            }
        }

        // Re-sync time every 5 minutes
        if last_time_sync.elapsed() >= Duration::from_secs(300) {
            sync_time(&dev);
            last_time_sync = Instant::now();
            if args.verbose {
                println!("  [time re-synced]");
            }
        }

        // Refresh weather periodically
        if !args.no_weather
            && last_weather_sync.elapsed()
                >= Duration::from_secs(args.weather_interval_mins * 60)
        {
            if args.verbose {
                print!("  [weather refresh...");
            }
            match fetch_weather() {
                Some(w) => {
                    send_weather(&dev, &w);
                    if args.verbose {
                        println!(
                            " OK: WMO:{}, {:.0}/{:.0}/{:.0}°C]",
                            w.wmo, w.current, w.min, w.max
                        );
                    }
                    cached_weather = Some(w);
                }
                None => {
                    if args.verbose {
                        println!(" failed, keeping cached]");
                    }
                }
            }
            last_weather_sync = Instant::now();
        }

        thread::sleep(Duration::from_millis(250));
    }

    // Graceful shutdown: navigate home + re-sync time/weather
    shutdown(&dev, &cached_weather, args.verbose);

    // Stop caffeinate — Mac can sleep again
    if let Some(ref mut child) = caffeinate {
        let _ = child.kill();
        let _ = child.wait();
        if args.verbose {
            println!("Sleep prevention stopped.");
        }
    }
}
