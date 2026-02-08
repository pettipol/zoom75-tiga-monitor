//! HID protocol implementation for the Zoom75 TIGA display module.
//!
//! The TIGA uses 32-byte HID packets with CRC-CCITT and XOR checksum,
//! communicating over Usage Page 0xFF60 / Usage 0x61.

use std::sync::{LazyLock, RwLock};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use hidapi::{HidApi, HidDevice};
use types::{NavAction, ScreenPosition, WeatherIcon};
use zoom_sync_core::{
    Board, BoardError, BoardInfo, HasImage, HasScreen, HasScreenSize, HasSystemInfo, HasTime,
    HasWeather, Result, ScreenGroup, ScreenPosition as CoreScreenPosition,
};

pub mod abi;
pub mod protocol;
pub mod types;

pub mod consts {
    pub const TIGA_VENDOR_ID: u16 = 0x1EA7;
    pub const TIGA_PRODUCT_ID: u16 = 0xCEDD;
    pub const TIGA_USAGE_PAGE: u16 = 0xFF60;
    pub const TIGA_USAGE: u16 = 0x61;
}

/// Static board info for detection
pub static INFO: BoardInfo = BoardInfo {
    name: "Zoom75 TIGA",
    cli_name: "zoom75-tiga",
    vendor_id: consts::TIGA_VENDOR_ID,
    product_id: consts::TIGA_PRODUCT_ID,
    usage_page: Some(consts::TIGA_USAGE_PAGE),
    usage: Some(consts::TIGA_USAGE),
};

/// Screen positions for this board
pub static SCREEN_POSITIONS: &[CoreScreenPosition] = &[
    CoreScreenPosition {
        id: "home",
        display_name: "Home",
        group: ScreenGroup::Logo,
    },
    CoreScreenPosition {
        id: "time",
        display_name: "Time",
        group: ScreenGroup::Time,
    },
    CoreScreenPosition {
        id: "weather",
        display_name: "Weather",
        group: ScreenGroup::Time,
    },
    CoreScreenPosition {
        id: "image",
        display_name: "Image",
        group: ScreenGroup::Logo,
    },
];

/// Screen dimensions: 320x172 px
pub const SCREEN_WIDTH: u32 = 320;
pub const SCREEN_HEIGHT: u32 = 172;

/// Expected image size in bytes: 320 * 172 * 2 (RGB565, 16-bit per pixel)
pub const IMAGE_SIZE: usize = (SCREEN_WIDTH as usize) * (SCREEN_HEIGHT as usize) * 2;

/// Lazy handle to hidapi
static API: LazyLock<RwLock<HidApi>> =
    LazyLock::new(|| RwLock::new(HidApi::new().expect("failed to init hidapi")));

/// High level abstraction for managing a Zoom75 TIGA keyboard display
pub struct Zoom75Tiga {
    pub device: HidDevice,
    buf: [u8; 32],
}

impl Zoom75Tiga {
    /// Find and open the TIGA device for modifications
    pub fn open() -> Result<Self> {
        API.write().unwrap().refresh_devices()?;
        let api = API.read().unwrap();
        let this = Self {
            device: api
                .device_list()
                .find(|d| {
                    d.vendor_id() == consts::TIGA_VENDOR_ID
                        && d.product_id() == consts::TIGA_PRODUCT_ID
                        && d.usage_page() == consts::TIGA_USAGE_PAGE
                        && d.usage() == consts::TIGA_USAGE
                })
                .ok_or(BoardError::DeviceNotFound)?
                .open_device(&api)?,
            buf: [0u8; 32],
        };

        Ok(this)
    }

    /// Internal method to send a 32-byte packet and read the response.
    /// Uses a 500ms read timeout so Ctrl+C cleanup is never blocked.
    fn execute(&mut self, payload: [u8; 32]) -> Result<Vec<u8>> {
        self.device.write(&payload)?;
        let len = self.device.read_timeout(&mut self.buf, 500)?;
        Ok(self.buf[..len].to_vec())
    }

    /// Set the display time.
    /// Year is full 16-bit (e.g. 2026). Weekday: 0=Sunday .. 6=Saturday.
    pub fn set_time<Tz: TimeZone>(&mut self, time: DateTime<Tz>, _12hr: bool) -> Result<()> {
        let hour = if _12hr { time.hour12().1 } else { time.hour() } as u8;
        // chrono weekday: Mon=0 .. Sun=6. TIGA likely uses Sun=0 .. Sat=6.
        let weekday = time.weekday().num_days_from_sunday() as u8;

        let pkt = abi::set_time(
            time.year() as u16,
            time.month() as u8,
            time.day() as u8,
            hour,
            time.minute() as u8,
            time.second() as u8,
            weekday,
        );
        self.execute(pkt)?;
        Ok(())
    }

    /// Set the weather display.
    pub fn set_weather(
        &mut self,
        icon: WeatherIcon,
        current: f32,
        max: f32,
        min: f32,
    ) -> Result<()> {
        let pkt = abi::set_weather(icon, current, max, min);
        self.execute(pkt)?;
        Ok(())
    }

    /// Navigate the display.
    pub fn screen_nav(&mut self, action: NavAction) -> Result<()> {
        let pkt = abi::screen_nav(action);
        self.execute(pkt)?;
        Ok(())
    }

    /// Reset display to default screen (two-step reset with 100ms delay).
    pub fn reset_screen(&mut self) -> Result<()> {
        self.execute(abi::display_reset_1())?;
        thread::sleep(Duration::from_millis(100));
        self.execute(abi::display_reset_2())?;
        Ok(())
    }

    /// Navigate to a specific screen position from home.
    pub fn set_screen(&mut self, position: ScreenPosition) -> Result<()> {
        let (down, switches) = position.to_directions();

        // Return to home first
        self.reset_screen()?;

        // Navigate down
        for _ in 0..down {
            self.screen_nav(NavAction::Down)?;
        }

        // Switch horizontally
        for _ in 0..switches {
            self.screen_nav(NavAction::Switch)?;
        }

        Ok(())
    }

    /// Upload a raw RGB565 image (320x172, 110080 bytes) to the display.
    pub fn upload_image(&mut self, data: &[u8], cb: &mut dyn FnMut(usize)) -> Result<()> {
        if data.len() != IMAGE_SIZE {
            return Err(BoardError::MediaTooLarge(
                "image must be exactly 110080 bytes (320x172 RGB565)",
            ));
        }

        // Send upload start command
        self.execute(abi::image_upload_start())?;

        // Send image data in chunks
        // Max data per packet: 17 bytes (19 available in data area - 2 for chunk index)
        let chunk_size = 17;
        for (i, chunk) in data.chunks(chunk_size).enumerate() {
            cb(i);
            let pkt = abi::image_data_chunk(i as u16, chunk)
                .ok_or(BoardError::CommandFailed("chunk too large"))?;

            // Send and wait for ACK
            let res = self.execute(pkt)?;

            // The device should respond with a packet containing 0xFC to ACK
            if res.first().copied() != Some(0x1C) {
                return Err(BoardError::CommandFailed("device did not acknowledge chunk"));
            }
        }

        Ok(())
    }

    /// Clear the uploaded image.
    pub fn clear_image(&mut self) -> Result<()> {
        self.reset_screen()
    }
}

// === Trait Implementations ===

impl Board for Zoom75Tiga {
    fn info(&self) -> &'static BoardInfo {
        &INFO
    }

    fn as_time(&mut self) -> Option<&mut dyn HasTime> {
        Some(self)
    }

    fn as_weather(&mut self) -> Option<&mut dyn HasWeather> {
        Some(self)
    }

    fn as_system_info(&mut self) -> Option<&mut dyn HasSystemInfo> {
        Some(self)
    }

    fn as_screen(&mut self) -> Option<&mut dyn HasScreen> {
        Some(self)
    }

    fn as_screen_size(&self) -> Option<(u32, u32)> {
        Some((SCREEN_WIDTH, SCREEN_HEIGHT))
    }

    fn as_image(&mut self) -> Option<&mut dyn HasImage> {
        Some(self)
    }
}

impl HasTime for Zoom75Tiga {
    fn set_time(&mut self, time: DateTime<Local>, use_12hr: bool) -> Result<()> {
        Zoom75Tiga::set_time(self, time, use_12hr)
    }
}

impl HasWeather for Zoom75Tiga {
    fn set_weather(
        &mut self,
        wmo: u8,
        is_day: bool,
        current: u8,
        low: u8,
        high: u8,
    ) -> Result<()> {
        let icon = WeatherIcon::from_wmo(wmo, is_day)
            .ok_or(BoardError::CommandFailed("unknown WMO code"))?;
        // The core trait passes temperatures as u8 (integer degrees).
        // Convert to f32 for the TIGA protocol which uses fixed-point * 10.
        Zoom75Tiga::set_weather(self, icon, current as f32, high as f32, low as f32)
    }
}

impl HasSystemInfo for Zoom75Tiga {
    fn set_system_info(&mut self, cpu: u8, gpu: u8, download: f32) -> Result<()> {
        // Map the core trait's download (Mbps) to TIGA's net_speed u16 (firmware divides by 10)
        let net_speed = (download * 10.0).round().min(65535.0) as u16;
        let pkt = abi::set_system_info(cpu, gpu, 0, 0, net_speed);
        self.execute(pkt)?;
        Ok(())
    }
}

impl HasScreen for Zoom75Tiga {
    fn screen_positions(&self) -> &'static [CoreScreenPosition] {
        SCREEN_POSITIONS
    }

    fn set_screen(&mut self, id: &str) -> Result<()> {
        let pos: ScreenPosition = id
            .parse()
            .map_err(BoardError::InvalidScreenPosition)?;
        Zoom75Tiga::set_screen(self, pos)
    }

    fn screen_up(&mut self) -> Result<()> {
        // TIGA uses "return" for going back/up
        self.screen_nav(NavAction::Return)
    }

    fn screen_down(&mut self) -> Result<()> {
        self.screen_nav(NavAction::Down)
    }

    fn screen_switch(&mut self) -> Result<()> {
        self.screen_nav(NavAction::Switch)
    }

    fn reset_screen(&mut self) -> Result<()> {
        Zoom75Tiga::reset_screen(self)
    }
}

impl HasScreenSize for Zoom75Tiga {
    fn screen_size(&self) -> (u32, u32) {
        (SCREEN_WIDTH, SCREEN_HEIGHT)
    }
}

impl HasImage for Zoom75Tiga {
    fn upload_image(&mut self, data: &[u8], progress: &mut dyn FnMut(usize)) -> Result<()> {
        Zoom75Tiga::upload_image(self, data, progress)
    }

    fn clear_image(&mut self) -> Result<()> {
        Zoom75Tiga::clear_image(self)
    }
}
