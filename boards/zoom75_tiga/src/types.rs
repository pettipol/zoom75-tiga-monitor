//! TIGA-specific type definitions.

use std::str::FromStr;

/// Weather icon IDs for the TIGA display (1-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeatherIcon {
    Sunny = 1,
    PartlyCloudy = 2,
    Cloudy = 3,
    Rainy = 4,
    Thunderstorm = 5,
    Snowy = 6,
    Foggy = 7,
    NightClear = 8,
}

impl WeatherIcon {
    /// Convert a WMO weather code to a TIGA weather icon, adapting for day/night.
    /// WMO codes from <https://open-meteo.com/en/docs>
    pub fn from_wmo(wmo: u8, is_day: bool) -> Option<Self> {
        match wmo {
            // Clear / mainly clear
            0 | 1 => Some(if is_day { Self::Sunny } else { Self::NightClear }),
            // Partly cloudy
            2 => Some(Self::PartlyCloudy),
            // Overcast / fog
            3 | 45 | 48 => Some(if wmo == 3 { Self::Cloudy } else { Self::Foggy }),
            // Drizzle + freezing drizzle + rain + freezing rain
            51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 => Some(Self::Rainy),
            // Rain showers
            80..=82 => Some(Self::Rainy),
            // Snowfall + snow showers
            71 | 73 | 75 | 77 | 85 | 86 => Some(Self::Snowy),
            // Thunderstorm
            95 | 96 | 99 => Some(Self::Thunderstorm),
            _ => None,
        }
    }
}

/// Display navigation actions for command 0x39.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NavAction {
    Down = 2,
    Switch = 3,
    Return = 4,
}

/// Theme IDs for the TIGA display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThemeId {
    Theme1 = 1,
    Theme2 = 2,
    Theme3 = 3,
}

/// Screen positions for the TIGA display.
///
/// The TIGA display navigation uses switch (horizontal) and down (vertical).
/// Exact layout TBD from hardware testing, but we map the known positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenPosition {
    /// Default/home position (e.g., clock face)
    Home,
    /// Time display
    Time,
    /// Weather display
    Weather,
    /// Custom image
    Image,
}

impl Default for ScreenPosition {
    fn default() -> Self {
        Self::Home
    }
}

impl ScreenPosition {
    pub const OPTIONS: &'static str = "[ home, time, weather, image ]";

    /// Convert screen position into navigation directions from home as `[down_count, switch_count]`.
    pub fn to_directions(&self) -> (usize, usize) {
        match self {
            ScreenPosition::Home => (0, 0),
            ScreenPosition::Time => (0, 1),
            ScreenPosition::Weather => (0, 2),
            ScreenPosition::Image => (0, 3),
        }
    }
}

impl FromStr for ScreenPosition {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "home" | "h" => Ok(Self::Home),
            "time" | "t" => Ok(Self::Time),
            "weather" | "w" => Ok(Self::Weather),
            "image" | "i" => Ok(Self::Image),
            _ => Err(format!(
                "invalid screen position, must be one of: {}",
                Self::OPTIONS
            )),
        }
    }
}

/// Encode a temperature value as a 16-bit signed value for the TIGA protocol.
/// The TIGA uses: bit 15 = negative flag, value = degrees * 10.
pub fn encode_temperature(degrees: f32) -> [u8; 2] {
    let negative = degrees < 0.0;
    let raw = (degrees.abs() * 10.0).round() as u16;
    let value = if negative { raw | 0x8000 } else { raw };
    // Big-endian
    [(value >> 8) as u8, (value & 0xFF) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_encoding() {
        // 22.5°C -> 225 -> [0x00, 0xE1]
        assert_eq!(encode_temperature(22.5), [0x00, 0xE1]);

        // -5.0°C -> 50 | 0x8000 -> 0x8032 -> [0x80, 0x32]
        assert_eq!(encode_temperature(-5.0), [0x80, 0x32]);

        // 0.0°C -> 0 -> [0x00, 0x00]
        assert_eq!(encode_temperature(0.0), [0x00, 0x00]);

        // 30.0°C -> 300 -> [0x01, 0x2C]
        assert_eq!(encode_temperature(30.0), [0x01, 0x2C]);
    }

    #[test]
    fn test_weather_icon_from_wmo() {
        assert_eq!(WeatherIcon::from_wmo(0, true), Some(WeatherIcon::Sunny));
        assert_eq!(WeatherIcon::from_wmo(0, false), Some(WeatherIcon::NightClear));
        assert_eq!(WeatherIcon::from_wmo(2, true), Some(WeatherIcon::PartlyCloudy));
        assert_eq!(WeatherIcon::from_wmo(95, true), Some(WeatherIcon::Thunderstorm));
        assert_eq!(WeatherIcon::from_wmo(71, true), Some(WeatherIcon::Snowy));
        assert_eq!(WeatherIcon::from_wmo(200, true), None);
    }

    #[test]
    fn test_screen_position_parse() {
        assert_eq!("home".parse::<ScreenPosition>(), Ok(ScreenPosition::Home));
        assert_eq!("t".parse::<ScreenPosition>(), Ok(ScreenPosition::Time));
        assert_eq!("weather".parse::<ScreenPosition>(), Ok(ScreenPosition::Weather));
        assert!("invalid".parse::<ScreenPosition>().is_err());
    }
}
