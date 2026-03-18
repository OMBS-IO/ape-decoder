#!/usr/bin/env python3
"""Generate stress-test WAV files for APE decoder integration tests.

Signals are designed to exercise the adaptive NN filter and predictor
more aggressively than simple sine/silence/noise patterns.
"""

import math
import os
import random
import struct
import wave

RATE = 44100
DURATION = 1.0
NFRAMES = int(RATE * DURATION)
OUTPUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tests', 'fixtures', 'wav')


def clamp16(x):
    return max(-32768, min(32767, int(x)))


def write_stereo_wav(name, samples_l, samples_r):
    """Write 16-bit stereo WAV."""
    path = os.path.join(OUTPUT_DIR, name)
    with wave.open(path, 'w') as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(RATE)
        frames = b''
        for l, r in zip(samples_l, samples_r):
            frames += struct.pack('<hh', clamp16(l), clamp16(r))
        w.writeframes(frames)
    print(f"  wrote {name} ({len(samples_l)} samples)")


def gen_chirp():
    """Linear frequency sweep 20 Hz to 20 kHz. Forces constant NN filter re-adaptation."""
    f0, f1 = 20.0, 20000.0
    amp = 30000
    left = []
    right = []
    for i in range(NFRAMES):
        t = i / RATE
        # Instantaneous frequency increases linearly
        freq = f0 + (f1 - f0) * (t / DURATION)
        phase = 2 * math.pi * (f0 * t + (f1 - f0) * t * t / (2 * DURATION))
        left.append(amp * math.sin(phase))
        right.append(amp * math.sin(phase + math.pi / 4))  # phase offset for stereo
    write_stereo_wav('chirp_16s.wav', left, right)


def gen_multitone():
    """Sum of 7 inharmonic frequencies at varying amplitudes. Complex beating patterns."""
    freqs = [100, 317, 1003, 3167, 7919, 12853, 18097]
    amps = [4000, 3500, 3000, 2500, 2000, 1500, 1000]
    left = []
    right = []
    for i in range(NFRAMES):
        t = i / RATE
        l = sum(a * math.sin(2 * math.pi * f * t) for f, a in zip(freqs, amps))
        r = sum(a * math.sin(2 * math.pi * f * t + f * 0.001) for f, a in zip(freqs, amps))
        left.append(l)
        right.append(r)
    write_stereo_wav('multitone_16s.wav', left, right)


def gen_transient():
    """Silence punctuated by short noise bursts. Tests predictor transient response."""
    rng = random.Random(42)
    left = [0.0] * NFRAMES
    right = [0.0] * NFRAMES

    # Place 10 noise bursts at random positions
    burst_len = int(0.01 * RATE)  # 10ms
    for _ in range(10):
        start = rng.randint(0, NFRAMES - burst_len - 1)
        amp = rng.uniform(5000, 30000)
        for j in range(burst_len):
            left[start + j] = amp * (rng.random() * 2 - 1)
            right[start + j] = amp * (rng.random() * 2 - 1)

    write_stereo_wav('transient_16s.wav', left, right)


def gen_fade():
    """Exponential fade-in on left, fade-out on right. Asymmetric mid-side stress."""
    freq = 1000.0
    left = []
    right = []
    for i in range(NFRAMES):
        t = i / RATE
        frac = t / DURATION
        # Exponential curves for more gradual change
        l_amp = 30000 * (frac ** 2)
        r_amp = 30000 * ((1 - frac) ** 2)
        s = math.sin(2 * math.pi * freq * t)
        left.append(l_amp * s)
        right.append(r_amp * s)
    write_stereo_wav('fade_16s.wav', left, right)


def gen_square():
    """Band-limited square wave at 440 Hz (sum of odd harmonics)."""
    freq = 440.0
    left = []
    right = []
    # Use first 15 odd harmonics for band-limiting
    for i in range(NFRAMES):
        t = i / RATE
        l = 0.0
        for k in range(1, 30, 2):  # odd harmonics 1,3,5,...,29
            if k * freq > RATE / 2:
                break
            l += math.sin(2 * math.pi * k * freq * t) / k
        l *= 25000 * (4 / math.pi)
        # Right channel slightly detuned for stereo interest
        r = 0.0
        freq_r = 441.0
        for k in range(1, 30, 2):
            if k * freq_r > RATE / 2:
                break
            r += math.sin(2 * math.pi * k * freq_r * t) / k
        r *= 25000 * (4 / math.pi)
        left.append(l)
        right.append(r)
    write_stereo_wav('square_16s.wav', left, right)


def gen_intermod():
    """Two-tone intermodulation: 1000 Hz + 1001 Hz creating 1 Hz beat."""
    amp = 16000
    left = []
    right = []
    for i in range(NFRAMES):
        t = i / RATE
        l = amp * math.sin(2 * math.pi * 1000 * t) + amp * math.sin(2 * math.pi * 1001 * t)
        r = amp * math.sin(2 * math.pi * 1000 * t) - amp * math.sin(2 * math.pi * 1001 * t)
        left.append(l)
        right.append(r)
    write_stereo_wav('intermod_16s.wav', left, right)


if __name__ == '__main__':
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    print("Generating stress-test WAV files...")
    gen_chirp()
    gen_multitone()
    gen_transient()
    gen_fade()
    gen_square()
    gen_intermod()
    print("Done.")
