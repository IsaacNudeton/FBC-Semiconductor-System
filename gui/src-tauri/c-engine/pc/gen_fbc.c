/*
 * gen_fbc.c — FBC compressed binary generator
 *
 * Converts PcPattern (IR) to compressed .fbc format.
 * Compatible with Rust FBC format (host/src/vector/format.rs).
 *
 * File layout:
 *   [0:32]   FbcHeader  (32 bytes, LE)
 *   [32:112] PinConfig  (80 bytes, 160 pins × 4 bits)
 *   [112:..]  Compressed vector data (opcodes + payloads)
 *
 * Compression strategy (ONETWO-derived, matching Rust compiler.rs):
 *   vector == 0          → VECTOR_ZERO   (1 byte)
 *   vector == all-ones   → VECTOR_ONES   (1 byte)
 *   vector == previous   → VECTOR_RUN    (accumulate, then 1+4 bytes)
 *   hamming(v, prev) ≤15 → VECTOR_SPARSE (1+1+N bytes, N = changed pins)
 *   otherwise            → VECTOR_FULL   (1+20 bytes)
 *
 * Isaac Oravec & Claude, March 2026
 */

#include "pc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ═══════════════════════════════════════════════════════════════
 * FBC FORMAT CONSTANTS (matching host/src/vector/format.rs)
 * ═══════════════════════════════════════════════════════════════ */

#define FBC_MAGIC           0x00434246  /* "FBC\0" LE */
#define FBC_VERSION         1
#define FBC_PIN_COUNT       160
#define FBC_VECTOR_BYTES    20          /* 160 bits = 20 bytes */
#define FBC_HEADER_SIZE     32
#define FBC_PINCONFIG_SIZE  80
#define FBC_SPARSE_CROSSOVER 15
#define FBC_THERMAL_SEGMENT  1024  /* vectors per thermal segment */

/* Header flags */
#define FBC_FLAG_THERMAL_PROFILE 0x01  /* bit 0: thermal profile appended after OP_END */

/* Opcodes */
#define OP_NOP              0x00
#define OP_VECTOR_FULL      0x01
#define OP_VECTOR_SPARSE    0x02
#define OP_VECTOR_RUN       0x03
#define OP_VECTOR_ZERO      0x04
#define OP_VECTOR_ONES      0x05
#define OP_VECTOR_XOR       0x06
#define OP_END              0x07

/* Thermal power levels (matching firmware thermal.rs PowerLevel) */
#define THERMAL_POWER_LOW    0
#define THERMAL_POWER_MEDIUM 1
#define THERMAL_POWER_HIGH   2

/* Thermal segment: 8 bytes per segment, appended after OP_END */
typedef struct {
    uint32_t vector_offset;     /* starting vector index */
    uint8_t  avg_toggle_rate;   /* average toggles per vector (0-160) */
    uint8_t  avg_active_pins;   /* average active pins (0-160) */
    uint8_t  power_level;       /* 0=Low, 1=Medium, 2=High */
    uint8_t  reserved;
} FbcThermalSegment;

/* Thermal accumulator (used during compression) */
typedef struct {
    uint64_t total_toggles;     /* sum of toggles in current segment */
    uint64_t total_active;      /* sum of active pins in current segment */
    uint32_t vectors_in_segment; /* vectors counted in current segment */
    uint32_t segment_start;     /* vector offset where segment started */
    FbcThermalSegment *segments; /* dynamic array of completed segments */
    int num_segments;
    int cap_segments;
} ThermalAccum;

static int thermal_init(ThermalAccum *ta)
{
    ta->total_toggles = 0;
    ta->total_active = 0;
    ta->vectors_in_segment = 0;
    ta->segment_start = 0;
    ta->num_segments = 0;
    ta->cap_segments = 16;
    ta->segments = (FbcThermalSegment *)malloc(ta->cap_segments * sizeof(FbcThermalSegment));
    return ta->segments ? 0 : -1;
}

static int thermal_flush_segment(ThermalAccum *ta)
{
    if (ta->vectors_in_segment == 0) return 0;

    /* Grow if needed */
    if (ta->num_segments >= ta->cap_segments) {
        int new_cap = ta->cap_segments * 2;
        FbcThermalSegment *new_buf = (FbcThermalSegment *)realloc(
            ta->segments, new_cap * sizeof(FbcThermalSegment));
        if (!new_buf) return -1;
        ta->segments = new_buf;
        ta->cap_segments = new_cap;
    }

    uint32_t avg_toggles = (uint32_t)(ta->total_toggles / ta->vectors_in_segment);
    uint32_t avg_active  = (uint32_t)(ta->total_active / ta->vectors_in_segment);

    /* Classify power level (same thresholds as thermal.rs estimate_power) */
    uint8_t level;
    if (avg_toggles > 40 && avg_active > 60)
        level = THERMAL_POWER_HIGH;
    else if (avg_toggles > 20 || avg_active > 40)
        level = THERMAL_POWER_MEDIUM;
    else
        level = THERMAL_POWER_LOW;

    FbcThermalSegment seg;
    seg.vector_offset   = ta->segment_start;
    seg.avg_toggle_rate = (uint8_t)(avg_toggles > 160 ? 160 : avg_toggles);
    seg.avg_active_pins = (uint8_t)(avg_active > 160 ? 160 : avg_active);
    seg.power_level     = level;
    seg.reserved        = 0;

    ta->segments[ta->num_segments++] = seg;

    /* Reset for next segment */
    ta->total_toggles = 0;
    ta->total_active = 0;
    ta->segment_start += ta->vectors_in_segment;
    ta->vectors_in_segment = 0;

    return 0;
}

/* Accumulate stats for one vector (call with toggles from prev and active pin count) */
static int thermal_add(ThermalAccum *ta, int toggles, int active_pins, uint32_t repeat)
{
    for (uint32_t r = 0; r < repeat; r++) {
        ta->total_toggles += (uint64_t)toggles;
        ta->total_active  += (uint64_t)active_pins;
        ta->vectors_in_segment++;

        if (ta->vectors_in_segment >= FBC_THERMAL_SEGMENT) {
            if (thermal_flush_segment(ta) < 0) return -1;
        }
    }
    return 0;
}

static void thermal_free(ThermalAccum *ta)
{
    free(ta->segments);
    ta->segments = NULL;
}

/* Pin types (matching format.rs PinType enum) */
#define FBC_PIN_BIDI        0
#define FBC_PIN_INPUT       1
#define FBC_PIN_OUTPUT      2

/* ═══════════════════════════════════════════════════════════════
 * FBC VECTOR — 160 bits packed into 20 bytes
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    uint8_t data[FBC_VECTOR_BYTES];
} FbcVector;

static const FbcVector FBC_VEC_ZERO = {{0}};
static const FbcVector FBC_VEC_ONES = {{
    0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
    0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
    0xFF,0xFF,0xFF,0xFF
}};

static int fbc_vec_eq(const FbcVector *a, const FbcVector *b)
{
    return memcmp(a->data, b->data, FBC_VECTOR_BYTES) == 0;
}

static int fbc_vec_is_zero(const FbcVector *v)
{
    return fbc_vec_eq(v, &FBC_VEC_ZERO);
}

static int fbc_vec_is_ones(const FbcVector *v)
{
    return fbc_vec_eq(v, &FBC_VEC_ONES);
}

/* Set bit at position (0-159) */
static void fbc_vec_set_bit(FbcVector *v, int pos, int val)
{
    int byte_idx = pos / 8;
    int bit_idx  = pos % 8;
    if (val)
        v->data[byte_idx] |= (uint8_t)(1 << bit_idx);
    else
        v->data[byte_idx] &= (uint8_t)~(1 << bit_idx);
}

/* Get bit at position (0-159) */
static int fbc_vec_get_bit(const FbcVector *v, int pos)
{
    int byte_idx = pos / 8;
    int bit_idx  = pos % 8;
    return (v->data[byte_idx] >> bit_idx) & 1;
}

/* XOR two vectors */
static FbcVector fbc_vec_xor(const FbcVector *a, const FbcVector *b)
{
    FbcVector r;
    for (int i = 0; i < FBC_VECTOR_BYTES; i++)
        r.data[i] = a->data[i] ^ b->data[i];
    return r;
}

/* Popcount of vector (total 1 bits) */
static int fbc_vec_popcount(const FbcVector *v)
{
    int count = 0;
    for (int i = 0; i < FBC_VECTOR_BYTES; i++) {
        uint8_t b = v->data[i];
        /* Brian Kernighan's trick */
        while (b) { count++; b &= b - 1; }
    }
    return count;
}

/* ═══════════════════════════════════════════════════════════════
 * DYNAMIC BYTE BUFFER
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    uint8_t *buf;
    size_t   len;
    size_t   cap;
} ByteBuf;

static int buf_init(ByteBuf *b, size_t initial_cap)
{
    b->buf = (uint8_t *)malloc(initial_cap);
    if (!b->buf) return -1;
    b->len = 0;
    b->cap = initial_cap;
    return 0;
}

static int buf_push(ByteBuf *b, uint8_t byte)
{
    if (b->len >= b->cap) {
        size_t new_cap = b->cap * 2;
        uint8_t *new_buf = (uint8_t *)realloc(b->buf, new_cap);
        if (!new_buf) return -1;
        b->buf = new_buf;
        b->cap = new_cap;
    }
    b->buf[b->len++] = byte;
    return 0;
}

static int buf_push_bytes(ByteBuf *b, const uint8_t *data, size_t n)
{
    while (b->len + n > b->cap) {
        size_t new_cap = b->cap * 2;
        uint8_t *new_buf = (uint8_t *)realloc(b->buf, new_cap);
        if (!new_buf) return -1;
        b->buf = new_buf;
        b->cap = new_cap;
    }
    memcpy(b->buf + b->len, data, n);
    b->len += n;
    return 0;
}

static int buf_push_u32_le(ByteBuf *b, uint32_t val)
{
    uint8_t bytes[4] = {
        (uint8_t)(val & 0xFF),
        (uint8_t)((val >> 8) & 0xFF),
        (uint8_t)((val >> 16) & 0xFF),
        (uint8_t)((val >> 24) & 0xFF)
    };
    return buf_push_bytes(b, bytes, 4);
}

static void buf_free(ByteBuf *b)
{
    free(b->buf);
    b->buf = NULL;
    b->len = 0;
    b->cap = 0;
}

/* ═══════════════════════════════════════════════════════════════
 * CONVERT PcVector → FbcVector
 *
 * PcVector.states[128] → 160-bit VALUE vector
 * Channels 0-127: STATE_VALUE[state]
 * Channels 128-159: always 0 (unused in legacy patterns)
 * ═══════════════════════════════════════════════════════════════ */

static void pc_to_fbc_vector(const PcPattern *p, int vec_idx, FbcVector *out)
{
    const PcVector *src = &p->vectors[vec_idx];
    memset(out->data, 0, FBC_VECTOR_BYTES);

    for (int si = 0; si < p->num_signals && si < PC_MAX_CH; si++) {
        int ch;
        if (p->mapped) {
            ch = p->signals[si].channel;
        } else {
            ch = si; /* identity: signal index = channel */
        }
        if (ch < 0 || ch >= FBC_PIN_COUNT) continue;

        uint8_t state = src->states[si];
        if (state >= PS__COUNT) state = PS_DONT_CARE;
        int value = STATE_VALUE[state];
        fbc_vec_set_bit(out, ch, value);
    }
}

/* ═══════════════════════════════════════════════════════════════
 * BUILD PIN CONFIG
 *
 * Map PcSignal.pin_type to FBC PinType, pack into 80 bytes.
 * Byte[i] = type[2*i] | (type[2*i+1] << 4)
 * ═══════════════════════════════════════════════════════════════ */

static void build_pin_config(const PcPattern *p, uint8_t config[FBC_PINCONFIG_SIZE])
{
    /* Default: all pins Bidi (0) */
    uint8_t types[FBC_PIN_COUNT];
    memset(types, FBC_PIN_BIDI, FBC_PIN_COUNT);

    for (int si = 0; si < p->num_signals && si < PC_MAX_CH; si++) {
        int ch;
        if (p->mapped) {
            ch = p->signals[si].channel;
        } else {
            ch = si;
        }
        if (ch < 0 || ch >= FBC_PIN_COUNT) continue;

        /* Map pc pin_type (0=IO, 1=input-only, 2=output-only) to FBC */
        switch (p->signals[si].pin_type) {
        case 1:  types[ch] = FBC_PIN_INPUT;  break;
        case 2:  types[ch] = FBC_PIN_OUTPUT; break;
        default: types[ch] = FBC_PIN_BIDI;   break;
        }
    }

    /* Pack: 2 pins per byte, lo nibble = even pin, hi nibble = odd pin */
    for (int i = 0; i < FBC_PINCONFIG_SIZE; i++) {
        config[i] = (types[i * 2] & 0x0F) | ((types[i * 2 + 1] & 0x0F) << 4);
    }
}

/* ═══════════════════════════════════════════════════════════════
 * COMPRESSION ENGINE (matching Rust compiler.rs exactly)
 * ═══════════════════════════════════════════════════════════════ */

/*
 * Emit a single vector using optimal encoding.
 * Returns 0 on success, -1 on alloc failure.
 */
static int emit_vector(ByteBuf *data, const FbcVector *vec, FbcVector *prev)
{
    /* Check all-zeros */
    if (fbc_vec_is_zero(vec)) {
        if (buf_push(data, OP_VECTOR_ZERO) < 0) return -1;
        *prev = *vec;
        return 0;
    }

    /* Check all-ones */
    if (fbc_vec_is_ones(vec)) {
        if (buf_push(data, OP_VECTOR_ONES) < 0) return -1;
        *prev = *vec;
        return 0;
    }

    /* Calculate hamming distance from previous */
    FbcVector diff = fbc_vec_xor(vec, prev);
    int toggles = fbc_vec_popcount(&diff);

    if (toggles == 0) {
        /* Same as previous — emit RUN(1) */
        if (buf_push(data, OP_VECTOR_RUN) < 0) return -1;
        if (buf_push_u32_le(data, 1) < 0) return -1;
        return 0;
    }

    if (toggles <= FBC_SPARSE_CROSSOVER) {
        /* Sparse encoding: opcode + count + indices */
        if (buf_push(data, OP_VECTOR_SPARSE) < 0) return -1;
        if (buf_push(data, (uint8_t)toggles) < 0) return -1;

        /* For each changed bit: encode (pin_index << 1) | new_value */
        for (int pin = 0; pin < FBC_PIN_COUNT; pin++) {
            if (fbc_vec_get_bit(&diff, pin)) {
                int new_val = fbc_vec_get_bit(vec, pin);
                if (buf_push(data, (uint8_t)((pin << 1) | new_val)) < 0)
                    return -1;
            }
        }
    } else {
        /* Full encoding: opcode + 20 raw bytes */
        if (buf_push(data, OP_VECTOR_FULL) < 0) return -1;
        if (buf_push_bytes(data, vec->data, FBC_VECTOR_BYTES) < 0) return -1;
    }

    *prev = *vec;
    return 0;
}

/*
 * Emit a run of identical vectors.
 * First emits the vector itself (if different from prev), then RUN opcode.
 */
static int emit_run(ByteBuf *data, const FbcVector *vec, uint32_t count, FbcVector *prev)
{
    /* First emit the vector itself if different from previous */
    if (!fbc_vec_eq(vec, prev)) {
        if (emit_vector(data, vec, prev) < 0)
            return -1;
    }

    /* Then emit RUN if count > 1 (count-1 = additional repeats) */
    if (count > 1) {
        if (buf_push(data, OP_VECTOR_RUN) < 0) return -1;
        if (buf_push_u32_le(data, count - 1) < 0) return -1;
    }

    return 0;
}

/* ═══════════════════════════════════════════════════════════════
 * WRITE HEADER
 * ═══════════════════════════════════════════════════════════════ */

static void write_header(uint8_t hdr[FBC_HEADER_SIZE],
                         uint32_t num_vectors,
                         uint32_t compressed_size,
                         uint32_t vec_clock_hz,
                         uint32_t crc)
{
    memset(hdr, 0, FBC_HEADER_SIZE);

    /* magic (u32 LE) */
    hdr[0] = (uint8_t)(FBC_MAGIC & 0xFF);
    hdr[1] = (uint8_t)((FBC_MAGIC >> 8) & 0xFF);
    hdr[2] = (uint8_t)((FBC_MAGIC >> 16) & 0xFF);
    hdr[3] = (uint8_t)((FBC_MAGIC >> 24) & 0xFF);

    /* version (u16 LE) */
    hdr[4] = (uint8_t)(FBC_VERSION & 0xFF);
    hdr[5] = (uint8_t)((FBC_VERSION >> 8) & 0xFF);

    /* pin_count (u8) */
    hdr[6] = (uint8_t)FBC_PIN_COUNT;

    /* flags (u8) — reserved */
    hdr[7] = 0;

    /* num_vectors (u32 LE) */
    hdr[8]  = (uint8_t)(num_vectors & 0xFF);
    hdr[9]  = (uint8_t)((num_vectors >> 8) & 0xFF);
    hdr[10] = (uint8_t)((num_vectors >> 16) & 0xFF);
    hdr[11] = (uint8_t)((num_vectors >> 24) & 0xFF);

    /* compressed_size (u32 LE) */
    hdr[12] = (uint8_t)(compressed_size & 0xFF);
    hdr[13] = (uint8_t)((compressed_size >> 8) & 0xFF);
    hdr[14] = (uint8_t)((compressed_size >> 16) & 0xFF);
    hdr[15] = (uint8_t)((compressed_size >> 24) & 0xFF);

    /* vec_clock_hz (u32 LE) */
    hdr[16] = (uint8_t)(vec_clock_hz & 0xFF);
    hdr[17] = (uint8_t)((vec_clock_hz >> 8) & 0xFF);
    hdr[18] = (uint8_t)((vec_clock_hz >> 16) & 0xFF);
    hdr[19] = (uint8_t)((vec_clock_hz >> 24) & 0xFF);

    /* crc32 (u32 LE) */
    hdr[20] = (uint8_t)(crc & 0xFF);
    hdr[21] = (uint8_t)((crc >> 8) & 0xFF);
    hdr[22] = (uint8_t)((crc >> 16) & 0xFF);
    hdr[23] = (uint8_t)((crc >> 24) & 0xFF);

    /* _reserved[8] = zeros (already zeroed by memset) */
}

/* ═══════════════════════════════════════════════════════════════
 * PUBLIC API: pc_gen_fbc
 *
 * Writes compressed .fbc file from PcPattern.
 * Returns PC_OK on success, error code on failure.
 *
 * vec_clock_hz: vector clock frequency (pass 0 for default 100MHz)
 * ═══════════════════════════════════════════════════════════════ */

int pc_gen_fbc(const PcPattern *p, const char *path, uint32_t vec_clock_hz)
{
    if (!p || !path) return PC_ERR_FILE;
    if (p->num_vectors == 0) return PC_ERR_FORMAT;

    if (vec_clock_hz == 0)
        vec_clock_hz = 100000000; /* 100 MHz default */

    /* ── Phase 1: Build pin config ── */
    uint8_t pin_config[FBC_PINCONFIG_SIZE];
    build_pin_config(p, pin_config);

    /* ── Phase 2: Compress vectors + accumulate thermal profile ── */
    ByteBuf data;
    if (buf_init(&data, 4096) < 0)
        return PC_ERR_ALLOC;

    ThermalAccum thermal;
    if (thermal_init(&thermal) < 0) {
        buf_free(&data);
        return PC_ERR_ALLOC;
    }

    FbcVector prev = FBC_VEC_ZERO;
    FbcVector thermal_prev = FBC_VEC_ZERO; /* separate prev for thermal (tracks uncompressed) */
    uint64_t total_vectors = 0;

    /* Pending run state */
    int      has_pending = 0;
    FbcVector pending_vec;
    uint32_t pending_count = 0;

    for (int vi = 0; vi < p->num_vectors; vi++) {
        FbcVector vec;
        pc_to_fbc_vector(p, vi, &vec);
        uint64_t repeat = p->vectors[vi].repeat;
        if (repeat == 0) repeat = 1;

        /* Thermal analysis: XOR + popcount (already computed by compression, but we
           need it per-uncompressed-vector for accurate thermal segmentation) */
        FbcVector tdiff = fbc_vec_xor(&vec, &thermal_prev);
        int toggles = fbc_vec_popcount(&tdiff);
        int active  = fbc_vec_popcount(&vec);
        if (thermal_add(&thermal, toggles, active, (uint32_t)repeat) < 0) {
            buf_free(&data);
            thermal_free(&thermal);
            return PC_ERR_ALLOC;
        }
        thermal_prev = vec;

        if (has_pending) {
            if (fbc_vec_eq(&vec, &pending_vec)) {
                /* Extend the pending run */
                pending_count += (uint32_t)repeat;
                total_vectors += repeat;
                continue;
            } else {
                /* Flush the pending run */
                if (emit_run(&data, &pending_vec, pending_count, &prev) < 0) {
                    buf_free(&data);
                    thermal_free(&thermal);
                    return PC_ERR_ALLOC;
                }
                has_pending = 0;
            }
        }

        /* Check if this vector starts a new run (repeat >= 2) */
        if (repeat >= 2) {
            has_pending = 1;
            pending_vec = vec;
            pending_count = (uint32_t)repeat;
            total_vectors += repeat;
        } else {
            /* Emit individual vector */
            if (emit_vector(&data, &vec, &prev) < 0) {
                buf_free(&data);
                thermal_free(&thermal);
                return PC_ERR_ALLOC;
            }
            total_vectors += 1;
        }
    }

    /* Flush any remaining pending run */
    if (has_pending) {
        if (emit_run(&data, &pending_vec, pending_count, &prev) < 0) {
            buf_free(&data);
            thermal_free(&thermal);
            return PC_ERR_ALLOC;
        }
    }

    /* Emit END opcode */
    if (buf_push(&data, OP_END) < 0) {
        buf_free(&data);
        thermal_free(&thermal);
        return PC_ERR_ALLOC;
    }

    /* Flush final thermal segment */
    if (thermal_flush_segment(&thermal) < 0) {
        buf_free(&data);
        thermal_free(&thermal);
        return PC_ERR_ALLOC;
    }

    /* Append thermal profile after OP_END */
    for (int si = 0; si < thermal.num_segments; si++) {
        FbcThermalSegment *seg = &thermal.segments[si];
        if (buf_push_u32_le(&data, seg->vector_offset) < 0 ||
            buf_push(&data, seg->avg_toggle_rate) < 0 ||
            buf_push(&data, seg->avg_active_pins) < 0 ||
            buf_push(&data, seg->power_level) < 0 ||
            buf_push(&data, seg->reserved) < 0) {
            buf_free(&data);
            thermal_free(&thermal);
            return PC_ERR_ALLOC;
        }
    }

    int num_thermal_segments = thermal.num_segments;
    thermal_free(&thermal);

    /* ── Phase 3: Calculate CRC32 ── */
    /* CRC covers: header (with crc32 field zeroed) + pin_config + data */
    uint8_t hdr_bytes[FBC_HEADER_SIZE];
    uint32_t num_vec_clamped = (total_vectors > 0xFFFFFFFF)
                               ? 0xFFFFFFFF
                               : (uint32_t)total_vectors;
    write_header(hdr_bytes, num_vec_clamped, (uint32_t)data.len, vec_clock_hz, 0);

    /* Set flags: thermal profile present */
    if (num_thermal_segments > 0) {
        hdr_bytes[7] = FBC_FLAG_THERMAL_PROFILE;
        /* Store segment count in _reserved[0:4] (offset 24-27, LE) */
        hdr_bytes[24] = (uint8_t)(num_thermal_segments & 0xFF);
        hdr_bytes[25] = (uint8_t)((num_thermal_segments >> 8) & 0xFF);
        hdr_bytes[26] = (uint8_t)((num_thermal_segments >> 16) & 0xFF);
        hdr_bytes[27] = (uint8_t)((num_thermal_segments >> 24) & 0xFF);
    }

    /* CRC = hash(header_with_crc_zero + pin_config + data) */
    size_t crc_total = FBC_HEADER_SIZE + FBC_PINCONFIG_SIZE + data.len;
    uint8_t *crc_buf = (uint8_t *)malloc(crc_total);
    if (!crc_buf) {
        buf_free(&data);
        return PC_ERR_ALLOC;
    }
    memcpy(crc_buf, hdr_bytes, FBC_HEADER_SIZE);
    memcpy(crc_buf + FBC_HEADER_SIZE, pin_config, FBC_PINCONFIG_SIZE);
    memcpy(crc_buf + FBC_HEADER_SIZE + FBC_PINCONFIG_SIZE, data.buf, data.len);

    uint32_t crc = pc_crc32(crc_buf, crc_total);
    free(crc_buf);

    /* Rewrite header with real CRC */
    write_header(hdr_bytes, num_vec_clamped, (uint32_t)data.len, vec_clock_hz, crc);

    /* ── Phase 4: Write file ── */
    FILE *f = fopen(path, "wb");
    if (!f) {
        buf_free(&data);
        return PC_ERR_FILE;
    }

    if (fwrite(hdr_bytes, 1, FBC_HEADER_SIZE, f) != FBC_HEADER_SIZE ||
        fwrite(pin_config, 1, FBC_PINCONFIG_SIZE, f) != FBC_PINCONFIG_SIZE ||
        fwrite(data.buf, 1, data.len, f) != data.len) {
        fclose(f);
        buf_free(&data);
        return PC_ERR_WRITE;
    }

    fclose(f);
    buf_free(&data);
    return PC_OK;
}
