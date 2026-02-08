# Zoom75 TIGA System Monitor

Real-time system monitor for the **Meletrix Zoom75 TIGA** keyboard display on **macOS Apple Silicon**.

Streams CPU temperature, GPU temperature, fan speed, and network throughput from your Mac to the keyboard's built-in 320x172 display, automatically cycling between data pages.

> This project is a fork of [zoom-sync](https://github.com/ozboar/zoom-sync) by ozboar, which provides the foundational HID communication layer and board abstraction for the Zoom65 family. The Zoom75 TIGA display protocol was reverse-engineered from the official MeletrixID Windows application.

---

> **DISCLAIMER — USE AT YOUR OWN RISK**
>
> This software is **experimental** (pre-release/beta) and interacts directly with your keyboard's hardware via USB HID. Under certain conditions (system sleep, unexpected USB disconnection), the keyboard display may freeze and become unresponsive. **Recovery from a frozen display may require physically opening the keyboard** — removing the top case and disconnecting the internal display ribbon cable to cut power to the display module. This procedure involves handling fragile ribbon cables and connectors, and carries a real risk of **mechanical and/or electrical damage** to your keyboard.
>
> **The authors accept no responsibility for any damage** — hardware, software, or otherwise — resulting from the use of this software. By downloading or running these tools, you acknowledge these risks and agree to proceed entirely at your own risk.

---

## Features

- Real-time CPU/GPU temperature, fan RPM, and network speed on the keyboard display
- Automatic screen cycling through sysinfo pages (CPU, GPU, RPM, mbps)
- Weather sync (automatic geolocation via ipinfo.io + Open-Meteo API)
- Time synchronization
- Graceful shutdown with display restore (Ctrl+C)
- Sleep prevention via `caffeinate` (prevents display freeze)

## Requirements

- macOS on Apple Silicon (M1/M2/M3/M4)
- Meletrix Zoom75 TIGA connected via USB (Bluetooth is not supported)
- Internet connection (optional, for weather geolocation)

## Quick Start

### Pre-built binaries

Download `tiga_monitor` and `tiga_cmd` from the [latest release](https://github.com/pettipol/zoom75-tiga-monitor/releases/latest).

```bash
# Make executable
chmod +x tiga_monitor tiga_cmd

# Remove macOS quarantine (unsigned binaries)
xattr -d com.apple.quarantine tiga_monitor tiga_cmd

# Set clock and weather (after first connect or USB reset)
./tiga_cmd restore

# Start real-time monitoring
./tiga_monitor -v
```

### Build from source

```bash
git clone https://github.com/pettipol/zoom75-tiga-monitor.git
cd zoom75-tiga-monitor
cargo build --release --example tiga_monitor --example tiga_cmd
# Binaries in target/release/examples/
```

## Usage

### `tiga_monitor` — Real-time system monitor

```bash
# Standard use (recommended)
./tiga_monitor -v

# Custom intervals: data every 1s, screen switch every 2s
./tiga_monitor --interval 1 --cycle 2

# Without weather (useful offline)
./tiga_monitor --no-weather -v
```

| Option                       | Description                        | Default |
|------------------------------|------------------------------------|---------|
| `--interval <SEC>`          | Sensor update interval (seconds)   | 2       |
| `--cycle <SEC>`             | Screen switch interval (seconds)   | 3       |
| `--no-cycle`                | Stay on one screen (no cycling)    | —       |
| `--no-weather`              | Disable weather sync               | —       |
| `--weather-interval <MIN>`  | Weather refresh interval (minutes) | 30      |
| `-v` / `--verbose`          | Print sensor values to terminal    | off     |

### `tiga_cmd` — One-shot commands

```bash
./tiga_cmd restore        # Set clock + weather (most useful after reset)
./tiga_cmd time           # Set clock only
./tiga_cmd weather        # Set weather only
./tiga_cmd home           # Return display to home screen
./tiga_cmd sysinfo 42 77 35 1350 500   # Manual: CPU GPU SSD Fan Net
```

See [USER_GUIDE.md](USER_GUIDE.md) for the full command reference, display navigation, troubleshooting, and detailed usage scenarios.

## Warning — Display Freeze and Hardware Reset

**This software is in pre-release/beta state.** If macOS goes to sleep while the monitor is running, the USB connection drops and the keyboard display may freeze on the last sysinfo screen.

The `caffeinate` integration prevents sleep in normal use. However, if a freeze does occur:

- The keyboard itself continues to work normally — **only the display module is affected**
- **Disconnecting the USB cable is NOT sufficient to recover** — the display module has its own power state independent of the USB connection, and retains whatever was on screen
- Recovery requires a **hardware reset**: you must **physically open the keyboard top case**, locate the internal display ribbon cable, **disconnect it** to cut power to the display, then reconnect it and close the case
- This involves handling **fragile ribbon cables, clips, and connectors** — there is a real risk of **mechanical and/or electrical damage** to your keyboard

**To minimize risk:**
- **Always stop the monitor with Ctrl+C** (triggers a clean shutdown that restores the home screen)
- **Never close the laptop lid** or let the Mac sleep while the monitor is running
- Do not force-kill the process (`kill -9`) or close the terminal abruptly

See [USER_GUIDE.md — Scenario 6](USER_GUIDE.md#scenario-6-display-frozen-after-sleepsuspend) for the full hardware reset procedure.

## How It Works

| Sensor   | Source                           | Notes                           |
|----------|----------------------------------|---------------------------------|
| CPU temp | PMU die sensors (hottest core)   | via `sysinfo` crate             |
| GPU temp | PMU die sensors (average)        | via `sysinfo` crate             |
| Fan RPM  | Apple SMC                        | via `macsmc` crate              |
| Net speed| Active network interfaces        | Sum of upload + download        |

The monitor communicates with the keyboard via 32-byte HID packets using a reverse-engineered protocol (CRC-CCITT checksum, command-based). See the [boards/zoom75_tiga](boards/zoom75_tiga/) crate for protocol details.

## Credits

- **Upstream**: [zoom-sync](https://github.com/ozboar/zoom-sync) by ozboar — HID communication layer and board abstraction
- **Protocol**: Reverse-engineered from MeletrixID (.NET 4.7.2, decompiled with ILSpy)
- **Weather**: [Open-Meteo](https://open-meteo.com) (CC BY 4.0) + [ipinfo.io](https://ipinfo.io)
- **AI-assisted development**:
  - [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (Anthropic) — CLI agent for code generation, debugging, and reverse engineering
  - Local LLMs via [Ollama](https://ollama.com) — including [Qwen 3](https://github.com/QwenLM/Qwen3) for protocol analysis and code review

## License

This project inherits the license from [zoom-sync](https://github.com/ozboar/zoom-sync).
