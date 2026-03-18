/*
 * pc.h — Pattern Converter: single header for all types and declarations
 *
 * Zero-dependency C library for converting ATP/STIL/AVC patterns
 * to Sonoma .hex/.seq binary format.
 *
 * SMART PARSERS INCLUDED:
 *   - parse_stil_smart.c: Infers pin types, groups, timing from STIL semantics
 *   - parse_avc_smart.c: Infers test conditions, pin behavior from AVC patterns
 *
 * Compile:
 *   CLI: gcc -O2 -std=c11 -o pattern_converter src/ir.c src/crc32.c ...
 *   DLL: gcc -O2 -std=c11 -shared -DPC_EXPORT -o pattern_converter.dll ...
 *
 * Isaac Oravec & Claude, March 2026
 */

#ifndef PC_H
#define PC_H

#include <stdint.h>
#include <stddef.h>

/* Windows compat: strcasecmp → _stricmp */
#ifdef _WIN32
  #include <string.h>
  #ifndef strcasecmp
    #define strcasecmp _stricmp
  #endif
#endif

/* ═══════════════════════════════════════════════════════════════
 * EXPORT MACRO
 * ═══════════════════════════════════════════════════════════════ */

#ifdef _WIN32
  #ifdef PC_EXPORT
    #define PC_API __declspec(dllexport)
  #else
    #define PC_API __declspec(dllimport)
  #endif
#else
  #ifdef PC_EXPORT
    #define PC_API __attribute__((visibility("default")))
  #else
    #define PC_API
  #endif
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* ═══════════════════════════════════════════════════════════════
 * RETURN CODES
 * ═══════════════════════════════════════════════════════════════ */

#define PC_OK           0
#define PC_ERR_FILE    -1
#define PC_ERR_PARSE   -2
#define PC_ERR_ALLOC   -3
#define PC_ERR_PINMAP  -4
#define PC_ERR_HANDLE  -5
#define PC_ERR_FORMAT  -6
#define PC_ERR_WRITE   -7

/* ═══════════════════════════════════════════════════════════════
 * PIN STATES — the ground truth
 * ═══════════════════════════════════════════════════════════════ */

typedef enum {
    PS_DRIVE_0   = 0,   /* '0' — drive low              */
    PS_DRIVE_1   = 1,   /* '1' — drive high             */
    PS_EXPECT_L  = 2,   /* 'L' — expect low (compare)   */
    PS_EXPECT_H  = 3,   /* 'H' — expect high (compare)  */
    PS_DONT_CARE = 4,   /* 'X' — don't care             */
    PS_HIGH_Z    = 5,   /* 'Z' — high impedance         */
    PS_PULSE     = 6,   /* 'P' — pulse high             */
    PS_NEG_PULSE = 7,   /* 'N' — pulse low / negative   */
    PS_RISING    = 8,   /* 'U' — rising edge            */
    PS_FALLING   = 9,   /* 'D' — falling edge           */
    PS_TERMINATE = 10,  /* 'T' — termination            */
    PS_CLOCK     = 11,  /* 'C' — clock                  */
    PS__COUNT    = 12
} PinState;

/*
 * State-to-bits lookup tables (indexed by PinState enum).
 *
 * VALUE: the data bit driven/expected on the pin.
 * OEN:   output enable. OEN=0 → DRIVE (output), OEN=1 → SENSE (input).
 *
 * Confirmed from io_cell.v RTL + real Calibration.hex/drive_even.hex.
 */
static const uint8_t STATE_VALUE[PS__COUNT] = {
/*  D0  D1  EL  EH  DC  HZ   P   N   U   D   T   C  */
    0,  1,  0,  1,  0,  0,  1,  0,  1,  0,  0,  1
};
static const uint8_t STATE_OEN[PS__COUNT] = {
/*  D0  D1  EL  EH  DC  HZ   P   N   U   D   T   C  */
    0,  0,  1,  1,  1,  1,  0,  0,  0,  0,  1,  0
};

/* ═══════════════════════════════════════════════════════════════
 * PIN TYPES — smart parser inference categories
 * ═══════════════════════════════════════════════════════════════ */

typedef enum {
    PC_PIN_IO         = 0,   /* Bidirectional IO */
    PC_PIN_PULSE_POS  = 1,   /* Positive pulse (clock) */
    PC_PIN_PULSE_NEG  = 2,   /* Negative pulse */
    PC_PIN_MONITOR    = 3,   /* Output monitor / sense */
    PC_PIN_SUPPLY     = 4,   /* Power supply pin */
    PC_PIN__COUNT     = 5
} PcPinType;

/* ═══════════════════════════════════════════════════════════════
 * INPUT FORMAT
 * ═══════════════════════════════════════════════════════════════ */

typedef enum {
    FMT_AUTO = 0,
    FMT_ATP  = 1,
    FMT_STIL = 2,
    FMT_AVC  = 3,
    FMT__COUNT
} InputFormat;

/* ═══════════════════════════════════════════════════════════════
 * CORE STRUCTS
 * ═══════════════════════════════════════════════════════════════ */

#define PC_MAX_CH   128
#define PC_MAX_SIG  256
#define PC_MAX_NAME 256
#define PC_MAX_ERR  512

typedef struct {
    uint8_t  states[PC_MAX_CH];  /* PinState per channel (0-127) */
    uint64_t repeat;             /* repeat count (0 = single execution) */
} PcVector;

typedef struct {
    char name[PC_MAX_NAME];
    int  channel;   /* GPIO number (0-127), -1 = unmapped */
    int  pin_type;  /* 0=IO, 1=input-only, 2=output-only */
    char group[64]; /* Signal group (e.g. "JTAG", "CLOCK") — set by smart parsers */
} PcSignal;

typedef struct {
    char      name[PC_MAX_NAME];
    PcSignal  signals[PC_MAX_SIG];
    int       num_signals;
    PcVector *vectors;           /* heap array (realloc growth) */
    int       num_vectors;
    int       cap_vectors;
    int       mapped;            /* nonzero if pin map applied */
    char      errmsg[PC_MAX_ERR];
} PcPattern;

/* ═══════════════════════════════════════════════════════════════
 * HEX BINARY FORMAT — 40 bytes per vector
 *
 *   [0:16]   VALUE  (128 bits, LE, bit N = channel N)
 *   [16:32]  OUTEN  (128 bits, LE, bit N = channel N)
 *   [32:40]  REPEAT (uint64_t, LE)
 *
 * OEN=0 → DRIVE, OEN=1 → SENSE
 * ═══════════════════════════════════════════════════════════════ */

#define PC_HEX_VECTOR_SIZE 40

typedef struct {
    uint64_t value_lo;   /* channels 0-63  */
    uint64_t value_hi;   /* channels 64-127 */
    uint64_t oen_lo;     /* channels 0-63  */
    uint64_t oen_hi;     /* channels 64-127 */
    uint64_t repeat;
} PcHexVector;

/* ═══════════════════════════════════════════════════════════════
 * CRC32
 * ═══════════════════════════════════════════════════════════════ */

uint32_t pc_crc32(const void *data, size_t len);

/* ═══════════════════════════════════════════════════════════════
 * IR LIFECYCLE (ir.c)
 * ═══════════════════════════════════════════════════════════════ */

void pc_pattern_init(PcPattern *p, const char *name);
void pc_pattern_free(PcPattern *p);
int  pc_pattern_add_signal(PcPattern *p, const char *name);
int  pc_pattern_add_vector(PcPattern *p, const PcVector *v);

/* Character ↔ PinState conversion */
PinState pc_char_to_state(char c);
char     pc_state_to_char(PinState s);

/* Encode a single vector into 40-byte hex format */
void pc_encode_vector(const PcVector *v, PcHexVector *out);

/* ═══════════════════════════════════════════════════════════════
 * ATP PARSER (parse_atp.c)
 * ═══════════════════════════════════════════════════════════════ */

int pc_parse_atp(PcPattern *p, const char *path);

/* ═══════════════════════════════════════════════════════════════
 * SMART STIL PARSER (parse_stil_smart.c)
 *
 * Unlike dumb parsers, this INFERS:
 *   - Pin types from signal names (JTAG_TCK → P_PULSE, TDO → MONITOR)
 *   - Groups from SignalGroups (JTAG_GROUP → all JTAG signals linked)
 *   - Timing from waveform patterns (01 01 → 50ns/150ns delays)
 *   - Test conditions from naming (tset_gen_tp1 → temp=100°C)
 * ═══════════════════════════════════════════════════════════════ */

int pc_parse_stil_smart(PcPattern *p, const char *path);

/* ═══════════════════════════════════════════════════════════════
 * SMART AVC PARSER (parse_avc_smart.c)
 *
 * Unlike dumb parsers, this INFERS:
 *   - Test type from timing set names (burn → burn-in, func → functional)
 *   - Temperature from timing set names (tp1 → 100°C, tp2 → 125°C)
 *   - Pin behavior from vector patterns (clock, drive, sense)
 *   - Repeat meaning (R60000 → burn-in stress)
 * ═══════════════════════════════════════════════════════════════ */

int pc_parse_avc_smart(PcPattern *p, const char *path);

/* ═══════════════════════════════════════════════════════════════
 * PIN MAP PARSER (parse_pinmap.c)
 * ═══════════════════════════════════════════════════════════════ */

int pc_load_pinmap(PcPattern *p, const char *path);

/* Apply identity map: signal index N → channel N (default for GPIO-named pins) */
void pc_apply_identity_map(PcPattern *p);

/* ═══════════════════════════════════════════════════════════════
 * GENERATORS
 * ═══════════════════════════════════════════════════════════════ */

/* gen_hex.c — write .hex binary */
int pc_gen_hex(const PcPattern *p, const char *path, int append_crc);

/* gen_seq.c — write .seq text */
int pc_gen_seq(const PcPattern *p, const char *path, const char *atp_name);

/* gen_fbc.c — write compressed .fbc binary (FBC format) */
int pc_gen_fbc(const PcPattern *p, const char *path, uint32_t vec_clock_hz);

/* ═══════════════════════════════════════════════════════════════
 * DLL API — handle-based, FFI-safe (dll_api.c)
 * ═══════════════════════════════════════════════════════════════ */

PC_API int         pc_create(void);
PC_API void        pc_destroy(int h);
PC_API int         pc_dll_load_pinmap(int h, const char *path);
PC_API int         pc_dll_load_input(int h, const char *path, int format);
PC_API int         pc_dll_convert(int h, const char *hex_path, const char *seq_path);
PC_API int         pc_dll_gen_fbc(int h, const char *fbc_path, uint32_t vec_clock_hz);
PC_API int         pc_dll_num_signals(int h);
PC_API int         pc_dll_num_vectors(int h);
PC_API const char *pc_dll_last_error(int h);
PC_API const char *pc_dll_version(void);

#ifdef __cplusplus
}
#endif

#endif /* PC_H */
