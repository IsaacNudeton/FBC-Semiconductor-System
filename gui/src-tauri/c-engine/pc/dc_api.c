/*
 * dc_api.c — Handle-based DLL API for device config generation
 *
 * Same pattern as dll_api.c: static pool, integer handles,
 * no structs cross the boundary. Separate pool from pc_ handles.
 */

#ifndef PC_EXPORT
#define PC_EXPORT
#endif
#include "dc.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#define DC_MAX_HANDLES 16

static DcHandle  g_dc[DC_MAX_HANDLES];
static uint8_t   g_dc_used[DC_MAX_HANDLES];
static int       g_dc_init = 0;

#define DC_SAFE(h) ((h) >= 0 && (h) < DC_MAX_HANDLES && g_dc_used[h])

/* ═══════════════════════════════════════════════════════════════
 * LIFECYCLE
 * ═══════════════════════════════════════════════════════════════ */

DC_API int dc_create(void)
{
    if (!g_dc_init) {
        memset(g_dc_used, 0, sizeof(g_dc_used));
        g_dc_init = 1;
    }
    for (int i = 0; i < DC_MAX_HANDLES; i++) {
        if (!g_dc_used[i]) {
            g_dc_used[i] = 1;
            memset(&g_dc[i], 0, sizeof(DcHandle));
            return i;
        }
    }
    return DC_ERR_HANDLE;
}

DC_API void dc_destroy(int h)
{
    if (DC_SAFE(h)) {
        memset(&g_dc[h], 0, sizeof(DcHandle));
        g_dc_used[h] = 0;
    }
}

/* ═══════════════════════════════════════════════════════════════
 * LOADING
 * ═══════════════════════════════════════════════════════════════ */

DC_API int dc_load_profile(int h, const char *path_or_name)
{
    if (!DC_SAFE(h)) return DC_ERR_HANDLE;
    DcHandle *dh = &g_dc[h];

    /* Try built-in first */
    const char *builtin = dc_get_builtin_profile(path_or_name);
    int rc;
    if (builtin) {
        rc = dc_parse_profile(builtin, &dh->profile);
    } else {
        rc = dc_load_profile_from_file(path_or_name, &dh->profile);
    }

    if (rc == DC_OK) {
        dh->profile_loaded = 1;
    } else {
        snprintf(dh->errmsg, DC_MAX_ERR, "Failed to load profile '%s' (rc=%d)",
                 path_or_name, rc);
    }
    return rc;
}

DC_API int dc_load_device(int h, const char *path)
{
    if (!DC_SAFE(h)) return DC_ERR_HANDLE;
    DcHandle *dh = &g_dc[h];

    if (!dh->profile_loaded) {
        snprintf(dh->errmsg, DC_MAX_ERR, "Load profile before device config");
        return DC_ERR_PROFILE;
    }

    int rc = dc_load_device_from_file(path, &dh->device, &dh->profile);
    if (rc == DC_OK) {
        dh->device_loaded = 1;
    } else {
        snprintf(dh->errmsg, DC_MAX_ERR, "Failed to load device config '%s' (rc=%d)",
                 path, rc);
    }
    return rc;
}

/* ═══════════════════════════════════════════════════════════════
 * VALIDATION
 * ═══════════════════════════════════════════════════════════════ */

DC_API int dc_validate(int h)
{
    if (!DC_SAFE(h)) return DC_ERR_HANDLE;
    DcHandle *dh = &g_dc[h];

    if (!dh->profile_loaded || !dh->device_loaded) {
        snprintf(dh->errmsg, DC_MAX_ERR, "Load profile and device before validation");
        return DC_ERR_PROFILE;
    }

    return dc_validate_device(&dh->profile, &dh->device, dh->errmsg, DC_MAX_ERR);
}

/* ═══════════════════════════════════════════════════════════════
 * GENERATION
 * ═══════════════════════════════════════════════════════════════ */

DC_API int dc_generate(int h, const char *output_dir)
{
    if (!DC_SAFE(h)) return DC_ERR_HANDLE;
    DcHandle *dh = &g_dc[h];

    if (!dh->profile_loaded || !dh->device_loaded) {
        snprintf(dh->errmsg, DC_MAX_ERR, "Load profile and device before generation");
        return DC_ERR_PROFILE;
    }

    int rc = dc_gen_all(&dh->profile, &dh->device, output_dir);
    if (rc != DC_OK)
        snprintf(dh->errmsg, DC_MAX_ERR, "Generation failed (rc=%d)", rc);
    return rc;
}

DC_API int dc_gen_file(int h, const char *output_dir, int file_type)
{
    if (!DC_SAFE(h)) return DC_ERR_HANDLE;
    DcHandle *dh = &g_dc[h];

    if (!dh->profile_loaded || !dh->device_loaded) {
        snprintf(dh->errmsg, DC_MAX_ERR, "Load profile and device before generation");
        return DC_ERR_PROFILE;
    }

    const DcTesterProfile *p = &dh->profile;
    const DcDeviceIR *d = &dh->device;

    switch (file_type) {
    case DC_FILE_PINMAP:    return dc_gen_pinmap(p, d, output_dir);
    case DC_FILE_MAP:       return dc_gen_map(p, d, output_dir);
    case DC_FILE_LVL:       return dc_gen_lvl(p, d, output_dir);
    case DC_FILE_TIM:       return dc_gen_tim(p, d, output_dir);
    case DC_FILE_TP:        return dc_gen_tp(p, d, output_dir);
    case DC_FILE_POWER_ON:  return dc_gen_power_on(p, d, output_dir);
    case DC_FILE_POWER_OFF: return dc_gen_power_off(p, d, output_dir);
    default:
        snprintf(dh->errmsg, DC_MAX_ERR, "Unknown file type: %d", file_type);
        return DC_ERR_WRITE;
    }
}

/* ═══════════════════════════════════════════════════════════════
 * QUERIES
 * ═══════════════════════════════════════════════════════════════ */

DC_API int dc_num_channels(int h)
{
    return DC_SAFE(h) ? g_dc[h].device.num_channels : 0;
}

DC_API int dc_num_supplies(int h)
{
    return DC_SAFE(h) ? g_dc[h].device.num_supplies : 0;
}

DC_API int dc_num_steps(int h)
{
    return DC_SAFE(h) ? g_dc[h].device.num_steps : 0;
}

DC_API const char *dc_last_error(int h)
{
    if (!DC_SAFE(h)) return "Invalid handle";
    return g_dc[h].errmsg;
}

DC_API const char *dc_profile_name(int h)
{
    if (!DC_SAFE(h)) return "";
    return g_dc[h].profile.name;
}
