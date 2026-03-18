#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let frame_idx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let file_data = &data[4..];

    let cursor = Cursor::new(file_data);
    if let Ok(mut decoder) = ape_decoder::ApeDecoder::new(cursor) {
        if decoder.total_frames() > 0 {
            let idx = frame_idx % decoder.total_frames();
            let _ = decoder.decode_frame(idx);
        }
    }
});
