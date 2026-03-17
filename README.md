# ape-decoder

Pure Rust decoder for [Monkey's Audio](https://monkeysaudio.com/) (APE) lossless audio files.

## Features

- Decodes APE files to raw PCM audio
- Supports all compression levels (Fast, Normal, High, Extra High, Insane)
- Supports 8, 16, 24, and 32-bit audio
- Supports mono, stereo, and multichannel
- No unsafe code
- Single dependency (`crc32fast`)

## Usage

### Simple (decode entire file)

```rust
use std::fs::File;
use std::io::BufReader;

let file = File::open("audio.ape").unwrap();
let mut reader = BufReader::new(file);

// Decode to raw PCM bytes (little-endian, interleaved)
let pcm_data = ape_decoder::decode(&mut reader).unwrap();
```

### Streaming (frame-by-frame)

```rust
use ape_decoder::ApeDecoder;
use std::fs::File;
use std::io::BufReader;

let file = File::open("audio.ape").unwrap();
let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();

// Access metadata
let info = decoder.info();
println!("{}Hz, {}ch, {}-bit, {} samples",
    info.sample_rate, info.channels, info.bits_per_sample, info.total_samples);

// Decode frame by frame
for frame_result in decoder.frames() {
    let pcm_bytes = frame_result.unwrap();
    // process pcm_bytes...
}

// Or decode a specific frame
let frame_0 = decoder.decode_frame(0).unwrap();
```

## Supported Formats

| Bit Depth | Channels | Status |
|-----------|----------|--------|
| 8-bit | Mono/Stereo | Supported |
| 16-bit | Mono/Stereo/Multichannel | Supported |
| 24-bit | Mono/Stereo/Multichannel | Supported |
| 32-bit | Mono/Stereo | Supported |

All five compression levels are supported: Fast (1000), Normal (2000), High (3000),
Extra High (4000), and Insane (5000).

## Limitations

- Decode only (no encoder)
- Requires APE file version >= 3950 (files created by Monkey's Audio 3.95 or later)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Acknowledgments

Based on the [Monkey's Audio SDK](https://monkeysaudio.com/) by Matthew T. Ashland,
licensed under the 3-clause BSD license.
