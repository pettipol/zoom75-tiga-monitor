//! Single-command tool for interactive TIGA testing.
//!
//! Usage:
//!   cargo run --example tiga_cmd -- <command> [args...]
//!
//! Commands:
//!   home          - Reset to home screen
//!   down          - Navigate down
//!   switch        - Navigate switch (horizontal)
//!   return        - Navigate return/back
//!   time          - Sync current time
//!   weather       - Fetch and send weather (auto geolocation)
//!   restore       - Sync time + weather in one shot
//!   cpu <temp>    - Send CPU temp (0x37) continuously for 10s
//!   gpu <temp>    - Send GPU temp (0x38, short) continuously for 10s
//!   fan <rpm>     - Send fan speed (0x39, 3-byte) continuously for 10s
//!   net <speed>   - Send net speed (0x3D) continuously for 10s
//!   all <cpu> <gpu> <rpm>  - Send all system data continuously for 15s

use zoom75_tiga::protocol::build_packet;
use zoom75_tiga::consts::*;
use zoom75_tiga::types::{WeatherIcon, encode_temperature};
use std::env;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: tiga_cmd <command> [args...]");
        println!("Commands: home, down, switch, return, time, weather, restore,");
        println!("          cpu <t>, gpu <t>, fan <rpm>, net <spd>, all <cpu> <gpu> <rpm>");
        return;
    }

    let api = hidapi::HidApi::new().expect("hidapi init failed");
    let dev = api.device_list()
        .find(|d| d.vendor_id() == TIGA_VENDOR_ID && d.product_id() == TIGA_PRODUCT_ID
            && d.usage_page() == TIGA_USAGE_PAGE && d.usage() == TIGA_USAGE)
        .expect("TIGA not found")
        .open_device(&api)
        .expect("failed to open device");
    let mut buf = [0u8; 32];

    match args[1].as_str() {
        "home" => {
            send(&dev, &build_packet(0x34, &[0x00, 0x01]), &mut buf);
            thread::sleep(Duration::from_millis(150));
            send(&dev, &build_packet(0xFB, &[0x00]), &mut buf);
            println!("OK: Reset to Home screen");
        }
        "down" => {
            send(&dev, &build_packet(0x39, &[0x00, 0x02]), &mut buf);
            println!("OK: Navigated Down");
        }
        "switch" => {
            send(&dev, &build_packet(0x39, &[0x00, 0x03]), &mut buf);
            println!("OK: Navigated Switch (horizontal)");
        }
        "return" => {
            send(&dev, &build_packet(0x39, &[0x00, 0x04]), &mut buf);
            println!("OK: Navigated Return/Back");
        }
        "up" => {
            send(&dev, &build_packet(0x39, &[0x00, 0x01]), &mut buf);
            println!("OK: Navigated Up (action=1)");
        }
        "nav" => {
            let val: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            send(&dev, &build_packet(0x39, &[0x00, val]), &mut buf);
            println!("OK: Nav action={} sent", val);
        }
        "time" => {
            let now = chrono::Local::now();
            use chrono::{Datelike, Timelike};
            send(&dev, &build_packet(0x38, &[
                0x00, 0x01,
                (now.year() as u16 >> 8) as u8, (now.year() as u16 & 0xFF) as u8,
                now.month() as u8, now.day() as u8,
                now.hour() as u8, now.minute() as u8,
                now.second() as u8,
                now.weekday().num_days_from_sunday() as u8,
            ]), &mut buf);
            println!("OK: Time set to {}", now.format("%Y-%m-%d %H:%M:%S"));
        }
        "cpu" => {
            let temp: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(42);
            println!("Sending CPU temp = {}°C via cmd=0xFF for 10 seconds...", temp);
            send_loop(&dev, &mut buf, 10, || build_packet(0xFF, &[0x00, 0x00, temp]));
            println!("\nDone. CPU temp {} sent.", temp);
        }
        "sysinfo" => {
            // Full system data using MeletrixID protocol (cmd=0xFF, 11-byte payload)
            let cpu: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(42);
            let gpu: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(55);
            let ssd: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let fan: u16 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
            let net: u16 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
            println!("Sending system data: CPU={}°C GPU={}°C SSD={}°C Fan={}RPM Net={}", cpu, gpu, ssd, fan, net);
            println!("Protocol: 0xFF with 11-byte payload [0,0,cpu,0,gpu,0,ssd,fan_hi,fan_lo,net_hi,net_lo]");
            let pkt = build_packet(0xFF, &[
                0x00, 0x00, cpu, 0x00, gpu, 0x00, ssd,
                (fan >> 8) as u8, (fan & 0xFF) as u8,
                (net >> 8) as u8, (net & 0xFF) as u8,
            ]);
            println!("Packet bytes [8..24]: {:02X?}", &pkt[8..24]);
            send(&dev, &pkt, &mut buf);
            println!("Sent! Check all 4 screens on the display.");
        }
        "raw" => {
            // raw <cmd_hex> <d0> <d1> <d2> [<d3> <d4>]
            let cmd: u8 = args.get(2).and_then(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok()).unwrap_or(0xFF);
            let data_bytes: Vec<u8> = args[3..].iter().filter_map(|s| s.parse().ok()).collect();
            println!("Sending cmd=0x{:02X} data={:?}", cmd, data_bytes);
            send(&dev, &build_packet(cmd, &data_bytes), &mut buf);
            println!("Done.");
        }
        "gpu" => {
            let temp: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(55);
            println!("Sending GPU temp = {}°C for 10 seconds...", temp);
            send_loop(&dev, &mut buf, 10, || build_packet(0x38, &[0x00, 0x00, temp]));
            println!("\nDone. GPU temp {} sent for 10s.", temp);
        }
        "fan" => {
            let rpm: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1200);
            println!("Sending fan speed = {} RPM for 10 seconds...", rpm);
            send_loop(&dev, &mut buf, 10, || {
                build_packet(0x39, &[0x00, (rpm >> 8) as u8, (rpm & 0xFF) as u8])
            });
            println!("\nDone. Fan {} RPM sent for 10s.", rpm);
        }
        "net" => {
            let spd: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(500);
            println!("Sending net speed = {} for 10 seconds...", spd);
            send_loop(&dev, &mut buf, 10, || {
                build_packet(0x3D, &[0x00,
                    ((spd >> 24) & 0xFF) as u8, ((spd >> 16) & 0xFF) as u8,
                    ((spd >> 8) & 0xFF) as u8, (spd & 0xFF) as u8])
            });
            println!("\nDone. Net speed {} sent for 10s.", spd);
        }
        "all" => {
            let cpu: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(42);
            let gpu: u8 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(55);
            let rpm: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1200);
            println!("Sending ALL: CPU={}°C GPU={}°C Fan={} RPM for 15 seconds...", cpu, gpu, rpm);
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(15) {
                send(&dev, &build_packet(0x37, &[0x00, 0x00, cpu]), &mut buf);
                thread::sleep(Duration::from_millis(80));
                send(&dev, &build_packet(0x38, &[0x00, 0x00, gpu]), &mut buf);
                thread::sleep(Duration::from_millis(80));
                send(&dev, &build_packet(0x39, &[0x00, (rpm>>8) as u8, (rpm&0xFF) as u8]), &mut buf);
                thread::sleep(Duration::from_millis(840));
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            println!("\nDone. All system data sent for 15s.");
        }
        "probe_sys" => {
            let temp: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(42);
            let candidates: &[u8] = &[
                0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
                0xFF, 0x35, 0x36, 0x3A, 0x3C, 0x3E,
            ];
            println!("Probing {} command IDs for CPU temp={}. Watch the display!", candidates.len(), temp);
            for &cmd in candidates {
                println!("  Trying cmd=0x{:02X} ...", cmd);
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, 0x00, temp]), &mut buf);
                    thread::sleep(Duration::from_millis(200));
                }
                thread::sleep(Duration::from_millis(1500));
            }
            println!("Probe complete.");
        }
        "probe_id" => {
            // Each command gets a UNIQUE value so user can identify which one worked
            let candidates: &[(u8, u8)] = &[
                (0xF0, 10), (0xF1, 11), (0xF2, 12), (0xF3, 13),
                (0xF4, 14), (0xF5, 15), (0xF6, 16), (0xF7, 17),
                (0xF8, 18), (0xF9, 19), (0xFA, 20),
                (0xFF, 25), (0x35, 35), (0x36, 36), (0x3A, 50),
                (0x3C, 60), (0x3E, 70),
            ];
            println!("Sending UNIQUE values per command ID. Watch for the value that appears!");
            println!("Mapping: F0=10, F1=11, F2=12, F3=13, F4=14, F5=15, F6=16, F7=17");
            println!("         F8=18, F9=19, FA=20, FF=25, 35=35, 36=36, 3A=50, 3C=60, 3E=70\n");
            for &(cmd, val) in candidates {
                println!("  cmd=0x{:02X} → value={}", cmd, val);
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, 0x00, val]), &mut buf);
                    thread::sleep(Duration::from_millis(200));
                }
                thread::sleep(Duration::from_millis(2000));
            }
            println!("\nDone! What value do you see on the display?");
        }
        "probe_gpu" => {
            // Comprehensive GPU command probe.
            // User MUST be on the GPU screen. Each candidate gets a unique value.
            println!("=== GPU Command Probe ===");
            println!("You MUST be on the GPU screen! Each candidate sends a unique temp value.");
            println!("At the end, tell me what number the GPU screen shows.\n");

            let mut mapping: Vec<(String, u8)> = Vec::new();
            let mut v: u8 = 60; // start at 60 to clearly differ from current GPU=55

            // Phase A: 0xFF with selector byte in data[0] (0x01..0x09)
            // Hypothesis: 0xFF is universal, data[0] selects sensor
            println!("Phase A: 0xFF with data[0] selector (0x01-0x09)...");
            for sel in 1u8..=9 {
                mapping.push((format!("0xFF[{:02X},00,{}]", sel, v), v));
                for _ in 0..3 {
                    send(&dev, &build_packet(0xFF, &[sel, 0x00, v]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 60-68)");

            // Phase B: 0xFF with selector in data[1] (0x01..0x04)
            println!("Phase B: 0xFF with data[1] selector...");
            for sel in 1u8..=4 {
                mapping.push((format!("0xFF[00,{:02X},{}]", sel, v), v));
                for _ in 0..3 {
                    send(&dev, &build_packet(0xFF, &[0x00, sel, v]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 69-72)");

            // Phase C: zoom75.py format (byte[1]=0x00) for key commands
            // Maybe system data uses zoom75 format, not TIGA format
            println!("Phase C: zoom75 format (byte[1]=0x00) for 0x38, 0x37, 0x3B...");
            let zoom_cmds: &[(u8, &str)] = &[
                (0x38, "GPU/0x38"), (0x37, "CPU/0x37"), (0x3B, "Wthr/0x3B"),
                (0x3C, "0x3C"), (0x3E, "0x3E"), (0x3F, "Time/0x3F"),
            ];
            for &(cmd, label) in zoom_cmds {
                mapping.push((format!("zoom75_fmt({})={}", label, v), v));
                let pkt = build_zoom75_packet(cmd, &[0x00, 0x00, v], 4 + 3 + 1);
                for _ in 0..3 {
                    send(&dev, &pkt, &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 73-78)");

            // Phase D: Commands 0xF0-0xFA (re-test — last time we were on CPU screen!)
            println!("Phase D: Re-test 0xF0-0xFA (now on GPU screen)...");
            for cmd in 0xF0u8..=0xFA {
                mapping.push((format!("cmd=0x{:02X}", cmd), v));
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, 0x00, v]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 79-89)");

            // Phase E: Commands 0xE0-0xEF (untested range)
            println!("Phase E: Commands 0xE0-0xEF...");
            for cmd in 0xE0u8..=0xEF {
                mapping.push((format!("cmd=0x{:02X}", cmd), v));
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, 0x00, v]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 90-105)");

            // Phase F: Remaining 0x30-0x3F (excluding 0x34=reset, 0x38=time, 0x39=nav)
            println!("Phase F: Commands 0x30-0x3F (safe ones)...");
            for &cmd in &[0x30u8, 0x31, 0x32, 0x33, 0x35, 0x36, 0x37, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F] {
                mapping.push((format!("cmd=0x{:02X}", cmd), v));
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, 0x00, v]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                v += 1;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (values 106-118)");

            println!("\n=== Probe complete ({} candidates tested) ===", mapping.len());
            println!("\nValue → Command mapping:");
            for (label, val) in &mapping {
                println!("  {:>3} → {}", val, label);
            }
            println!("\nWhat value does the GPU screen show?");
        }
        "probe_fan" => {
            // Fan/RPM probe — same structure but for RPM screen
            // RPM uses 16-bit value, we send small unique RPMs
            println!("=== Fan/RPM Command Probe ===");
            println!("You MUST be on the RPM screen!\n");

            let mut mapping: Vec<(String, u16)> = Vec::new();
            let mut rpm: u16 = 100;

            // Phase A: 0xFF with selector bytes, RPM-style 3-byte [sel, hi, lo]
            println!("Phase A: 0xFF selectors with RPM format...");
            for sel in 1u8..=9 {
                mapping.push((format!("0xFF[{:02X},hi,lo] rpm={}", sel, rpm), rpm));
                for _ in 0..3 {
                    send(&dev, &build_packet(0xFF, &[sel, (rpm>>8) as u8, (rpm&0xFF) as u8]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                rpm += 100;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (100-900 RPM)");

            // Phase B: Commands 0xF0-0xFA
            println!("Phase B: Commands 0xF0-0xFA...");
            for cmd in 0xF0u8..=0xFA {
                mapping.push((format!("cmd=0x{:02X} rpm={}", cmd, rpm), rpm));
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, (rpm>>8) as u8, (rpm&0xFF) as u8]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                rpm += 100;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (1000-2000 RPM)");

            // Phase C: Commands 0xE0-0xEF
            println!("Phase C: Commands 0xE0-0xEF...");
            for cmd in 0xE0u8..=0xEF {
                mapping.push((format!("cmd=0x{:02X} rpm={}", cmd, rpm), rpm));
                for _ in 0..3 {
                    send(&dev, &build_packet(cmd, &[0x00, (rpm>>8) as u8, (rpm&0xFF) as u8]), &mut buf);
                    thread::sleep(Duration::from_millis(300));
                }
                rpm += 100;
                thread::sleep(Duration::from_millis(300));
            }
            println!("  done (2100-3600 RPM)");

            println!("\n=== Probe complete ===");
            println!("RPM → Command mapping:");
            for (label, val) in &mapping {
                println!("  {:>5} RPM → {}", val, label);
            }
            println!("\nWhat RPM value does the screen show?");
        }
        "weather" => {
            println!("Fetching weather (geolocation + open-meteo)...");
            match fetch_weather_blocking() {
                Some((icon, curr, max_t, min_t, wmo, current, min, max)) => {
                    let pkt = build_packet(
                        0xFE,
                        &[0x00, icon as u8, curr[0], curr[1], max_t[0], max_t[1], min_t[0], min_t[1]],
                    );
                    send(&dev, &pkt, &mut buf);
                    println!("OK: Weather set (WMO:{}, {:.0}/{:.0}/{:.0}°C)", wmo, current, min, max);
                }
                None => println!("Failed to fetch weather data."),
            }
        }
        "restore" => {
            // Sync time
            let now = chrono::Local::now();
            use chrono::{Datelike, Timelike};
            send(&dev, &build_packet(0x38, &[
                0x00, 0x01,
                (now.year() as u16 >> 8) as u8, (now.year() as u16 & 0xFF) as u8,
                now.month() as u8, now.day() as u8,
                now.hour() as u8, now.minute() as u8,
                now.second() as u8,
                now.weekday().num_days_from_sunday() as u8,
            ]), &mut buf);
            println!("Time set to {}", now.format("%Y-%m-%d %H:%M:%S"));

            // Sync weather
            thread::sleep(Duration::from_millis(300));
            println!("Fetching weather...");
            match fetch_weather_blocking() {
                Some((icon, curr, max_t, min_t, wmo, current, min, max)) => {
                    let pkt = build_packet(
                        0xFE,
                        &[0x00, icon as u8, curr[0], curr[1], max_t[0], max_t[1], min_t[0], min_t[1]],
                    );
                    send(&dev, &pkt, &mut buf);
                    println!("Weather set (WMO:{}, {:.0}/{:.0}/{:.0}°C)", wmo, current, min, max);
                }
                None => println!("Weather fetch failed (time was still synced)."),
            }
            println!("Restore complete.");
        }
        _ => println!("Unknown command: {}", args[1]),
    }
}

/// Build packet in zoom75.py style: byte[1]=0x00, checksum includes 0xA5
fn build_zoom75_packet(cmd: u8, payload: &[u8], length_field: u8) -> [u8; 32] {
    let mut data = [0u8; 32];
    // Phase 1: command data (bytes 0-7 still zero for checksum calc)
    data[8] = 0xA5;
    data[9] = cmd;
    data[10] = 0x00;
    data[11] = payload.len() as u8;
    for (i, &b) in payload.iter().enumerate() {
        data[12 + i] = b;
    }
    // Checksum: zoom75.py style - sum ALL bytes, & 0xFF, ^ 255, % 255
    let ck_pos = 12 + payload.len();
    let sum: u16 = data.iter().map(|&b| b as u16).sum();
    data[ck_pos] = (((sum & 0xFF) as u8) ^ 0xFF) % 255;
    // Phase 2: header AFTER checksum
    data[0] = 0x1C;
    // byte[1] stays 0x00!
    data[5] = length_field;
    // Phase 3: CRC
    let crc = zoom75_tiga::protocol::crc_ccitt(&data);
    data[6] = (crc & 0xFF) as u8;
    data[7] = ((crc >> 8) & 0xFF) as u8;
    data
}

fn send(dev: &hidapi::HidDevice, pkt: &[u8], buf: &mut [u8]) {
    let _ = dev.write(pkt);
    let _ = dev.read_timeout(buf, 300);
}

fn send_loop(dev: &hidapi::HidDevice, buf: &mut [u8], secs: u64, build: impl Fn() -> [u8; 32]) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        send(dev, &build(), buf);
        thread::sleep(Duration::from_millis(1000));
        print!(".");
        std::io::stdout().flush().unwrap();
    }
}

/// Fetch weather via ipinfo + open-meteo (blocking). Returns encoded data ready for packet.
fn fetch_weather_blocking() -> Option<(WeatherIcon, [u8; 2], [u8; 2], [u8; 2], u8, f32, f32, f32)> {
    let rt = tokio::runtime::Runtime::new().ok()?;
    rt.block_on(async {
        let mut ipinfo = ipinfo::IpInfo::new(ipinfo::IpInfoConfig {
            token: None,
            ..Default::default()
        })
        .ok()?;
        let info = ipinfo.lookup_self_v4().await.ok()?;
        let (lat_s, lon_s) = info.loc.split_once(',')?;
        let lat: f32 = lat_s.parse().ok()?;
        let lon: f32 = lon_s.parse().ok()?;

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

        let cw = res.current_weather?;
        let daily = res.daily?;
        let wmo = cw.weathercode as u8;
        let is_day = cw.is_day == 1.0;
        let current = cw.temperature;
        let min = *daily.temperature_2m_min.first()?.as_ref()?;
        let max = *daily.temperature_2m_max.first()?.as_ref()?;

        let icon = WeatherIcon::from_wmo(wmo, is_day)?;

        Some((icon, encode_temperature(current), encode_temperature(max), encode_temperature(min), wmo, current, min, max))
    })
}
