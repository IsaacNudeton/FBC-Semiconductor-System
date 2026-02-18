#!/usr/bin/env python3
"""
ONETWO Routing Verification — FBC Bitstream
=============================================
Applies the ONETWO methodology to verify the PIP database claim:
"86 entries cover 92% of cases"

The Question:
  Does our PIP database have VALUE-AT-POSITION knowledge?
  Or are we just guessing (flat temporal patterns)?

Test Method:
  1. Parse reference bitstream (ground truth)
  2. Parse our generated bitstream
  3. For each routing frame: did we predict the correct Word 50?

This is exactly the ONETWO engine's VAL|POS metric:
  "When position X is observed, did I know what value was there?"

Isaac & Claude — February 2026
"""

import struct
from pathlib import Path
from collections import defaultdict

# Constants
FRAME_WORDS = 101
ROUTING_WORD = 50
SYNC_WORD = bytes([0xAA, 0x99, 0x55, 0x66])


def decode_far(far):
    """Decode Frame Address Register."""
    return {
        'block_type': (far >> 23) & 0x7,
        'top_bottom': (far >> 22) & 0x1,
        'row': (far >> 17) & 0x1F,
        'column': (far >> 7) & 0x3FF,
        'minor': far & 0x7F,
    }


def parse_bitstream(filepath):
    """Extract Word 50 from each frame."""
    with open(filepath, 'rb') as f:
        data = f.read()

    sync_pos = data.find(SYNC_WORD)
    if sync_pos == -1:
        print(f"No sync word in {filepath}")
        return {}

    pos = sync_pos + 4
    frames = {}
    current_far = 0

    while pos + 4 <= len(data):
        cmd = struct.unpack('>I', data[pos:pos+4])[0]
        pos += 4

        pkt_type = (cmd >> 29) & 0x7

        if pkt_type == 1:
            opcode = (cmd >> 27) & 0x3
            reg = (cmd >> 13) & 0x1F
            words = cmd & 0x7FF

            if opcode == 2 and reg == 1 and words == 1:
                if pos + 4 <= len(data):
                    current_far = struct.unpack('>I', data[pos:pos+4])[0]
                    pos += 4
            else:
                pos += words * 4

        elif pkt_type == 2:
            words = cmd & 0x07FFFFFF
            frame_data = []

            for _ in range(words):
                if pos + 4 <= len(data):
                    word = struct.unpack('>I', data[pos:pos+4])[0]
                    frame_data.append(word)
                    pos += 4

            for i in range(0, len(frame_data), FRAME_WORDS):
                chunk = frame_data[i:i+FRAME_WORDS]
                if len(chunk) == FRAME_WORDS:
                    word50 = chunk[ROUTING_WORD]
                    if word50 != 0:  # Only non-zero routing
                        frames[current_far] = word50
                    current_far += 1

    return frames


def analyze_coverage(ref_patterns, our_patterns):
    """
    ONETWO Analysis:
      - Exact match = we knew the value at that position
      - Mismatch = we guessed wrong
      - Missing = we didn't even try
    """

    exact_matches = 0
    mismatches = 0
    missing = 0
    our_only = 0

    mismatch_details = []

    for far, ref_val in ref_patterns.items():
        decoded = decode_far(far)
        col = decoded['column']
        minor = decoded['minor']

        if far in our_patterns:
            our_val = our_patterns[far]
            if our_val == ref_val:
                exact_matches += 1
            else:
                mismatches += 1
                mismatch_details.append({
                    'far': far,
                    'col': col,
                    'minor': minor,
                    'ref': ref_val,
                    'ours': our_val,
                    'diff': ref_val ^ our_val,
                })
        else:
            missing += 1

    for far in our_patterns:
        if far not in ref_patterns:
            our_only += 1

    return {
        'exact': exact_matches,
        'mismatch': mismatches,
        'missing': missing,
        'our_only': our_only,
        'total_ref': len(ref_patterns),
        'total_ours': len(our_patterns),
        'details': mismatch_details,
    }


def categorize_mismatches(details):
    """
    ONETWO decomposition of mismatches:
      - By column (are certain columns problematic?)
      - By bit pattern (which mux selects are wrong?)
    """

    by_column = defaultdict(list)
    by_bit = defaultdict(int)

    for d in details:
        by_column[d['col']].append(d)
        diff = d['diff']
        for bit in range(32):
            if (diff >> bit) & 1:
                by_bit[bit] += 1

    return by_column, by_bit


def run_verification(ref_path, our_path):
    """Full ONETWO verification."""

    print("=" * 70)
    print("  ONETWO Routing Verification")
    print("  'Does our PIP database have VALUE-AT-POSITION knowledge?'")
    print("=" * 70)

    print(f"\n  Reference: {ref_path}")
    print(f"  Ours:      {our_path}")

    ref = parse_bitstream(ref_path)
    ours = parse_bitstream(our_path)

    print(f"\n  Reference frames with routing: {len(ref)}")
    print(f"  Our frames with routing:       {len(ours)}")

    results = analyze_coverage(ref, ours)

    print("\n" + "-" * 70)
    print("  COVERAGE ANALYSIS")
    print("-" * 70)

    total = results['total_ref']
    exact = results['exact']
    mismatch = results['mismatch']
    missing = results['missing']

    print(f"\n  Exact match:  {exact:5d} ({100*exact/total:.1f}%)")
    print(f"  Mismatch:     {mismatch:5d} ({100*mismatch/total:.1f}%)")
    print(f"  Missing:      {missing:5d} ({100*missing/total:.1f}%)")
    print(f"  Our unique:   {results['our_only']:5d}")

    if mismatch > 0:
        by_col, by_bit = categorize_mismatches(results['details'])

        print("\n" + "-" * 70)
        print("  MISMATCH DECOMPOSITION (ONETWO)")
        print("-" * 70)

        print("\n  By column (top 10):")
        for col in sorted(by_col.keys(), key=lambda c: -len(by_col[c]))[:10]:
            count = len(by_col[col])
            print(f"    Column {col:3d}: {count} mismatches")

        print("\n  By bit (which mux selects are wrong):")
        for bit in sorted(by_bit.keys(), key=lambda b: -by_bit[b])[:12]:
            count = by_bit[bit]
            print(f"    Bit {bit:2d}: {count} mismatches")

        print("\n  First 5 mismatch details:")
        for d in results['details'][:5]:
            print(f"    FAR 0x{d['far']:08X} Col {d['col']:2d} Minor {d['minor']:3d}")
            print(f"      Ref:  0x{d['ref']:08X}")
            print(f"      Ours: 0x{d['ours']:08X}")
            print(f"      Diff: 0x{d['diff']:08X}")

    print("\n" + "=" * 70)
    print("  VERDICT")
    print("=" * 70)

    accuracy = 100 * exact / total if total > 0 else 0

    if accuracy > 90:
        print(f"\n  [OK] VALUE-AT-POSITION accuracy: {accuracy:.1f}%")
        print("    PIP database has strong spatial knowledge.")
    elif accuracy > 70:
        print(f"\n  [~] VALUE-AT-POSITION accuracy: {accuracy:.1f}%")
        print("    PIP database has moderate spatial knowledge.")
        print("    Consider expanding database for problematic columns.")
    else:
        print(f"\n  [X] VALUE-AT-POSITION accuracy: {accuracy:.1f}%")
        print("    PIP database is mostly guessing.")
        print("    Need more patterns from reference.")

    # The ONETWO insight
    print("\n  ONETWO Insight:")
    if missing > mismatch:
        print(f"    Most gaps are MISSING frames ({missing}), not wrong values ({mismatch}).")
        print("    Our database knows what it knows — it just doesn't cover everything.")
    else:
        print(f"    Most gaps are WRONG values ({mismatch}), not missing frames ({missing}).")
        print("    Fallback patterns are firing but producing incorrect routes.")

    return results


if __name__ == "__main__":
    import sys

    if len(sys.argv) >= 3:
        ref_path = sys.argv[1]
        our_path = sys.argv[2]
    else:
        # Default paths
        base = Path("C:/Dev/projects/FBC-Semiconductor-System")
        ref_path = base / "reference/kzhang_v2_2016/top.bit"
        our_path = base / "build/bitstreams/fbc_full.bit"

    run_verification(ref_path, our_path)
