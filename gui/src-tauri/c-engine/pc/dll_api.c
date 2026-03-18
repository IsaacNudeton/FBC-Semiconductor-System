/*
 * dll_api.c — Handle-based DLL API for FFI (P/Invoke, koffi)
 *
 * Same pattern as onetwo_api.c: static pool, integer handles,
 * no structs cross the boundary.
 */

#ifndef PC_EXPORT
#define PC_EXPORT
#endif
#include "pc.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

#define MAX_HANDLES 16

static PcPattern g_patterns[MAX_HANDLES];
static uint8_t   g_used[MAX_HANDLES];
static int       g_initialized = 0;

#define SAFE(h) ((h) >= 0 && (h) < MAX_HANDLES && g_used[h])

/* ═══════════════════════════════════════════════════════════════
 * LIFECYCLE
 * ═══════════════════════════════════════════════════════════════ */

PC_API int pc_create(void)
{
    if (!g_initialized) {
        memset(g_used, 0, sizeof(g_used));
        g_initialized = 1;
    }
    for (int i = 0; i < MAX_HANDLES; i++) {
        if (!g_used[i]) {
            g_used[i] = 1;
            pc_pattern_init(&g_patterns[i], "");
            return i;
        }
    }
    return -1;
}

PC_API void pc_destroy(int h)
{
    if (SAFE(h)) {
        pc_pattern_free(&g_patterns[h]);
        g_used[h] = 0;
    }
}

/* ═══════════════════════════════════════════════════════════════
 * OPERATIONS
 * ═══════════════════════════════════════════════════════════════ */

PC_API int pc_dll_load_pinmap(int h, const char *path)
{
    if (!SAFE(h)) return PC_ERR_HANDLE;
    return pc_load_pinmap(&g_patterns[h], path);
}

PC_API int pc_dll_load_input(int h, const char *path, int format)
{
    if (!SAFE(h)) return PC_ERR_HANDLE;
    if (!path) return PC_ERR_FILE;

    PcPattern *p = &g_patterns[h];

    /* Auto-detect format from extension if FMT_AUTO */
    int fmt = format;
    if (fmt == FMT_AUTO) {
        const char *ext = strrchr(path, '.');
        if (ext) {
            if (strcasecmp(ext, ".atp") == 0)  fmt = FMT_ATP;
            else if (strcasecmp(ext, ".stil") == 0) fmt = FMT_STIL;
            else if (strcasecmp(ext, ".avc") == 0)  fmt = FMT_AVC;
            else fmt = FMT_ATP; /* default */
        } else {
            fmt = FMT_ATP;
        }
    }

    switch (fmt) {
    case FMT_ATP:
        return pc_parse_atp(p, path);
    case FMT_STIL:
    case FMT_AVC:
        snprintf(p->errmsg, PC_MAX_ERR, "Format %d not yet implemented", fmt);
        return PC_ERR_FORMAT;
    default:
        snprintf(p->errmsg, PC_MAX_ERR, "Unknown format: %d", fmt);
        return PC_ERR_FORMAT;
    }
}

PC_API int pc_dll_convert(int h, const char *hex_path, const char *seq_path)
{
    if (!SAFE(h)) return PC_ERR_HANDLE;

    PcPattern *p = &g_patterns[h];

    /* Apply identity map if not already mapped */
    if (!p->mapped)
        pc_apply_identity_map(p);

    int rc;

    if (hex_path && *hex_path) {
        rc = pc_gen_hex(p, hex_path, 0);
        if (rc != PC_OK) return rc;
    }

    if (seq_path && *seq_path) {
        /* Derive ATP name from pattern name */
        char atp_name[PC_MAX_NAME + 8];
        snprintf(atp_name, sizeof(atp_name), "%s.atp", p->name);
        rc = pc_gen_seq(p, seq_path, atp_name);
        if (rc != PC_OK) return rc;
    }

    return PC_OK;
}

PC_API int pc_dll_gen_fbc(int h, const char *fbc_path, uint32_t vec_clock_hz)
{
    if (!SAFE(h)) return PC_ERR_HANDLE;

    PcPattern *p = &g_patterns[h];

    /* Apply identity map if not already mapped */
    if (!p->mapped)
        pc_apply_identity_map(p);

    if (!fbc_path || !*fbc_path) return PC_ERR_FILE;

    return pc_gen_fbc(p, fbc_path, vec_clock_hz);
}

/* ═══════════════════════════════════════════════════════════════
 * QUERIES
 * ═══════════════════════════════════════════════════════════════ */

PC_API int pc_dll_num_signals(int h)
{
    return SAFE(h) ? g_patterns[h].num_signals : 0;
}

PC_API int pc_dll_num_vectors(int h)
{
    return SAFE(h) ? g_patterns[h].num_vectors : 0;
}

PC_API const char *pc_dll_last_error(int h)
{
    if (!SAFE(h)) return "Invalid handle";
    return g_patterns[h].errmsg;
}

PC_API const char *pc_dll_version(void)
{
    return "1.0.0";
}
