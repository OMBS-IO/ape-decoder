//! Integration tests: stress signals at all compression levels.
//!
//! These signals are designed to exercise the adaptive NN filter and predictor
//! more aggressively than the simple sine/silence/noise fixtures.

use std::fs::File;
use std::io::BufReader;

use ape_decoder::ApeDecoder;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn load_reference_pcm(name: &str) -> Vec<u8> {
    let data = std::fs::read(fixture_path(&format!("ref/{}", name))).unwrap();
    // Strip 44-byte WAV header to get raw PCM
    data[44..].to_vec()
}

fn decode_fixture(name: &str) -> Vec<u8> {
    let file = File::open(fixture_path(&format!("ape/{}", name))).unwrap();
    let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();
    decoder.decode_all().unwrap()
}

const STRESS_SIGNALS: &[&str] = &[
    "chirp_16s",
    "multitone_16s",
    "transient_16s",
    "fade_16s",
    "square_16s",
    "intermod_16s",
];

const LEVELS: &[&str] = &["c1000", "c2000", "c3000", "c4000", "c5000"];

/// Decode all stress signals at all compression levels and compare byte-for-byte.
#[test]
fn test_stress_signals_all_levels() {
    for signal in STRESS_SIGNALS {
        let ref_pcm = load_reference_pcm(&format!("{}_c2000.wav", signal));
        for level in LEVELS {
            let ape_name = format!("{}_{}.ape", signal, level);
            let decoded = decode_fixture(&ape_name);
            assert_eq!(
                decoded.len(),
                ref_pcm.len(),
                "{} at {}: length mismatch ({} vs {})",
                signal,
                level,
                decoded.len(),
                ref_pcm.len()
            );
            assert!(
                decoded == ref_pcm,
                "{} at {}: PCM data mismatch",
                signal,
                level
            );
        }
    }
}

/// Parallel decode must produce identical output.
#[test]
fn test_stress_signals_parallel_decode() {
    for signal in STRESS_SIGNALS {
        let ref_pcm = load_reference_pcm(&format!("{}_c2000.wav", signal));
        let ape_name = format!("{}_c3000.ape", signal);
        let file = File::open(fixture_path(&format!("ape/{}", ape_name))).unwrap();
        let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();
        let decoded = decoder.decode_all_parallel(2).unwrap();
        assert_eq!(
            decoded, ref_pcm,
            "{}: parallel decode mismatch at c3000",
            signal
        );
    }
}

/// Frame-by-frame decode must concatenate to the same PCM.
#[test]
fn test_stress_signals_frame_by_frame() {
    for signal in STRESS_SIGNALS {
        let ref_pcm = load_reference_pcm(&format!("{}_c2000.wav", signal));
        let ape_name = format!("{}_c2000.ape", signal);
        let file = File::open(fixture_path(&format!("ape/{}", ape_name))).unwrap();
        let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();
        let mut all = Vec::new();
        for i in 0..decoder.total_frames() {
            all.extend_from_slice(&decoder.decode_frame(i).unwrap());
        }
        assert_eq!(
            all, ref_pcm,
            "{}: frame-by-frame mismatch at c2000",
            signal
        );
    }
}

/// MD5 verification must pass for all stress signal fixtures.
#[test]
fn test_stress_signals_md5() {
    for signal in STRESS_SIGNALS {
        for level in LEVELS {
            let ape_name = format!("{}_{}.ape", signal, level);
            let file = File::open(fixture_path(&format!("ape/{}", ape_name))).unwrap();
            let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();
            assert!(
                decoder.verify_md5().unwrap(),
                "{} at {}: MD5 verification failed",
                signal,
                level
            );
        }
    }
}
