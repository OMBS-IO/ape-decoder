//! Per-frame CRC validation tests.
//!
//! The decoder validates CRC-32 internally on every `decode_frame()` call.
//! These tests exercise that path explicitly and verify corruption detection.

use std::fs::File;
use std::io::{BufReader, Cursor};

use ape_decoder::ApeDecoder;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

const ALL_FIXTURES: &[&str] = &[
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

/// Every frame of every fixture must decode without CRC error.
#[test]
fn test_every_frame_crc_passes() {
    for name in ALL_FIXTURES {
        let file = File::open(fixture_path(&format!("ape/{}", name))).unwrap();
        let mut dec = ApeDecoder::new(BufReader::new(file)).unwrap();
        let total = dec.total_frames();
        for i in 0..total {
            dec.decode_frame(i).unwrap_or_else(|e| {
                panic!("{} frame {}/{}: {:?}", name, i, total, e);
            });
        }
    }
}

/// Corrupting a byte in frame data should cause InvalidChecksum or DecodingError.
#[test]
fn test_corrupted_frame_detected() {
    let data = std::fs::read(fixture_path("ape/sine_16s_c2000.ape")).unwrap();

    // Corrupt a byte in the middle of the file (frame data region)
    let mut corrupted = data.clone();
    let corrupt_offset = data.len() / 2;
    corrupted[corrupt_offset] ^= 0xFF;

    let cursor = Cursor::new(corrupted);
    match ApeDecoder::new(cursor) {
        Ok(mut dec) => {
            // At least one frame should fail
            let mut found_error = false;
            for i in 0..dec.total_frames() {
                if dec.decode_frame(i).is_err() {
                    found_error = true;
                    break;
                }
            }
            assert!(found_error, "Expected decode error from corrupted file");
        }
        Err(_) => {
            // Parsing failure is also acceptable for corrupted data
        }
    }
}

/// CRC errors propagate through the FrameIterator.
#[test]
fn test_frame_iterator_crc_passes() {
    let file = File::open(fixture_path("ape/multiframe_16s_c2000.ape")).unwrap();
    let mut dec = ApeDecoder::new(BufReader::new(file)).unwrap();
    for (i, result) in dec.frames().enumerate() {
        assert!(
            result.is_ok(),
            "Frame {} failed: {:?}",
            i,
            result.as_ref().err()
        );
    }
}

/// Corrupt one frame of a multiframe file; verify other frames still decode.
#[test]
fn test_single_frame_corruption_isolated() {
    let data = std::fs::read(fixture_path("ape/multiframe_16s_c2000.ape")).unwrap();

    // Corrupt near the end (likely in the last frame's data)
    let mut corrupted = data.clone();
    let offset = data.len() - 100;
    corrupted[offset] ^= 0xFF;

    let cursor = Cursor::new(corrupted);
    if let Ok(mut dec) = ApeDecoder::new(cursor) {
        let total = dec.total_frames();
        let mut errors = 0;
        let mut successes = 0;
        for i in 0..total {
            match dec.decode_frame(i) {
                Ok(_) => successes += 1,
                Err(_) => errors += 1,
            }
        }
        // With corruption, we expect at least one error and at least one success
        assert!(
            errors >= 1,
            "Expected at least one frame error, got {} successes / {} total",
            successes,
            total
        );
        assert!(
            successes >= 1,
            "Expected at least one successful frame, got {} errors / {} total",
            errors,
            total
        );
    }
}
