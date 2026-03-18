# ape-decoder

Pure Rust decoder for [Monkey's Audio](https://monkeysaudio.com/) (APE) lossless audio files.

## Features

- Decode APE files to raw PCM audio
- All compression levels (Fast, Normal, High, Extra High, Insane)
- All bit depths (8, 16, 24, 32-bit) and channel layouts (mono, stereo, multichannel)
- Streaming frame-by-frame decode with iterator
- Sample-level seeking
- Multi-threaded parallel decoding
- Range decoding (decode a subset of samples)
- Progress callbacks with cancellation
- APEv2 tag read/write/remove
- ID3v2 tag parsing (v2.3 and v2.4)
- MD5 quick verification (no decompression needed)
- WAV header generation for APE-to-WAV export
- No unsafe code

## Quick Start

```rust
use std::fs::File;
use std::io::BufReader;

let file = File::open("audio.ape").unwrap();
let mut reader = BufReader::new(file);

// Decode entire file to raw PCM bytes (little-endian, interleaved)
let pcm_data = ape_decoder::decode(&mut reader).unwrap();
```

## Streaming Decode

```rust
use ape_decoder::ApeDecoder;
use std::fs::File;
use std::io::BufReader;

let file = File::open("audio.ape").unwrap();
let mut decoder = ApeDecoder::new(BufReader::new(file)).unwrap();

// Access metadata
let info = decoder.info();
println!("{}Hz {}ch {}-bit, {} samples, {}ms",
    info.sample_rate, info.channels, info.bits_per_sample,
    info.total_samples, info.duration_ms);

// Decode frame by frame
for frame_result in decoder.frames() {
    let pcm_bytes = frame_result.unwrap();
    // process pcm_bytes...
}
```

## Seeking

```rust
// Seek to a specific sample (returns frame index + skip offset)
let pos = decoder.seek(44100)?; // seek to 1 second
println!("Frame {}, skip {} samples", pos.frame_index, pos.skip_samples);

// Or seek and decode in one call
let pcm_from_1s = decoder.decode_from(44100)?;
```

## Reading Tags

```rust
// APEv2 tags
if let Some(tag) = decoder.read_tag()? {
    println!("Title: {}", tag.title().unwrap_or("Unknown"));
    println!("Artist: {}", tag.artist().unwrap_or("Unknown"));

    // Access any field by name (case-insensitive)
    if let Some(year) = tag.get("Year") {
        println!("Year: {}", year);
    }
}

// ID3v2 tags (if present in file header)
if let Some(id3) = decoder.read_id3v2_tag()? {
    println!("Title: {}", id3.title().unwrap_or_default());
}
```

## Writing Tags

```rust
use ape_decoder::{ApeTag, write_tag};
use std::fs::OpenOptions;

let mut file = OpenOptions::new().read(true).write(true).open("audio.ape")?;

let mut tag = ApeTag::new();
tag.set("Title", "My Song");
tag.set("Artist", "My Band");
tag.set("Album", "My Album");
tag.set("Year", "2026");

write_tag(&mut file, &tag)?;
```

## Parallel Decode

```rust
// Decode using 4 threads (output is byte-identical to single-threaded)
let pcm = decoder.decode_all_parallel(4)?;
```

## Range Decode

```rust
// Decode only samples 44100..88200 (1 second starting at 1s)
let pcm = decoder.decode_range(44100, 88200)?;
```

## Progress Callback

```rust
let pcm = decoder.decode_all_with(|progress| {
    println!("{:.0}%", progress * 100.0);
    true // return false to cancel
})?;
```

## APE to WAV Export

```rust
let header = decoder.wav_header_data()
    .map(|h| h.to_vec())
    .unwrap_or_else(|| decoder.info().generate_wav_header());

let pcm = decoder.decode_all()?;

let mut wav = File::create("output.wav")?;
wav.write_all(&header)?;
wav.write_all(&pcm)?;
```

## MD5 Verification

```rust
// Quick verify without decompressing (checks stored MD5 hash)
if decoder.verify_md5()? {
    println!("File integrity OK");
}
```

## Supported Formats

| Bit Depth | Channels | Status |
|-----------|----------|--------|
| 8-bit | Mono/Stereo | Supported |
| 16-bit | Mono/Stereo/Multichannel | Supported |
| 24-bit | Mono/Stereo/Multichannel | Supported |
| 32-bit | Mono/Stereo | Supported |

All five compression levels: Fast (1000), Normal (2000), High (3000),
Extra High (4000), Insane (5000).

## Limitations

- Decode only (no encoder)
- Requires APE file version >= 3950 (files created by Monkey's Audio 3.95+)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Acknowledgments

Based on the [Monkey's Audio SDK](https://monkeysaudio.com/) by Matthew T. Ashland,
licensed under the 3-clause BSD license.
