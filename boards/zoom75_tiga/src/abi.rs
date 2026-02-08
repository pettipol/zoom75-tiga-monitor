//! TIGA command builders.
//!
//! Each function constructs a 32-byte packet using the TIGA protocol wrapper.

use crate::protocol::build_packet;
use crate::types::{encode_temperature, NavAction, ThemeId, WeatherIcon};

/// Time sync command (0x38).
/// Year is full 16-bit (e.g., 2026), not modulo 100.
pub fn set_time(
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    weekday: u8,
) -> [u8; 32] {
    let data = [
        0x00,
        0x01,
        (year >> 8) as u8,
        (year & 0xFF) as u8,
        month,
        day,
        hour,
        minute,
        second,
        weekday,
    ];
    build_packet(0x38, &data)
}

/// Weather display command (0xFE).
/// Temperatures are encoded as 16-bit signed (bit 15 = negative, value = degrees * 10).
pub fn set_weather(icon: WeatherIcon, current: f32, max: f32, min: f32) -> [u8; 32] {
    let curr = encode_temperature(current);
    let max_t = encode_temperature(max);
    let min_t = encode_temperature(min);
    let data = [
        0x00,
        icon as u8,
        curr[0],
        curr[1],
        max_t[0],
        max_t[1],
        min_t[0],
        min_t[1],
    ];
    build_packet(0xFE, &data)
}

/// Theme command (0xFD).
/// Sets background color, font color (both RGB565), and theme ID.
pub fn set_theme(bg_rgb565: u16, font_rgb565: u16, theme: ThemeId) -> [u8; 32] {
    let data = [
        0x00,
        (bg_rgb565 >> 8) as u8,
        (bg_rgb565 & 0xFF) as u8,
        (font_rgb565 >> 8) as u8,
        (font_rgb565 & 0xFF) as u8,
        theme as u8,
    ];
    build_packet(0xFD, &data)
}

/// Display navigation command (0x39).
pub fn screen_nav(action: NavAction) -> [u8; 32] {
    build_packet(0x39, &[0x00, action as u8])
}

/// Display reset step 1 (0x34).
pub fn display_reset_1() -> [u8; 32] {
    build_packet(0x34, &[0x00, 0x01])
}

/// Display reset step 2 (0xFB).
pub fn display_reset_2() -> [u8; 32] {
    build_packet(0xFB, &[0x00])
}

/// System data command (0xFF).
/// Sends CPU temp, GPU temp, SSD temp, fan speed, and net speed in a single packet.
/// Format reverse-engineered from MeletrixID V3.3.4 + device testing.
///
/// Layout (data bytes): [0x00, 0x00, cpu, 0x00, gpu, 0x00, ssd, fan_hi, fan_lo, net_hi, net_lo]
/// - CPU, GPU, SSD: u8 temperatures (°C), separated by 0x00
/// - Fan: u16 BE (raw RPM, displayed as-is by firmware)
/// - Net: u16 BE (firmware divides by 10 to get Mbps)
///
/// Note: MeletrixID hardcodes 0xFF as the checksum byte. Our build_packet computes
/// a proper checksum instead, which the firmware also accepts.
pub fn set_system_info(
    cpu_temp: u8,
    gpu_temp: u8,
    ssd_temp: u8,
    fan_speed: u16,
    net_speed: u16,
) -> [u8; 32] {
    let data = [
        0x00,
        0x00,
        cpu_temp,
        0x00,
        gpu_temp,
        0x00,
        ssd_temp,
        (fan_speed >> 8) as u8,
        (fan_speed & 0xFF) as u8,
        (net_speed >> 8) as u8,
        (net_speed & 0xFF) as u8,
    ];
    build_packet(0xFF, &data)
}

/// Image upload start command (0xFC).
pub fn image_upload_start() -> [u8; 32] {
    build_packet(0xFC, &[0x00, 0xFF, 0xFF])
}

/// Build a raw image data chunk packet for upload.
/// Each chunk is wrapped in the standard 32-byte TIGA protocol packet.
/// The command byte for image data is 0xFC.
/// Returns None if the chunk is too large for the packet.
pub fn image_data_chunk(chunk_index: u16, data: &[u8]) -> Option<[u8; 32]> {
    // Maximum data in a chunk: 32 - 8 (header) - 4 (cmd overhead) - 1 (checksum) = 19 bytes
    // But we also need 2 bytes for chunk index, so: 32 - 13 - 2 = 17 bytes max
    // Actually let's compute based on the protocol:
    // bytes 0-7: header/CRC, 8: magic, 9: cmd, 10: 0x00, 11: data_len
    // 12..12+data_len: data, then checksum
    // Total data area: 32 - 12 - 1(checksum) = 19 bytes for data
    // We use 2 bytes for chunk index + actual pixel data
    let max_data = 17; // 19 - 2 for chunk index
    if data.len() > max_data {
        return None;
    }

    let mut payload = Vec::with_capacity(2 + data.len());
    payload.push((chunk_index >> 8) as u8);
    payload.push((chunk_index & 0xFF) as u8);
    payload.extend_from_slice(data);

    Some(build_packet(0xFC, &payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_time_packet() {
        let pkt = set_time(2026, 2, 7, 16, 30, 0, 6); // Saturday
        assert_eq!(pkt[8], 0xA5);
        assert_eq!(pkt[9], 0x38);
        assert_eq!(pkt[11], 10); // data_len
        // Year 2026 = 0x07EA
        assert_eq!(pkt[14], 0x07);
        assert_eq!(pkt[15], 0xEA);
        // Month, day, hour, min, sec, weekday
        assert_eq!(pkt[16], 2);
        assert_eq!(pkt[17], 7);
        assert_eq!(pkt[18], 16);
        assert_eq!(pkt[19], 30);
        assert_eq!(pkt[20], 0);
        assert_eq!(pkt[21], 6);
    }

    #[test]
    fn test_set_weather_packet() {
        let pkt = set_weather(WeatherIcon::Sunny, 22.0, 25.0, 18.0);
        assert_eq!(pkt[9], 0xFE);
        assert_eq!(pkt[11], 8); // data_len
        assert_eq!(pkt[13], WeatherIcon::Sunny as u8); // icon
    }

    #[test]
    fn test_screen_nav_packet() {
        let pkt = screen_nav(NavAction::Switch);
        assert_eq!(pkt[9], 0x39);
        assert_eq!(pkt[13], NavAction::Switch as u8);
    }

    #[test]
    fn test_display_reset_packets() {
        let pkt1 = display_reset_1();
        assert_eq!(pkt1[9], 0x34);
        assert_eq!(pkt1[12], 0x00);
        assert_eq!(pkt1[13], 0x01);

        let pkt2 = display_reset_2();
        assert_eq!(pkt2[9], 0xFB);
        assert_eq!(pkt2[12], 0x00);
    }

    #[test]
    fn test_set_system_info_packet() {
        let pkt = set_system_info(42, 77, 35, 1350, 500);
        assert_eq!(pkt[8], 0xA5);
        assert_eq!(pkt[9], 0xFF);  // cmd
        assert_eq!(pkt[10], 0x00);
        assert_eq!(pkt[11], 11);   // data_len = 11
        assert_eq!(pkt[12], 0x00); // padding
        assert_eq!(pkt[13], 0x00); // padding
        assert_eq!(pkt[14], 42);   // CPU temp
        assert_eq!(pkt[15], 0x00); // separator
        assert_eq!(pkt[16], 77);   // GPU temp
        assert_eq!(pkt[17], 0x00); // separator
        assert_eq!(pkt[18], 35);   // SSD temp
        assert_eq!(pkt[19], 5);    // fan_speed >> 8 (1350 = 0x0546)
        assert_eq!(pkt[20], 70);   // fan_speed & 0xFF
        assert_eq!(pkt[21], 1);    // net_speed >> 8 (500 = 0x01F4)
        assert_eq!(pkt[22], 244);  // net_speed & 0xFF
    }

    #[test]
    fn test_image_upload_start_packet() {
        let pkt = image_upload_start();
        assert_eq!(pkt[9], 0xFC);
        assert_eq!(pkt[12], 0x00);
        assert_eq!(pkt[13], 0xFF);
        assert_eq!(pkt[14], 0xFF);
    }
}
