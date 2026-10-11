// src/crypto/poly1305.rs
//! Poly1305 one-time authenticator according to RFC 8439.

pub struct Poly1305 {
    r0: u32,
    r1: u32,
    r2: u32,
    r3: u32,
    r4: u32,
    s1: u32,
    s2: u32,
    s3: u32,
    s4: u32,
    h0: u32,
    h1: u32,
    h2: u32,
    h3: u32,
    h4: u32,
    pad: [u8; 16],
    buffer: [u8; 16],
    buffer_len: usize,
}

impl Poly1305 {
    pub fn new(key: &[u8; 32]) -> Self {
        // Clamp r
        let mut r = [0u8; 16];
        r.copy_from_slice(&key[0..16]);
        r[3] &= 15;
        r[7] &= 15;
        r[11] &= 15;
        r[15] &= 15;
        r[4] &= 252;
        r[8] &= 252;
        r[12] &= 252;

        let r0 = (u32::from_le_bytes(r[0..4].try_into().unwrap())) & 0x3ffffff;
        let r1 = (u32::from_le_bytes(r[3..7].try_into().unwrap()) >> 2) & 0x3ffff03;
        let r2 = (u32::from_le_bytes(r[6..10].try_into().unwrap()) >> 4) & 0x3ffc0ff;
        let r3 = (u32::from_le_bytes(r[9..13].try_into().unwrap()) >> 6) & 0x3f03fff;
        let r4 = (u32::from_le_bytes(r[12..16].try_into().unwrap()) >> 8) & 0x00fffff;

        let s1 = r1 * 5;
        let s2 = r2 * 5;
        let s3 = r3 * 5;
        let s4 = r4 * 5;

        let mut pad = [0u8; 16];
        pad.copy_from_slice(&key[16..32]);

        Self {
            r0,
            r1,
            r2,
            r3,
            r4,
            s1,
            s2,
            s3,
            s4,
            h0: 0,
            h1: 0,
            h2: 0,
            h3: 0,
            h4: 0,
            pad,
            buffer: [0u8; 16],
            buffer_len: 0,
        }
    }

    fn process_block(&mut self, block: &[u8], _is_final_partial: bool) {
        let mut b = [0u8; 17];
        b[..block.len()].copy_from_slice(block);
        b[block.len()] = 1; // 0x01 delimiter

        let m0 = (u32::from_le_bytes(b[0..4].try_into().unwrap())) & 0x3ffffff;
        let m1 = (u32::from_le_bytes(b[3..7].try_into().unwrap()) >> 2) & 0x3ffffff;
        let m2 = (u32::from_le_bytes(b[6..10].try_into().unwrap()) >> 4) & 0x3ffffff;
        let m3 = (u32::from_le_bytes(b[9..13].try_into().unwrap()) >> 6) & 0x3ffffff;
        let m4 = (u32::from_le_bytes(b[12..16].try_into().unwrap()) >> 8) | ((b[16] as u32) << 24);

        let h0 = self.h0 as u64 + m0 as u64;
        let h1 = self.h1 as u64 + m1 as u64;
        let h2 = self.h2 as u64 + m2 as u64;
        let h3 = self.h3 as u64 + m3 as u64;
        let h4 = self.h4 as u64 + m4 as u64;

        let r0 = self.r0 as u64;
        let r1 = self.r1 as u64;
        let r2 = self.r2 as u64;
        let r3 = self.r3 as u64;
        let r4 = self.r4 as u64;

        let s1 = self.s1 as u64;
        let s2 = self.s2 as u64;
        let s3 = self.s3 as u64;
        let s4 = self.s4 as u64;

        let d0 = h0 * r0 + h1 * s4 + h2 * s3 + h3 * s2 + h4 * s1;
        let mut d1 = h0 * r1 + h1 * r0 + h2 * s4 + h3 * s3 + h4 * s2;
        let mut d2 = h0 * r2 + h1 * r1 + h2 * r0 + h3 * s4 + h4 * s3;
        let mut d3 = h0 * r3 + h1 * r2 + h2 * r1 + h3 * r0 + h4 * s4;
        let mut d4 = h0 * r4 + h1 * r3 + h2 * r2 + h3 * r1 + h4 * r0;

        let mut c: u64;
        c = d0 >> 26;
        self.h0 = (d0 & 0x3ffffff) as u32;
        d1 += c;
        c = d1 >> 26;
        self.h1 = (d1 & 0x3ffffff) as u32;
        d2 += c;
        c = d2 >> 26;
        self.h2 = (d2 & 0x3ffffff) as u32;
        d3 += c;
        c = d3 >> 26;
        self.h3 = (d3 & 0x3ffffff) as u32;
        d4 += c;
        c = d4 >> 26;
        self.h4 = (d4 & 0x3ffffff) as u32;

        self.h0 += (c * 5) as u32;
        c = (self.h0 >> 26) as u64;
        self.h0 &= 0x3ffffff;
        self.h1 += c as u32;
    }

    pub fn update(&mut self, mut data: &[u8]) {
        if self.buffer_len > 0 {
            let needed = 16 - self.buffer_len;
            if data.len() >= needed {
                self.buffer[self.buffer_len..16].copy_from_slice(&data[..needed]);
                let buf = self.buffer;
                self.process_block(&buf, false);
                data = &data[needed..];
                self.buffer_len = 0;
            } else {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
        }

        while data.len() >= 16 {
            self.process_block(&data[..16], false);
            data = &data[16..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    pub fn finalize(mut self) -> [u8; 16] {
        if self.buffer_len > 0 {
            let len = self.buffer_len;
            let block = self.buffer[..len].to_vec();
            self.process_block(&block, true);
        }

        // Final reduction mod 2^130 - 5
        let mut c = (self.h1 >> 26) as u64;
        self.h1 &= 0x3ffffff;
        self.h2 += c as u32;
        c = (self.h2 >> 26) as u64;
        self.h2 &= 0x3ffffff;
        self.h3 += c as u32;
        c = (self.h3 >> 26) as u64;
        self.h3 &= 0x3ffffff;
        self.h4 += c as u32;
        c = (self.h4 >> 26) as u64;
        self.h4 &= 0x3ffffff;
        self.h0 += (c * 5) as u32;
        c = (self.h0 >> 26) as u64;
        self.h0 &= 0x3ffffff;
        self.h1 += c as u32;

        // Compute h - (2^130 - 5)
        let mut g0 = self.h0.wrapping_add(5);
        c = (g0 >> 26) as u64;
        g0 &= 0x3ffffff;
        let mut g1 = self.h1.wrapping_add(c as u32);
        c = (g1 >> 26) as u64;
        g1 &= 0x3ffffff;
        let mut g2 = self.h2.wrapping_add(c as u32);
        c = (g2 >> 26) as u64;
        g2 &= 0x3ffffff;
        let mut g3 = self.h3.wrapping_add(c as u32);
        c = (g3 >> 26) as u64;
        g3 &= 0x3ffffff;
        let g4 = self.h4.wrapping_add(c as u32).wrapping_sub(1 << 26);

        let mask = (g4 >> 31).wrapping_sub(1);
        self.h0 = (self.h0 & !mask) | (g0 & mask);
        self.h1 = (self.h1 & !mask) | (g1 & mask);
        self.h2 = (self.h2 & !mask) | (g2 & mask);
        self.h3 = (self.h3 & !mask) | (g3 & mask);
        self.h4 = (self.h4 & !mask) | (g4 & mask);

        // Convert back to 16 bytes: h0 | (h1 << 26) | (h2 << 52) ...
        let f0 = (self.h0 as u64) | ((self.h1 as u64) << 26) | ((self.h2 as u64) << 52);
        let f1 = ((self.h2 as u64) >> 12) | ((self.h3 as u64) << 14) | ((self.h4 as u64) << 40);

        let pad0 = u64::from_le_bytes(self.pad[0..8].try_into().unwrap());
        let pad1 = u64::from_le_bytes(self.pad[8..16].try_into().unwrap());

        let res0 = f0 as u128 + pad0 as u128;
        let res1 = f1 as u128 + pad1 as u128 + (res0 >> 64);

        let mut tag = [0u8; 16];
        tag[0..8].copy_from_slice(&(res0 as u64).to_le_bytes());
        tag[8..16].copy_from_slice(&(res1 as u64).to_le_bytes());
        tag
    }
}

/// Convenience one-shot Poly1305 MAC calculation.
pub fn poly1305_mac(key: &[u8; 32], message: &[u8]) -> [u8; 16] {
    let mut poly = Poly1305::new(key);
    poly.update(message);
    poly.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 8439 Section 2.5.2 Test Vector
    #[test]
    fn test_rfc8439_section2_5_2() {
        let key_bytes =
            from_hex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b").unwrap();
        let key: &[u8; 32] = key_bytes.as_slice().try_into().unwrap();
        let msg = b"Cryptographic Forum Research Group";

        let tag = poly1305_mac(key, msg);
        let expected_tag = "a8061dc1305136c6c22b8baf0c0127a9";
        assert_eq!(to_hex(&tag), expected_tag);
    }
}
