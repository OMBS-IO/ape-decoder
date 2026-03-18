# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- WAV header generation for decoded APE audio
- APE tag writing and removal
- MD5 file integrity verification
- Parallel multi-threaded decoding
- Range decoding (decode a subset of samples)
- ID3v2 tag parsing (v2.3 and v2.4)
- Progress callbacks with cancellation
- GitHub community templates and guidelines

## [0.1.1] - 2026-03-18

### Added
- Precise sample-level seeking
- File metadata access (sample rate, channels, bit depth, duration)

### Changed
- Improved release script error handling and output

## [0.1.0] - 2026-03-17

### Added
- Initial release
- APE frame decoding for all compression levels (Fast, Normal, High, Extra High, Insane)
- All bit depths (8, 16, 24, 32-bit) and channel layouts (mono, stereo)
- Streaming frame-by-frame decode with iterator
- CI workflow
- Automated release script

[Unreleased]: https://github.com/OMBS-IO/ape-decoder/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/OMBS-IO/ape-decoder/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/OMBS-IO/ape-decoder/releases/tag/v0.1.0
