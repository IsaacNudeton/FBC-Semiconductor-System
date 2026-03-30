/*
 * dc.h — Device Config Generator: types and declarations
 *
 * Pipeline 2: DeviceJSON + TesterProfile → PIN_MAP + .map + .lvl + .tim + .tp + scripts
 * Feeds Pipeline 1 (pc.h) — the PIN_MAP it generates is consumed by pc_load_pinmap().
 *
 * Isaac Oravec & Claude, March 2026
 */

#ifndef DC_H
#define DC_H

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
 * EXPORT MACRO (shares PC_EXPORT flag)
 * ═══════════════════════════════════════════════════════════════ */

#ifdef _WIN32
  #ifdef PC_EXPORT
    #define DC_API __declspec(dllexport)
  #else
    #define DC_API __declspec(dllimport)
  #endif
#else
  #ifdef PC_EXPORT
    #define DC_API __attribute__((visibility("default")))
  #else
    #define DC_API
  #endif
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* ═══════════════════════════════════════════════════════════════
 * RETURN CODES
 * ═══════════════════════════════════════════════════════════════ */

#define DC_OK            0
#define DC_ERR_FILE     -1
#define DC_ERR_PARSE    -2
#define DC_ERR_ALLOC    -3
#define DC_ERR_PROFILE  -4
#define DC_ERR_HANDLE   -5
#define DC_ERR_VALIDATE -6
#define DC_ERR_WRITE    -7

/* ═══════════════════════════════════════════════════════════════
 * LIMITS
 * ═══════════════════════════════════════════════════════════════ */

#define DC_MAX_CH       256
#define DC_MAX_BANKS     16
#define DC_MAX_SUPPLIES  32
#define DC_MAX_STEPS     64
#define DC_MAX_NAME     256
#define DC_MAX_ERR      512

/* ═══════════════════════════════════════════════════════════════
 * FILE TYPE ENUM (for dc_gen_file)
 * ═══════════════════════════════════════════════════════════════ */

typedef enum {
    DC_FILE_PINMAP    = 0,
    DC_FILE_MAP       = 1,
    DC_FILE_LVL       = 2,
    DC_FILE_TIM       = 3,
    DC_FILE_TP        = 4,
    DC_FILE_POWER_ON  = 5,
    DC_FILE_POWER_OFF = 6,
    DC_FILE_PLAN_JSON = 7,  /* FBC test plan JSON (TestPlanDef format) */
    DC_FILE__COUNT    = 8
} DcFileType;

/* ═══════════════════════════════════════════════════════════════
 * TESTER PROFILE — describes the SYSTEM, not the device
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    char name[DC_MAX_NAME];     /* "B13", "B33", "B34" */
    int  start_pin;             /* 0, 48, 96 */
    int  num_pins;              /* 48, 48, 32 */
} DcGpioBank;

typedef struct {
    char   name[DC_MAX_NAME];   /* "CORE1" */
    int    dac_channel;         /* DAC index */
    int    mio_pin;             /* GPIO for enable */
    double default_voltage;
} DcCoreHw;

typedef struct {
    char       name[DC_MAX_NAME];           /* "Sonoma", "MCC", "XP-160" */
    int        total_channels;              /* 128 for Sonoma */
    DcGpioBank banks[DC_MAX_BANKS];
    int        num_banks;
    DcCoreHw   cores[DC_MAX_SUPPLIES];
    int        num_cores;
    char       firmware_path[DC_MAX_NAME];  /* "/mnt/bin/linux_*.elf" */
    char       vector_dir[DC_MAX_NAME];     /* "/mnt/bin/vectors" */
    double     default_period_ns;
    double     default_drive_on_ns;
    double     default_drive_off_ns;
    double     default_compare_ns;
} DcTesterProfile;

/* ═══════════════════════════════════════════════════════════════
 * DEVICE IR — describes ONE device on ONE tester
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    char signal_name[DC_MAX_NAME];  /* "DQ0", "CLK" */
    int  channel;                   /* GPIO number */
    int  direction;                 /* 0=IO, 1=in, 2=out */
} DcChannelMap;

typedef struct {
    char   core_name[DC_MAX_NAME];  /* "CORE1" */
    double voltage;                 /* 1.8 */
    int    sequence_order;          /* power-on order */
    double ramp_delay_ms;           /* delay after enable */
} DcSupplyAssign;

typedef struct {
    char pattern_name[DC_MAX_NAME]; /* "Calibration" */
    char pattern_file[DC_MAX_NAME]; /* "Calibration.atp" */
    int  loop_count;
    /* FBC plan fields (0 = not set / use defaults) */
    int  pattern_id;        /* SD pattern index (0-255), -1 = auto-assign by step index */
    int  duration_secs;     /* per-step duration (0 = single pass) */
    int  fail_action;       /* 0=abort, 1=continue */
    int  error_threshold;   /* max errors before fail_action (0 = any) */
    int  temp_setpoint_dc;  /* 0.1°C units (0 = no change, 0x7FFF sentinel) */
    int  clock_div;         /* 0-4 freq_sel, -1 = no change (0xFF sentinel) */
} DcTestStep;

typedef struct {
    char           device_name[DC_MAX_NAME];
    char           lot_id[DC_MAX_NAME];
    DcChannelMap   channels[DC_MAX_CH];
    int            num_channels;
    DcSupplyAssign supplies[DC_MAX_SUPPLIES];
    int            num_supplies;
    double         bank_voltages[DC_MAX_BANKS];
    int            num_bank_voltages;
    DcTestStep     steps[DC_MAX_STEPS];
    int            num_steps;
    /* Timing overrides (0 = use profile defaults) */
    double         period_ns;
    double         drive_on_ns;
    double         drive_off_ns;
    double         compare_ns;
} DcDeviceIR;

/* ═══════════════════════════════════════════════════════════════
 * HANDLE STATE (used by dc_api.c)
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    DcTesterProfile profile;
    DcDeviceIR      device;
    int             profile_loaded;
    int             device_loaded;
    char            errmsg[DC_MAX_ERR];
} DcHandle;

/* ═══════════════════════════════════════════════════════════════
 * CSV PARSER (dc_csv.c)
 * ═══════════════════════════════════════════════════════════════ */

int dc_parse_csv(const char *path, DcDeviceIR *dev, char *errmsg, int errmax);

/* ═══════════════════════════════════════════════════════════════
 * JSON PARSERS (dc_json.c)
 * ═══════════════════════════════════════════════════════════════ */

int         dc_parse_profile(const char *json, DcTesterProfile *out);
int         dc_parse_device(const char *json, DcDeviceIR *out, const DcTesterProfile *prof);
const char *dc_get_builtin_profile(const char *name);
int         dc_load_profile_from_file(const char *path, DcTesterProfile *out);
int         dc_load_device_from_file(const char *path, DcDeviceIR *out, const DcTesterProfile *prof);

/* ═══════════════════════════════════════════════════════════════
 * GENERATORS (dc_gen.c)
 * ═══════════════════════════════════════════════════════════════ */

int dc_gen_pinmap(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_map(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_lvl(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_tim(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_tp(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_plan_json(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_power_on(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_power_off(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);
int dc_gen_all(const DcTesterProfile *prof, const DcDeviceIR *dev, const char *output_dir);

/* Validation */
int dc_validate_device(const DcTesterProfile *prof, const DcDeviceIR *dev, char *errmsg, int errmax);

/* ═══════════════════════════════════════════════════════════════
 * DLL API — handle-based, FFI-safe (dc_api.c)
 * ═══════════════════════════════════════════════════════════════ */

DC_API int         dc_create(void);
DC_API void        dc_destroy(int h);
DC_API int         dc_load_profile(int h, const char *path_or_name);
DC_API int         dc_load_device(int h, const char *path);
DC_API int         dc_validate(int h);
DC_API int         dc_generate(int h, const char *output_dir);
DC_API int         dc_gen_file(int h, const char *output_dir, int file_type);
DC_API int         dc_num_channels(int h);
DC_API int         dc_num_supplies(int h);
DC_API int         dc_num_steps(int h);
DC_API const char *dc_last_error(int h);
DC_API const char *dc_profile_name(int h);

#ifdef __cplusplus
}
#endif

#endif /* DC_H */
