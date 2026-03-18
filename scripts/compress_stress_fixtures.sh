#!/bin/bash
# Compress stress-test WAVs to APE at all 5 compression levels
# and create reference WAVs by round-tripping through the C++ codec.
set -euo pipefail

MAC="${MAC:-/home/johns/repos/ape/sdk/mac}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIXTURE_DIR="$SCRIPT_DIR/../tests/fixtures"
WAV_DIR="$FIXTURE_DIR/wav"
APE_DIR="$FIXTURE_DIR/ape"
REF_DIR="$FIXTURE_DIR/ref"

SIGNALS=(chirp_16s multitone_16s transient_16s fade_16s square_16s intermod_16s)
LEVELS=(c1000 c2000 c3000 c4000 c5000)

echo "=== Phase 1: Compress WAV -> APE ==="
for signal in "${SIGNALS[@]}"; do
    for level in "${LEVELS[@]}"; do
        src="$WAV_DIR/${signal}.wav"
        dst="$APE_DIR/${signal}_${level}.ape"
        echo "  $signal @ $level"
        "$MAC" "$src" "$dst" "-${level}" 2>/dev/null
    done
done

echo ""
echo "=== Phase 2: Verify APE files ==="
failures=0
for signal in "${SIGNALS[@]}"; do
    for level in "${LEVELS[@]}"; do
        ape="$APE_DIR/${signal}_${level}.ape"
        if ! "$MAC" "$ape" -V 2>/dev/null; then
            echo "  FAIL: $ape"
            failures=$((failures + 1))
        fi
    done
done

if [ "$failures" -gt 0 ]; then
    echo "ERROR: $failures verification failures"
    exit 1
fi
echo "  All verified OK"

echo ""
echo "=== Phase 3: Create reference WAVs (decompress c2000) ==="
for signal in "${SIGNALS[@]}"; do
    ape="$APE_DIR/${signal}_c2000.ape"
    ref="$REF_DIR/${signal}_c2000.wav"
    "$MAC" "$ape" "$ref" -d 2>/dev/null
    echo "  $signal -> ref"
done

echo ""
echo "=== Phase 4: Round-trip validation ==="
passed=0
failed=0
for signal in "${SIGNALS[@]}"; do
    src="$WAV_DIR/${signal}.wav"
    ref="$REF_DIR/${signal}_c2000.wav"
    if cmp -s "$src" "$ref"; then
        passed=$((passed + 1))
    else
        echo "  MISMATCH: $signal"
        failed=$((failed + 1))
    fi
done

echo ""
echo "=== Summary ==="
echo "APE files: $((${#SIGNALS[@]} * ${#LEVELS[@]}))"
echo "Reference WAVs: ${#SIGNALS[@]}"
echo "Round-trip: $passed passed, $failed failed"

if [ "$failed" -gt 0 ]; then
    exit 1
fi
