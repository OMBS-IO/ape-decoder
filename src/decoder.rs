use std::io::{Read, Seek, SeekFrom};

use crate::bitreader::BitReader;
use crate::crc::ape_crc;
use crate::entropy::EntropyState;
use crate::error::{ApeError, ApeResult};
use crate::format::{self, ApeFileInfo};
use crate::predictor::{Predictor3950, Predictor3950_32};
use crate::range_coder::RangeCoder;
use crate::unprepare;

// Special frame codes (from Prepare.h)
const SPECIAL_FRAME_MONO_SILENCE: i32 = 1;
const SPECIAL_FRAME_LEFT_SILENCE: i32 = 1;
const SPECIAL_FRAME_RIGHT_SILENCE: i32 = 2;
const SPECIAL_FRAME_PSEUDO_STEREO: i32 = 4;

/// File metadata accessible without decoding.
#[derive(Debug, Clone)]
pub struct ApeInfo {
    pub version: u16,
    pub compression_level: u16,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub total_samples: u64,
    pub total_frames: u32,
    pub blocks_per_frame: u32,
    pub final_frame_blocks: u32,
    pub duration_ms: u64,
    pub block_align: u16,
}

impl ApeInfo {
    fn from_file_info(info: &ApeFileInfo) -> Self {
        ApeInfo {
            version: info.descriptor.version,
            compression_level: info.header.compression_level,
            sample_rate: info.header.sample_rate,
            channels: info.header.channels,
            bits_per_sample: info.header.bits_per_sample,
            total_samples: info.total_blocks as u64,
            total_frames: info.header.total_frames,
            blocks_per_frame: info.header.blocks_per_frame,
            final_frame_blocks: info.header.final_frame_blocks,
            duration_ms: info.length_ms as u64,
            block_align: info.block_align,
        }
    }

    /// Number of samples (blocks) in a given frame.
    pub fn frame_samples(&self, frame_idx: u32) -> u32 {
        if frame_idx == self.total_frames - 1 {
            self.final_frame_blocks
        } else {
            self.blocks_per_frame
        }
    }
}

// ---------------------------------------------------------------------------
// Internal predictor state — either 16-bit or 32-bit path
// ---------------------------------------------------------------------------

enum Predictors {
    Path16(Vec<Predictor3950>),
    Path32(Vec<Predictor3950_32>),
}

/// A streaming APE decoder with seek support.
pub struct ApeDecoder<R: Read + Seek> {
    reader: R,
    file_info: ApeFileInfo,
    info: ApeInfo,
    predictors: Predictors,
    entropy_states: Vec<EntropyState>,
    range_coder: RangeCoder,
    interim_mode: bool,
}

impl<R: Read + Seek> ApeDecoder<R> {
    /// Open an APE file and parse its header.
    pub fn new(mut reader: R) -> ApeResult<Self> {
        let file_info = format::parse(&mut reader)?;
        let version = file_info.descriptor.version as i32;
        let channels = file_info.header.channels;
        let bits = file_info.header.bits_per_sample;
        let compression = file_info.header.compression_level as u32;

        if version < 3950 {
            return Err(ApeError::UnsupportedVersion(file_info.descriptor.version));
        }

        let predictors = if bits >= 32 {
            Predictors::Path32(
                (0..channels)
                    .map(|_| Predictor3950_32::new(compression, version))
                    .collect(),
            )
        } else {
            Predictors::Path16(
                (0..channels)
                    .map(|_| Predictor3950::new(compression, version, bits))
                    .collect(),
            )
        };

        let entropy_states = (0..channels).map(|_| EntropyState::new()).collect();
        let info = ApeInfo::from_file_info(&file_info);

        Ok(ApeDecoder {
            reader,
            file_info,
            info,
            predictors,
            entropy_states,
            range_coder: RangeCoder::new(),
            interim_mode: false,
        })
    }

    /// Get file metadata.
    pub fn info(&self) -> &ApeInfo {
        &self.info
    }

    /// Total number of frames in the file.
    pub fn total_frames(&self) -> u32 {
        self.info.total_frames
    }

    /// Decode a single frame by index, returning raw PCM bytes.
    pub fn decode_frame(&mut self, frame_idx: u32) -> ApeResult<Vec<u8>> {
        if frame_idx >= self.info.total_frames {
            return Err(ApeError::DecodingError("frame index out of bounds"));
        }

        let frame_data = self.read_frame_data(frame_idx)?;
        let seek_remainder = self.seek_remainder(frame_idx);
        let frame_blocks = self.file_info.frame_block_count(frame_idx) as usize;
        let version = self.info.version as i32;
        let channels = self.info.channels;
        let bits = self.info.bits_per_sample;
        let block_align = self.info.block_align as usize;

        match &mut self.predictors {
            Predictors::Path16(predictors) => {
                let result = try_decode_frame_16(
                    &frame_data,
                    seek_remainder,
                    frame_blocks,
                    version,
                    channels,
                    bits,
                    block_align,
                    predictors,
                    &mut self.entropy_states,
                    &mut self.range_coder,
                );

                match result {
                    Ok(pcm) => Ok(pcm),
                    Err(ApeError::InvalidChecksum) if bits == 24 && !self.interim_mode => {
                        self.interim_mode = true;
                        for p in predictors.iter_mut() {
                            p.set_interim_mode(true);
                        }
                        try_decode_frame_16(
                            &frame_data,
                            seek_remainder,
                            frame_blocks,
                            version,
                            channels,
                            bits,
                            block_align,
                            predictors,
                            &mut self.entropy_states,
                            &mut self.range_coder,
                        )
                    }
                    Err(e) => Err(e),
                }
            }
            Predictors::Path32(predictors) => try_decode_frame_32(
                &frame_data,
                seek_remainder,
                frame_blocks,
                version,
                channels,
                bits,
                block_align,
                predictors,
                &mut self.entropy_states,
                &mut self.range_coder,
            ),
        }
    }

    /// Decode all frames, returning all PCM bytes.
    pub fn decode_all(&mut self) -> ApeResult<Vec<u8>> {
        let total_pcm_bytes = self.info.total_samples as usize * self.info.block_align as usize;
        let mut pcm_output = Vec::with_capacity(total_pcm_bytes);

        for frame_idx in 0..self.info.total_frames {
            let frame_pcm = self.decode_frame(frame_idx)?;
            pcm_output.extend_from_slice(&frame_pcm);
        }

        Ok(pcm_output)
    }

    /// Seek to a specific sample position. Returns the actual sample position
    /// (snapped to frame boundary, since frames are independently decodable).
    pub fn seek(&mut self, sample: u64) -> ApeResult<u64> {
        if self.info.total_frames == 0 {
            return Ok(0);
        }
        let frame_idx =
            (sample / self.info.blocks_per_frame as u64).min(self.info.total_frames as u64 - 1);
        let actual_sample = frame_idx * self.info.blocks_per_frame as u64;
        Ok(actual_sample)
    }

    /// Returns an iterator over decoded frames.
    pub fn frames(&mut self) -> FrameIterator<'_, R> {
        FrameIterator {
            decoder: self,
            current_frame: 0,
        }
    }

    // -- Internal helpers --

    fn seek_remainder(&self, frame_idx: u32) -> u32 {
        let seek_byte = self.file_info.seek_byte(frame_idx);
        let seek_byte_0 = self.file_info.seek_byte(0);
        ((seek_byte - seek_byte_0) % 4) as u32
    }

    fn read_frame_data(&mut self, frame_idx: u32) -> ApeResult<Vec<u8>> {
        let seek_byte = self.file_info.seek_byte(frame_idx);
        let seek_remainder = self.seek_remainder(frame_idx);
        let frame_bytes = self.file_info.frame_byte_count(frame_idx);
        let read_bytes = (frame_bytes as u32 + seek_remainder + 4) as usize;

        self.reader
            .seek(SeekFrom::Start(seek_byte - seek_remainder as u64))?;
        let mut frame_data = vec![0u8; read_bytes];
        let bytes_read = self.reader.read(&mut frame_data)?;
        if bytes_read < read_bytes.saturating_sub(4) {
            return Err(ApeError::DecodingError("short read on frame data"));
        }
        frame_data.truncate(bytes_read);
        Ok(frame_data)
    }
}

/// Iterator that yields decoded frames as raw PCM bytes.
pub struct FrameIterator<'a, R: Read + Seek> {
    decoder: &'a mut ApeDecoder<R>,
    current_frame: u32,
}

impl<'a, R: Read + Seek> Iterator for FrameIterator<'a, R> {
    type Item = ApeResult<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.decoder.info.total_frames {
            return None;
        }
        let frame_idx = self.current_frame;
        self.current_frame += 1;
        Some(self.decoder.decode_frame(frame_idx))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.decoder.info.total_frames - self.current_frame) as usize;
        (remaining, Some(remaining))
    }
}

/// Convenience: decode an entire APE file to raw PCM bytes.
pub fn decode<R: Read + Seek>(reader: &mut R) -> ApeResult<Vec<u8>> {
    // ApeDecoder::new takes ownership, so we need to pass a reference wrapper
    // that implements Read + Seek. Since reader is &mut R where R: Read + Seek,
    // we can use it directly because &mut R also implements Read + Seek.
    let mut decoder = ApeDecoder::new_from_ref(reader)?;
    decoder.decode_all()
}

impl<R: Read + Seek> ApeDecoder<R> {
    fn new_from_ref<'a>(reader: &'a mut R) -> ApeResult<ApeDecoder<&'a mut R>> {
        ApeDecoder::new(reader)
    }
}

// ---------------------------------------------------------------------------
// Frame decode implementations (shared between owned and borrowed paths)
// ---------------------------------------------------------------------------

fn try_decode_frame_16(
    frame_data: &[u8],
    seek_remainder: u32,
    frame_blocks: usize,
    version: i32,
    channels: u16,
    bits: u16,
    block_align: usize,
    predictors: &mut [Predictor3950],
    entropy_states: &mut [EntropyState],
    range_coder: &mut RangeCoder,
) -> ApeResult<Vec<u8>> {
    let mut br = BitReader::from_frame_bytes(frame_data, seek_remainder * 8);

    // --- StartFrame ---
    let mut stored_crc = br.decode_value_x_bits(32);
    let mut special_codes: i32 = 0;
    if version > 3820 {
        if stored_crc & 0x80000000 != 0 {
            special_codes = br.decode_value_x_bits(32) as i32;
        }
        stored_crc &= 0x7FFFFFFF;
    }

    for p in predictors.iter_mut() {
        p.flush();
    }
    for s in entropy_states.iter_mut() {
        s.flush();
    }
    range_coder.flush_bit_array(&mut br);

    let mut last_x: i32 = 0;
    let mut pcm_output = Vec::with_capacity(frame_blocks * block_align);

    let decode_result: ApeResult<()> = (|| {
        if channels == 2 {
            if (special_codes & SPECIAL_FRAME_LEFT_SILENCE) != 0
                && (special_codes & SPECIAL_FRAME_RIGHT_SILENCE) != 0
            {
                for _ in 0..frame_blocks {
                    unprepare::unprepare(&[0, 0], channels, bits, &mut pcm_output)?;
                }
            } else if (special_codes & SPECIAL_FRAME_PSEUDO_STEREO) != 0 {
                for _ in 0..frame_blocks {
                    let val = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                    let x = predictors[0].decompress_value(val, 0);
                    unprepare::unprepare(&[x, 0], channels, bits, &mut pcm_output)?;
                }
            } else if version >= 3950 {
                for _ in 0..frame_blocks {
                    let ny = entropy_states[1].decode_value_range(range_coder, &mut br)?;
                    let nx = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                    let y = predictors[1].decompress_value(ny, last_x as i64);
                    let x = predictors[0].decompress_value(nx, y as i64);
                    last_x = x;
                    unprepare::unprepare(&[x, y], channels, bits, &mut pcm_output)?;
                }
            } else {
                for _ in 0..frame_blocks {
                    let ex = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                    let ey = entropy_states[1].decode_value_range(range_coder, &mut br)?;
                    let x = predictors[0].decompress_value(ex, 0);
                    let y = predictors[1].decompress_value(ey, 0);
                    unprepare::unprepare(&[x, y], channels, bits, &mut pcm_output)?;
                }
            }
        } else if channels == 1 {
            if (special_codes & SPECIAL_FRAME_MONO_SILENCE) != 0 {
                for _ in 0..frame_blocks {
                    unprepare::unprepare(&[0], channels, bits, &mut pcm_output)?;
                }
            } else {
                for _ in 0..frame_blocks {
                    let val = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                    let decoded = predictors[0].decompress_value(val, 0);
                    unprepare::unprepare(&[decoded], channels, bits, &mut pcm_output)?;
                }
            }
        } else {
            let ch = channels as usize;
            let mut values = vec![0i32; ch];
            for _ in 0..frame_blocks {
                for c in 0..ch {
                    let val = entropy_states[c].decode_value_range(range_coder, &mut br)?;
                    values[c] = predictors[c].decompress_value(val, 0);
                }
                unprepare::unprepare(&values, channels, bits, &mut pcm_output)?;
            }
        }
        Ok(())
    })();

    decode_result?;

    // --- EndFrame ---
    range_coder.finalize(&mut br);
    let computed_crc = ape_crc(&pcm_output);
    if computed_crc != stored_crc {
        return Err(ApeError::InvalidChecksum);
    }

    Ok(pcm_output)
}

fn try_decode_frame_32(
    frame_data: &[u8],
    seek_remainder: u32,
    frame_blocks: usize,
    version: i32,
    channels: u16,
    bits: u16,
    block_align: usize,
    predictors: &mut [Predictor3950_32],
    entropy_states: &mut [EntropyState],
    range_coder: &mut RangeCoder,
) -> ApeResult<Vec<u8>> {
    let mut br = BitReader::from_frame_bytes(frame_data, seek_remainder * 8);

    let mut stored_crc = br.decode_value_x_bits(32);
    let mut special_codes: i32 = 0;
    if version > 3820 {
        if stored_crc & 0x80000000 != 0 {
            special_codes = br.decode_value_x_bits(32) as i32;
        }
        stored_crc &= 0x7FFFFFFF;
    }

    for p in predictors.iter_mut() {
        p.flush();
    }
    for s in entropy_states.iter_mut() {
        s.flush();
    }
    range_coder.flush_bit_array(&mut br);

    let mut last_x: i64 = 0;
    let mut pcm_output = Vec::with_capacity(frame_blocks * block_align);

    if channels == 2 {
        if (special_codes & SPECIAL_FRAME_LEFT_SILENCE) != 0
            && (special_codes & SPECIAL_FRAME_RIGHT_SILENCE) != 0
        {
            for _ in 0..frame_blocks {
                unprepare::unprepare(&[0, 0], channels, bits, &mut pcm_output)?;
            }
        } else if (special_codes & SPECIAL_FRAME_PSEUDO_STEREO) != 0 {
            for _ in 0..frame_blocks {
                let val = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                let x = predictors[0].decompress_value(val, 0);
                unprepare::unprepare(&[x as i32, 0], channels, bits, &mut pcm_output)?;
            }
        } else {
            for _ in 0..frame_blocks {
                let ny = entropy_states[1].decode_value_range(range_coder, &mut br)?;
                let nx = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                let y = predictors[1].decompress_value(ny, last_x);
                let x = predictors[0].decompress_value(nx, y as i64);
                last_x = x as i64;
                unprepare::unprepare(&[x as i32, y as i32], channels, bits, &mut pcm_output)?;
            }
        }
    } else if channels == 1 {
        if (special_codes & SPECIAL_FRAME_MONO_SILENCE) != 0 {
            for _ in 0..frame_blocks {
                unprepare::unprepare(&[0], channels, bits, &mut pcm_output)?;
            }
        } else {
            for _ in 0..frame_blocks {
                let val = entropy_states[0].decode_value_range(range_coder, &mut br)?;
                let decoded = predictors[0].decompress_value(val, 0);
                unprepare::unprepare(&[decoded as i32], channels, bits, &mut pcm_output)?;
            }
        }
    }

    range_coder.finalize(&mut br);
    let computed_crc = ape_crc(&pcm_output);
    if computed_crc != stored_crc {
        return Err(ApeError::InvalidChecksum);
    }

    Ok(pcm_output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;

    fn test_fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn load_reference_pcm(name: &str) -> Vec<u8> {
        let path = test_fixture_path(&format!("ref/{}", name));
        let data = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        data[44..].to_vec()
    }

    fn open_ape(name: &str) -> BufReader<File> {
        let path = test_fixture_path(&format!("ape/{}", name));
        let file = File::open(&path)
            .unwrap_or_else(|e| panic!("Failed to open {}: {}", path.display(), e));
        BufReader::new(file)
    }

    fn decode_ape_file(name: &str) -> ApeResult<Vec<u8>> {
        let mut reader = open_ape(name);
        decode(&mut reader)
    }

    // --- Existing end-to-end tests (unchanged) ---

    #[test]
    fn test_decode_sine_16s_c1000() {
        let decoded = decode_ape_file("sine_16s_c1000.ape").unwrap();
        let expected = load_reference_pcm("sine_16s_c1000.wav");
        assert_eq!(decoded.len(), expected.len());
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_sine_16s_c2000() {
        let decoded = decode_ape_file("sine_16s_c2000.ape").unwrap();
        let expected = load_reference_pcm("sine_16s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_silence_16s() {
        let decoded = decode_ape_file("silence_16s_c2000.ape").unwrap();
        let expected = load_reference_pcm("silence_16s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_sine_16m() {
        let decoded = decode_ape_file("sine_16m_c2000.ape").unwrap();
        let expected = load_reference_pcm("sine_16m_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_short_16s() {
        let decoded = decode_ape_file("short_16s_c2000.ape").unwrap();
        let expected = load_reference_pcm("short_16s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_all_compression_levels() {
        for level in &["c1000", "c2000", "c3000", "c4000", "c5000"] {
            let name = format!("sine_16s_{}.ape", level);
            let ref_name = format!("sine_16s_{}.wav", level);
            let decoded = decode_ape_file(&name).unwrap_or_else(|e| panic!("{}: {:?}", name, e));
            let expected = load_reference_pcm(&ref_name);
            assert_eq!(decoded, expected, "Mismatch for {}", name);
        }
    }

    #[test]
    fn test_decode_8bit() {
        let decoded = decode_ape_file("sine_8s_c2000.ape").unwrap();
        let expected = load_reference_pcm("sine_8s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_24bit() {
        let decoded = decode_ape_file("sine_24s_c2000.ape").unwrap();
        let expected = load_reference_pcm("sine_24s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_32bit() {
        let decoded = decode_ape_file("sine_32s_c2000.ape").unwrap();
        let expected = load_reference_pcm("sine_32s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_multiframe() {
        let decoded = decode_ape_file("multiframe_16s_c2000.ape").unwrap();
        let expected = load_reference_pcm("multiframe_16s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_identical_channels() {
        let decoded = decode_ape_file("identical_16s_c2000.ape").unwrap();
        let expected = load_reference_pcm("identical_16s_c2000.wav");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_decode_all_fixtures() {
        let fixtures = [
            "dc_offset_16s_c2000.ape",
            "identical_16s_c2000.ape",
            "impulse_16s_c2000.ape",
            "left_only_16s_c2000.ape",
            "multiframe_16s_c2000.ape",
            "noise_16s_c2000.ape",
            "short_16s_c2000.ape",
            "silence_16s_c2000.ape",
            "sine_16m_c2000.ape",
            "sine_16s_c1000.ape",
            "sine_16s_c2000.ape",
            "sine_16s_c3000.ape",
            "sine_16s_c4000.ape",
            "sine_16s_c5000.ape",
            "sine_24s_c2000.ape",
            "sine_32s_c2000.ape",
            "sine_8s_c2000.ape",
        ];

        for fixture in &fixtures {
            let ref_name = fixture.replace(".ape", ".wav");
            let decoded = decode_ape_file(fixture)
                .unwrap_or_else(|e| panic!("Failed to decode {}: {:?}", fixture, e));
            let expected = load_reference_pcm(&ref_name);
            assert_eq!(
                decoded.len(),
                expected.len(),
                "Length mismatch for {}",
                fixture
            );
            assert_eq!(decoded, expected, "Data mismatch for {}", fixture);
        }
    }

    // --- New streaming API tests ---

    #[test]
    fn test_ape_decoder_info() {
        let reader = open_ape("sine_16s_c2000.ape");
        let decoder = ApeDecoder::new(reader).unwrap();
        let info = decoder.info();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(info.total_samples, 44100);
        assert_eq!(info.compression_level, 2000);
        assert_eq!(info.block_align, 4);
    }

    #[test]
    fn test_decode_frame_by_frame() {
        let reader = open_ape("sine_16s_c2000.ape");
        let mut decoder = ApeDecoder::new(reader).unwrap();
        let expected = load_reference_pcm("sine_16s_c2000.wav");

        let mut all_pcm = Vec::new();
        for frame_idx in 0..decoder.total_frames() {
            let frame_pcm = decoder.decode_frame(frame_idx).unwrap();
            all_pcm.extend_from_slice(&frame_pcm);
        }

        assert_eq!(all_pcm, expected);
    }

    #[test]
    fn test_decode_multiframe_frame_by_frame() {
        let reader = open_ape("multiframe_16s_c2000.ape");
        let mut decoder = ApeDecoder::new(reader).unwrap();
        let expected = load_reference_pcm("multiframe_16s_c2000.wav");

        assert!(decoder.total_frames() > 1, "Expected multiple frames");

        let mut all_pcm = Vec::new();
        for frame_idx in 0..decoder.total_frames() {
            let frame_pcm = decoder.decode_frame(frame_idx).unwrap();
            assert!(!frame_pcm.is_empty());
            all_pcm.extend_from_slice(&frame_pcm);
        }

        assert_eq!(all_pcm, expected);
    }

    #[test]
    fn test_frames_iterator() {
        let reader = open_ape("sine_16s_c2000.ape");
        let mut decoder = ApeDecoder::new(reader).unwrap();
        let expected = load_reference_pcm("sine_16s_c2000.wav");

        let all_pcm: Vec<u8> = decoder
            .frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .concat();

        assert_eq!(all_pcm, expected);
    }

    #[test]
    fn test_seek_snaps_to_frame() {
        let reader = open_ape("multiframe_16s_c2000.ape");
        let mut decoder = ApeDecoder::new(reader).unwrap();
        let bpf = decoder.info().blocks_per_frame as u64;

        // Seek to sample 0 → frame 0
        assert_eq!(decoder.seek(0).unwrap(), 0);

        // Seek to sample in middle of first frame → snaps to frame 0
        assert_eq!(decoder.seek(100).unwrap(), 0);

        // Seek to exactly frame 1
        assert_eq!(decoder.seek(bpf).unwrap(), bpf);

        // Seek to middle of frame 1 → snaps to frame 1
        assert_eq!(decoder.seek(bpf + 100).unwrap(), bpf);

        // Seek past end → snaps to last frame
        let last_frame_start = (decoder.total_frames() as u64 - 1) * bpf;
        assert_eq!(decoder.seek(u64::MAX).unwrap(), last_frame_start);
    }

    #[test]
    fn test_decode_frame_out_of_bounds() {
        let reader = open_ape("sine_16s_c2000.ape");
        let mut decoder = ApeDecoder::new(reader).unwrap();
        let result = decoder.decode_frame(999);
        assert!(result.is_err());
    }
}
