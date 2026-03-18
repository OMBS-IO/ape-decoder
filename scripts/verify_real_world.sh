#!/bin/bash
# Verify the Rust APE decoder against the C++ reference decoder.
#
# Usage: ./scripts/verify_real_world.sh <path-to-ape-file>
#
# Decodes the APE file with both decoders and compares raw PCM byte-for-byte.
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <path-to-ape-file>"
    exit 1
fi

APE_FILE="$1"
MAC="${MAC:-/home/johns/repos/ape/sdk/mac}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DECODER_DIR="$SCRIPT_DIR/.."

# Temp files
REF_WAV=$(mktemp /tmp/ape_ref_XXXXXX.wav)
RUST_PCM=$(mktemp /tmp/ape_rust_XXXXXX.pcm)
trap 'rm -f "$REF_WAV" "$RUST_PCM"' EXIT

echo "=== Verifying: $(basename "$APE_FILE") ==="
echo "    Size: $(du -h "$APE_FILE" | cut -f1)"
echo ""

# Step 1: C++ reference decode
echo "Step 1: Decoding with C++ mac..."
time "$MAC" "$APE_FILE" "$REF_WAV" -d
echo ""

# Step 2: Rust decode (raw PCM, no WAV header)
echo "Step 2: Decoding with Rust decoder..."
time cargo run --release --example decode_to_file --manifest-path "$DECODER_DIR/Cargo.toml" -- --raw "$APE_FILE" "$RUST_PCM"
echo ""

# Step 3: Find WAV data chunk offset and size
echo "Step 3: Comparing PCM data..."
DATA_OFFSET=$(python3 -c "
with open('$REF_WAV', 'rb') as f:
    data = f.read(4096)
    idx = data.find(b'data')
    if idx == -1:
        print(-1)
    else:
        import struct
        size = struct.unpack_from('<I', data, idx + 4)[0]
        print(idx + 8)
")

if [ "$DATA_OFFSET" = "-1" ]; then
    echo "ERROR: Could not find 'data' chunk in WAV output"
    exit 1
fi
echo "    WAV PCM starts at byte $DATA_OFFSET"

RUST_SIZE=$(stat -c%s "$RUST_PCM")
REF_TOTAL=$(stat -c%s "$REF_WAV")
echo "    Rust PCM: $RUST_SIZE bytes"
echo "    C++ WAV:  $REF_TOTAL bytes (header: $DATA_OFFSET bytes)"

# Compare using cmp with skip — no need to extract, compares directly
if cmp -s -i "$DATA_OFFSET":0 -n "$RUST_SIZE" "$REF_WAV" "$RUST_PCM"; then
    echo ""
    echo "PASS: PCM output matches byte-for-byte ($RUST_SIZE bytes)"
else
    RESULT=$(cmp -i "$DATA_OFFSET":0 -n "$RUST_SIZE" "$REF_WAV" "$RUST_PCM" 2>&1 | head -1)
    echo ""
    echo "FAIL: $RESULT"
    exit 1
fi
