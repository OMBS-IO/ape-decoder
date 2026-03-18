#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    if let Ok(mut decoder) = ape_decoder::ApeDecoder::new(cursor) {
        let _ = decoder.decode_all();
    }
});
