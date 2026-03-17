/// Bit array reader for Monkey's Audio compressed frame data.
///
/// Converts raw frame bytes into a u32 word array (little-endian) and provides
/// bit-level extraction methods used by the range coder and entropy decoder.

const POWERS_OF_TWO_MINUS_ONE: [u32; 33] = [
    0, 1, 3, 7, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767, 65535, 131071,
    262143, 524287, 1048575, 2097151, 4194303, 8388607, 16777215, 33554431, 67108863, 134217727,
    268435455, 536870911, 1073741823, 2147483647, 4294967295,
];

pub struct BitReader {
    words: Vec<u32>,
    bit_index: u32,
}

impl BitReader {
    /// Create a `BitReader` from raw frame bytes, converting to little-endian u32 words.
    ///
    /// `skip_bits` sets the initial bit index (typically 0).
    pub fn from_frame_bytes(raw: &[u8], skip_bits: u32) -> Self {
        let words: Vec<u32> = raw
            .chunks(4)
            .map(|chunk| {
                let mut buf = [0u8; 4];
                buf[..chunk.len()].copy_from_slice(chunk);
                u32::from_le_bytes(buf)
            })
            .collect();
        BitReader {
            words,
            bit_index: skip_bits,
        }
    }

    /// Extract one byte from the u32 word array at the current bit position.
    ///
    /// Bytes are read in big-endian order within each 32-bit word (MSB first),
    /// which corresponds to reversed file-byte order due to LE u32 interpretation.
    #[inline]
    pub fn decode_byte(&mut self) -> u32 {
        let word_idx = (self.bit_index >> 5) as usize;
        let bit_off = self.bit_index & 31;
        self.bit_index = self.bit_index.wrapping_add(8);
        (self.words[word_idx] >> (24 - bit_off)) & 0xFF
    }

    /// Read `n` raw bits from the bit array, handling word boundary splits.
    ///
    /// Uses the `POWERS_OF_TWO_MINUS_ONE` lookup table for masking.
    #[inline]
    pub fn decode_value_x_bits(&mut self, n: u32) -> u32 {
        let left_bits = 32 - (self.bit_index & 31);
        let word_idx = (self.bit_index >> 5) as usize;
        self.bit_index = self.bit_index.wrapping_add(n);

        if left_bits >= n {
            // Value fits within current word
            (self.words[word_idx] & POWERS_OF_TWO_MINUS_ONE[left_bits as usize]) >> (left_bits - n)
        } else {
            // Split across two words
            let right_bits = n - left_bits;
            let left_value =
                (self.words[word_idx] & POWERS_OF_TWO_MINUS_ONE[left_bits as usize]) << right_bits;
            let right_value = self.words[word_idx + 1] >> (32 - right_bits);
            left_value | right_value
        }
    }

    /// Advance the bit index to the next byte boundary.
    #[inline]
    pub fn advance_to_byte_boundary(&mut self) {
        let remainder = self.bit_index % 8;
        if remainder != 0 {
            self.bit_index += 8 - remainder;
        }
    }

    /// Advance the bit index by `n` bits without reading.
    #[inline]
    pub fn advance(&mut self, n: u32) {
        self.bit_index = self.bit_index.wrapping_add(n);
    }

    /// Return the current bit index.
    #[inline]
    pub fn bit_index(&self) -> u32 {
        self.bit_index
    }
}
