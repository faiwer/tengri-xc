pub struct BitWriter {
    bytes: Vec<u8>,
    bit_len: u8,
}

pub struct BitReader<'a> {
    bytes: &'a [u8],
    bit_idx: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_len: 0,
        }
    }

    pub fn push_signed(&mut self, value: i16, width: u8) {
        let mask = (1u16 << width) - 1;
        self.push_bits((value as u16) & mask, width);
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn push_bits(&mut self, value: u16, width: u8) {
        for shift in (0..width).rev() {
            if self.bit_len == 0 {
                self.bytes.push(0);
            }

            let bit = ((value >> shift) & 1) as u8;
            let last = self.bytes.len() - 1;
            self.bytes[last] |= bit << (7 - self.bit_len);
            self.bit_len = (self.bit_len + 1) % 8;
        }
    }
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit_idx: 0 }
    }

    pub fn read_signed(&mut self, width: u8) -> Option<i16> {
        let value = self.read_bits(width)?;
        let sign_bit = 1u16 << (width - 1);
        if value & sign_bit == 0 {
            Some(value as i16)
        } else {
            Some((i32::from(value) - (1i32 << width)) as i16)
        }
    }

    fn read_bits(&mut self, width: u8) -> Option<u16> {
        let mut value = 0u16;
        for _ in 0..width {
            let byte = *self.bytes.get(self.bit_idx / 8)?;
            let shift = 7 - (self.bit_idx % 8);
            value = (value << 1) | u16::from((byte >> shift) & 1);
            self.bit_idx += 1;
        }
        Some(value)
    }
}
