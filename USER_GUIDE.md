# Zoom75 TIGA System Monitor — User Guide

Real-time system monitor for the Meletrix Zoom75 TIGA keyboard.
Reads CPU temperature, GPU temperature, fan speed, and network throughput
from your Mac and displays them on the keyboard's built-in screen,
automatically cycling between data pages.

## Requirements

- macOS on Apple Silicon (M1/M2/M3/M4)
- Zoom75 TIGA connected via USB (Bluetooth is not supported)
- Internet connection (for weather geolocation — optional)

## Setup

### 1. Make binaries executable

```bash
chmod +x tiga_monitor tiga_cmd
```

### 2. Allow unsigned binaries (first run only)

macOS will block the binaries because they are not code-signed.
The easiest fix is to remove the quarantine attribute:

```bash
xattr -d com.apple.quarantine tiga_monitor tiga_cmd
```

Alternatively: try running the binary, then go to
**System Settings > Privacy & Security** and click **"Allow Anyway"**.

---

## Usage Scenarios

### Scenario 1: First connection or after a hard reset

After plugging in the keyboard (or after a USB disconnect/reconnect),
the clock and weather are lost. To restore everything in one step:

```bash
./tiga_cmd restore
```

Output:
```
Time set to 2026-02-08 12:06:17
Fetching weather...
Weather set (WMO:3, 13/8/14°C)
Restore complete.
```

The display now shows the updated clock. Weather data is saved by the
firmware and can be viewed by manually cycling from the home screen.

### Scenario 2: Start system monitoring

To show real-time CPU, GPU, fan, and network data on the display:

```bash
./tiga_monitor -v
```

Output:
```
Sleep prevention active (caffeinate pid=62005).
Connecting to Zoom75 TIGA...
Connected.
Initializing sensors...
Sensors ready (SMC + sysinfo).
Time synced.
Fetching weather... OK (WMO:3, day:true, 14/8/14°C)
Navigated to sysinfo.

Monitor running: interval=2s, cycle=3s, weather=every 30min, verbose
Press Ctrl+C to stop.

[12:57:05] CPU:45°C GPU:44°C Fan:1350RPM Net:0.3Mbps
[12:57:07] CPU:45°C GPU:44°C Fan:1342RPM Net:0.7Mbps
  [screen cycle]
[12:57:09] CPU:45°C GPU:44°C Fan:1355RPM Net:0.4Mbps
```

The monitor:
1. **Activates sleep prevention** — launches `caffeinate -di` in the
   background, which prevents the Mac from sleeping while the monitor
   is running. This is essential to avoid display freezes (see below).
2. Syncs clock and weather
3. Enters the sysinfo screens and cycles between CPU, GPU, RPM, mbps
4. Updates sensor values every 2 seconds, switches screen every 3 seconds
5. Refreshes weather every 30 minutes

### Scenario 3: Clean shutdown

To stop the monitor, press **Ctrl+C** in the terminal. This is the only
way to stop it, and it triggers a clean shutdown sequence:

```
^C
Shutting down gracefully...
  Navigating to home...
  Syncing time...
  Sending weather (WMO:3, 14/8/14°C)...
Shutdown complete. Display restored to home with time/weather.
Sleep prevention stopped.
```

What happens on Ctrl+C:
1. The monitor exits the sysinfo screens
2. Navigates the display back to the home screen (clock)
3. Re-syncs clock and weather so the display stays useful
4. **Stops sleep prevention** (`caffeinate` is terminated) — the Mac
   can sleep again normally

**Important**: Always use Ctrl+C to stop the monitor. Do not force-kill
the process (e.g., `kill -9`) or close the terminal window abruptly, as
this skips the shutdown sequence and leaves the display stuck on the
last sysinfo screen (though still manually navigable). It also leaves
`caffeinate` running, which would prevent your Mac from sleeping until
you manually kill it (`killall caffeinate`).

### Scenario 4: Set clock only (no internet needed)

```bash
./tiga_cmd time
```

### Scenario 5: Set weather only

```bash
./tiga_cmd weather
```

### Scenario 6: Display frozen after sleep/suspend

If the Mac went to sleep and the display is frozen, **disconnecting the
USB cable alone is NOT enough** — the display module retains its own power
state independently of the USB connection.

To recover, a **hardware reset** is required:

1. **Open the keyboard top case** to access the internal display connector
2. **Disconnect the display ribbon cable** to cut power to the display
3. **Reconnect the display ribbon cable**
4. **Close the top case**
5. Reconnect the USB cable
6. Restore clock and weather:

```bash
./tiga_cmd restore
```

**WARNING**: Opening the top case and manipulating internal connectors
carries risk of mechanical and electrical damage (fragile ribbon cables,
clips, connectors). Proceed with care and at your own risk.

---

## Command Reference — `tiga_cmd`

Single-command tool. Each invocation connects, runs the command, and
disconnects.

### Restore and sync

```bash
# Full restore (clock + weather) — most useful after a reset
./tiga_cmd restore

# Clock only
./tiga_cmd time

# Weather only (requires internet)
./tiga_cmd weather
```

### Display navigation

```bash
# Return to home (clock) — uses display reset, works from anywhere
./tiga_cmd home

# Navigate down (in menu: next item; in sysinfo: next screen)
./tiga_cmd down

# Confirm selection / enter submenu
./tiga_cmd switch

# Go back one level
./tiga_cmd return

# Navigate up
./tiga_cmd up
```

### Manual system data

Useful for testing and debugging. Values appear on sysinfo screens.

```bash
# Format: sysinfo <CPU°C> <GPU°C> <SSD°C> <FanRPM> <Net>
# Net: firmware divides by 10, so 500 = 50.0 Mbps

# Example: CPU=42°C, GPU=77°C, SSD=35°C, Fan=1350 RPM, Net=50.0 Mbps
./tiga_cmd sysinfo 42 77 35 1350 500

# Temperatures only (no fan/net)
./tiga_cmd sysinfo 50 60 0 0 0
```

### Manual sysinfo navigation

To enter the sysinfo screens (CPU/GPU/RPM/mbps) manually:

```bash
# From home: enter the data selection menu
./tiga_cmd down

# Activate/highlight the menu
./tiga_cmd switch

# Enter the first screen (CPU) — now shows the numeric value
./tiga_cmd switch

# From here, "down" cycles between screens:
# CPU → mbps → RPM → GPU → CPU
./tiga_cmd down
```

### Raw commands (advanced)

```bash
# Send a packet with arbitrary command and data bytes
# Format: raw <CMD_HEX> <byte0> <byte1> ...
./tiga_cmd raw FF 0 0 42 0 77 0 0 5 70 0 0
```

---

## Options Reference — `tiga_monitor`

### All options

| Option                       | Description                              | Default |
|------------------------------|------------------------------------------|---------|
| `--interval <SEC>`          | Sensor update interval (seconds)         | 2       |
| `--cycle <SEC>`             | Screen switch interval (seconds)         | 3       |
| `--no-cycle`                | Stay on one screen (no cycling)          | —       |
| `--no-weather`              | Disable weather sync                     | —       |
| `--weather-interval <MIN>`  | Weather refresh interval (minutes)       | 30      |
| `-v` / `--verbose`          | Print values to terminal                 | off     |
| `-h` / `--help`             | Show help                                | —       |

### Usage examples

```bash
# Standard use with terminal output (recommended)
./tiga_monitor -v

# Silent mode (no terminal output, display only)
./tiga_monitor

# Fast updates: data every 1s, screen switch every 2s
./tiga_monitor --interval 1 --cycle 2

# Relaxed updates: data every 5s, screen switch every 8s
./tiga_monitor --interval 5 --cycle 8

# Without weather (useful offline)
./tiga_monitor --no-weather -v

# Weather updated every 15 minutes (instead of default 30)
./tiga_monitor --weather-interval 15

# Fixed screen (stay on current screen, no cycling)
./tiga_monitor --no-cycle -v

# Fully custom: 3s data, 5s cycling, weather every 10min, verbose
./tiga_monitor --interval 3 --cycle 5 --weather-interval 10 -v
```

---

## Display Screens

### Home and main screens

The home screen shows the clock only. Weather, GIF, and animation are
on separate screens accessible by manually cycling from the home screen.
There is no combined clock+weather home screen — they are separate pages.

### Sysinfo screens (managed by the monitor)

The monitor automatically cycles between 4 data screens:

| Screen  | Data displayed                         |
|---------|----------------------------------------|
| **CPU** | CPU temperature (°C)                   |
| **GPU** | GPU temperature (°C)                   |
| **RPM** | Fan speed (revolutions per minute)     |
| **mbps**| Network speed (Megabits per second)    |

Cycling order: CPU → mbps → RPM → GPU → CPU

## How Sensors Work

| Sensor     | Source                              | Notes                             |
|------------|-------------------------------------|-----------------------------------|
| CPU temp   | PMU die sensors (hottest cluster)   | Highest value among die sensors   |
| GPU temp   | PMU die sensors (average)           | Average of remaining die sensors  |
| Fan RPM    | Apple SMC                           | First system fan                  |
| Net speed  | Active network interfaces           | Sum of upload + download          |

Notes:
- On M4 at idle, fans are off (Fan=0 RPM is normal)
- CPU/GPU values stabilize 2-4 seconds after first launch

## Sleep Prevention

The monitor automatically launches `caffeinate -di` at startup, which
prevents the Mac from sleeping (both display and system). This is
**critical** because if the Mac sleeps, the USB connection drops and the
keyboard display may freeze. A frozen display cannot be recovered by
simply reconnecting the USB cable — it requires a hardware reset
(opening the top case to disconnect the internal display connector),
which carries risk of damage. See **Scenario 6** for details.

When the monitor is stopped (Ctrl+C), caffeinate is terminated and the
Mac returns to its normal sleep behavior.

---

## Troubleshooting

**"Zoom75 TIGA not found"**
- Make sure the keyboard is connected via USB (not Bluetooth)
- Make sure no other application (e.g., MeletrixID) is using the device

**"Failed to connect to SMC"**
- The monitor requires macOS on Apple Silicon
- Intel Macs are not supported for temperature sensors

**CPU/GPU showing 0°C**
- May happen at first launch; values stabilize after 2-4 seconds

**Fan always at 0 RPM**
- Normal on M4 at idle: fans stay off until the system is under load

**Display not cycling through sysinfo screens**
- The monitor must navigate the hierarchy: home → menu → selection → CPU
- If cycling doesn't work, try resetting with `./tiga_cmd home` and restart

**"Fetching weather... failed"**
- Check your internet connection
- Use `--no-weather` to start the monitor without weather
- Weather will be retried at the next interval

**Display frozen after sleep**
- The monitor automatically prevents sleep (`caffeinate -di`)
- If the Mac somehow still slept and the display froze:
  - **Disconnecting the USB cable is NOT enough** — the display retains
    its own power state
  - A **hardware reset** is required: open the keyboard top case,
    disconnect and reconnect the internal display ribbon cable, then
    close the case. This carries risk of mechanical/electrical damage.
  - After the hardware reset, reconnect USB and run `./tiga_cmd restore`
  - See **Scenario 6** above for full instructions

---

## Acknowledgements and Sources

This project was built through reverse engineering and the work of the
open-source community.

### Protocol Reverse Engineering

The HID protocol used by the Zoom75 TIGA display was reverse-engineered
by decompiling the official **MeletrixID** Windows application (.NET
Framework 4.7.2, decompiled with ILSpy). Key findings:
- 32-byte HID packets with CRC-CCITT and XOR checksum
- Command set: Time (0x38), Weather (0xFE), Navigation (0x39), System
  Data (0xFF), Image (0xFC), Theme (0xFD), Display Reset (0x34+0xFB)
- Sensor data format and display firmware behavior were validated
  through extensive real-device testing

### Upstream Project

This tool is built as a fork of
[zoom-sync](https://github.com/ozboar/zoom-sync) by ozboar, which
provides the foundational HID communication layer and board abstraction
architecture for the Zoom65 family.

### Libraries and APIs

| Library / Service | Purpose | License |
|-------------------|---------|---------|
| [hidapi](https://crates.io/crates/hidapi) | USB HID communication | MIT |
| [sysinfo](https://crates.io/crates/sysinfo) | CPU/GPU temperature reading (PMU die sensors) | MIT |
| [macsmc](https://crates.io/crates/macsmc) | Apple SMC access (fan speed) | MIT/Apache-2.0 |
| [open-meteo-rs](https://crates.io/crates/open-meteo-rs) | Weather data (Open-Meteo API) | MIT |
| [ipinfo.io](https://ipinfo.io) | IP-based geolocation (for weather) | Free tier |
| [Open-Meteo](https://open-meteo.com) | Free weather API | CC BY 4.0 |
| [ctrlc](https://crates.io/crates/ctrlc) | Ctrl+C signal handling | MIT/Apache-2.0 |
| [tokio](https://crates.io/crates/tokio) | Async runtime (for HTTP requests) | MIT |

### Tools Used

- **ILSpy / ilspycmd** — .NET decompiler (used to analyze MeletrixID)
- **Rust** — programming language and toolchain
- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** — Anthropic's CLI agent for code generation, debugging, and reverse engineering
- **[Ollama](https://ollama.com)** — local LLM inference, including [Qwen 3](https://github.com/QwenLM/Qwen3) for protocol analysis and code review
