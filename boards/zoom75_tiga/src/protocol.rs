//! TIGA packet protocol: 32-byte HID packets with CRC-CCITT and XOR checksum.
//!
//! Packet structure (32 bytes):
//! ```text
//! [0]    = 0x1C (28)       — header
//! [1]    = 0x02            — sub-type
//! [2-4]  = 0x00            — reserved
//! [5]    = 4 + data_len + 1 — payload length
//! [6]    = CRC-CCITT low byte
//! [7]    = CRC-CCITT high byte
//! [8]    = 0xA5 (165)      — magic marker
//! [9]    = command_id
//! [10]   = 0x00
//! [11]   = data_len
//! [12..] = command data
//! [12+n] = checksum
//! ```

/// Build a 32-byte TIGA protocol packet for the given command and data.
pub fn build_packet(cmd_id: u8, data: &[u8]) -> [u8; 32] {
    let data_len = data.len() as u8;
    let mut buf = [0u8; 32];

    // Magic marker and command header (bytes 8-11)
    buf[8] = 0xA5;
    buf[9] = cmd_id;
    buf[10] = 0x00;
    buf[11] = data_len;

    // Copy command data (bytes 12..)
    let data_end = 12 + data.len();
    buf[12..data_end].copy_from_slice(data);

    // Compute and insert XOR checksum after the data
    let checksum_val = xor_checksum(&buf[9..data_end]);
    buf[data_end] = checksum_val;

    // Header fields
    buf[0] = 0x1C;
    buf[1] = 0x02;
    // bytes [2..5] remain 0x00 (reserved)
    buf[5] = 4 + data_len + 1; // payload length: cmd_id + 0x00 + data_len + data + checksum

    // Compute CRC-CCITT over the entire 32-byte buffer (with CRC bytes as 0)
    let crc = crc_ccitt(&buf);
    buf[6] = (crc & 0xFF) as u8;        // CRC low byte
    buf[7] = ((crc >> 8) & 0xFF) as u8;  // CRC high byte

    buf
}

/// CRC-CCITT: polynomial 0x1021, initial value 0xFFFF.
/// Computed over the full 32-byte buffer.
pub fn crc_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
            crc &= 0xFFFF;
        }
    }
    crc
}

/// XOR checksum: sum bytes [9..end], AND 0xFF, XOR 0xFF.
pub fn xor_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    sum ^ 0xFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_ccitt_known_value() {
        // CRC-CCITT of "123456789" should be 0x29B1
        let data = b"123456789";
        let crc = crc_ccitt(data);
        assert_eq!(crc, 0x29B1, "CRC-CCITT of '123456789' should be 0x29B1");
    }

    #[test]
    fn test_xor_checksum() {
        // checksum of [0x38, 0x00, 0x0A, ...data...] should be (sum & 0xFF) ^ 0xFF
        let data: &[u8] = &[0x38, 0x00, 0x0A, 0x00, 0x01, 0x07, 0xEA, 0x02, 0x07, 0x10, 0x1E, 0x00, 0x05];
        let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        let expected = sum ^ 0xFF;
        assert_eq!(xor_checksum(data), expected);
    }

    #[test]
    fn test_build_packet_structure() {
        let pkt = build_packet(0x38, &[0, 1, 0x07, 0xEA, 2, 7, 16, 30, 0, 5]);

        // Header
        assert_eq!(pkt[0], 0x1C);
        assert_eq!(pkt[1], 0x02);

        // Reserved
        assert_eq!(pkt[2], 0x00);
        assert_eq!(pkt[3], 0x00);
        assert_eq!(pkt[4], 0x00);

        // Payload length: 4 + 10 + 1 = 15
        assert_eq!(pkt[5], 15);

        // Magic marker
        assert_eq!(pkt[8], 0xA5);

        // Command
        assert_eq!(pkt[9], 0x38);
        assert_eq!(pkt[10], 0x00);

        // Data length
        assert_eq!(pkt[11], 10);

        // Data
        assert_eq!(&pkt[12..22], &[0, 1, 0x07, 0xEA, 2, 7, 16, 30, 0, 5]);

        // Checksum at position 22
        let expected_checksum = xor_checksum(&pkt[9..22]);
        assert_eq!(pkt[22], expected_checksum);

        // CRC bytes should be non-zero (computed over full buffer)
        assert!(pkt[6] != 0 || pkt[7] != 0, "CRC should be non-zero for a non-trivial packet");
    }

    #[test]
    fn test_build_packet_crc_consistency() {
        let pkt = build_packet(0xFE, &[0, 1, 0, 22, 0, 25, 0, 18]);

        // Verify CRC: recompute with CRC bytes zeroed, should match stored CRC
        let mut verify = pkt;
        verify[6] = 0;
        verify[7] = 0;
        let crc = crc_ccitt(&verify);
        assert_eq!(pkt[6], (crc & 0xFF) as u8);
        assert_eq!(pkt[7], ((crc >> 8) & 0xFF) as u8);
    }

    #[test]
    fn test_empty_data_packet() {
        let pkt = build_packet(0xFB, &[0]);

        assert_eq!(pkt[0], 0x1C);
        assert_eq!(pkt[5], 4 + 1 + 1); // 4 + data_len(1) + checksum(1) = 6
        assert_eq!(pkt[8], 0xA5);
        assert_eq!(pkt[9], 0xFB);
        assert_eq!(pkt[11], 1);
    }
}
