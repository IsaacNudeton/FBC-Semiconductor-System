/*
 * dc_json.c — JSON parser for tester profiles and device configs
 *
 * Uses vendored cJSON (MIT license) for parsing.
 * Built-in Sonoma profile embedded as C string literal.
 */

#include "dc.h"
#include "vendor/cJSON.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

/* ═══════════════════════════════════════════════════════════════
 * BUILT-IN SONOMA PROFILE
 * ═══════════════════════════════════════════════════════════════ */

static const char SONOMA_PROFILE_JSON[] =
"{"
"  \"name\": \"Sonoma\","
"  \"total_channels\": 128,"
"  \"banks\": ["
"    {\"name\": \"B13\", \"start_pin\": 0,  \"num_pins\": 48},"
"    {\"name\": \"B33\", \"start_pin\": 48, \"num_pins\": 48},"
"    {\"name\": \"B34\", \"start_pin\": 96, \"num_pins\": 32}"
"  ],"
"  \"cores\": ["
"    {\"name\": \"CORE1\", \"dac_channel\": 0, \"mio_pin\": 10, \"default_voltage\": 0.0},"
"    {\"name\": \"CORE2\", \"dac_channel\": 1, \"mio_pin\": 11, \"default_voltage\": 0.0},"
"    {\"name\": \"CORE3\", \"dac_channel\": 2, \"mio_pin\": 0,  \"default_voltage\": 0.0},"
"    {\"name\": \"CORE4\", \"dac_channel\": 3, \"mio_pin\": 9,  \"default_voltage\": 0.0},"
"    {\"name\": \"CORE5\", \"dac_channel\": 4, \"mio_pin\": 13, \"default_voltage\": 0.0},"
"    {\"name\": \"CORE6\", \"dac_channel\": 5, \"mio_pin\": 14, \"default_voltage\": 0.0}"
"  ],"
"  \"firmware_path\": \"/mnt/bin/linux_*.elf\","
"  \"vector_dir\": \"/mnt/bin/vectors\","
"  \"default_period_ns\": 200.0,"
"  \"default_drive_on_ns\": 0.0,"
"  \"default_drive_off_ns\": 90.0,"
"  \"default_compare_ns\": 100.0"
"}";

/* ═══════════════════════════════════════════════════════════════
 * HELPERS
 * ═══════════════════════════════════════════════════════════════ */

static void safe_strcpy(char *dst, const char *src, int max)
{
    if (!src) { dst[0] = '\0'; return; }
    strncpy(dst, src, max - 1);
    dst[max - 1] = '\0';
}

static const char *json_str(const cJSON *obj, const char *key)
{
    cJSON *item = cJSON_GetObjectItemCaseSensitive(obj, key);
    return (item && cJSON_IsString(item)) ? item->valuestring : NULL;
}

static int json_int(const cJSON *obj, const char *key, int def)
{
    cJSON *item = cJSON_GetObjectItemCaseSensitive(obj, key);
    return (item && cJSON_IsNumber(item)) ? item->valueint : def;
}

static double json_double(const cJSON *obj, const char *key, double def)
{
    cJSON *item = cJSON_GetObjectItemCaseSensitive(obj, key);
    return (item && cJSON_IsNumber(item)) ? item->valuedouble : def;
}

/* ═══════════════════════════════════════════════════════════════
 * PROFILE PARSER
 * ═══════════════════════════════════════════════════════════════ */

int dc_parse_profile(const char *json, DcTesterProfile *out)
{
    memset(out, 0, sizeof(*out));

    cJSON *root = cJSON_Parse(json);
    if (!root) return DC_ERR_PARSE;

    safe_strcpy(out->name, json_str(root, "name"), DC_MAX_NAME);
    out->total_channels = json_int(root, "total_channels", 128);

    /* Banks */
    cJSON *banks = cJSON_GetObjectItemCaseSensitive(root, "banks");
    if (banks && cJSON_IsArray(banks)) {
        cJSON *bank;
        cJSON_ArrayForEach(bank, banks) {
            if (out->num_banks >= DC_MAX_BANKS) break;
            DcGpioBank *b = &out->banks[out->num_banks++];
            safe_strcpy(b->name, json_str(bank, "name"), DC_MAX_NAME);
            b->start_pin = json_int(bank, "start_pin", 0);
            b->num_pins  = json_int(bank, "num_pins", 0);
        }
    }

    /* Cores */
    cJSON *cores = cJSON_GetObjectItemCaseSensitive(root, "cores");
    if (cores && cJSON_IsArray(cores)) {
        cJSON *core;
        cJSON_ArrayForEach(core, cores) {
            if (out->num_cores >= DC_MAX_SUPPLIES) break;
            DcCoreHw *c = &out->cores[out->num_cores++];
            safe_strcpy(c->name, json_str(core, "name"), DC_MAX_NAME);
            c->dac_channel     = json_int(core, "dac_channel", 0);
            c->mio_pin         = json_int(core, "mio_pin", 0);
            c->default_voltage = json_double(core, "default_voltage", 0.0);
        }
    }

    safe_strcpy(out->firmware_path, json_str(root, "firmware_path"), DC_MAX_NAME);
    safe_strcpy(out->vector_dir, json_str(root, "vector_dir"), DC_MAX_NAME);
    out->default_period_ns   = json_double(root, "default_period_ns", 200.0);
    out->default_drive_on_ns = json_double(root, "default_drive_on_ns", 0.0);
    out->default_drive_off_ns = json_double(root, "default_drive_off_ns", 90.0);
    out->default_compare_ns  = json_double(root, "default_compare_ns", 100.0);

    cJSON_Delete(root);
    return DC_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * DEVICE CONFIG PARSER
 * ═══════════════════════════════════════════════════════════════ */

int dc_parse_device(const char *json, DcDeviceIR *out, const DcTesterProfile *prof)
{
    memset(out, 0, sizeof(*out));

    cJSON *root = cJSON_Parse(json);
    if (!root) return DC_ERR_PARSE;

    safe_strcpy(out->device_name, json_str(root, "device_name"), DC_MAX_NAME);
    safe_strcpy(out->lot_id, json_str(root, "lot_id"), DC_MAX_NAME);

    /* Channels */
    cJSON *channels = cJSON_GetObjectItemCaseSensitive(root, "channels");
    if (channels && cJSON_IsArray(channels)) {
        cJSON *ch;
        cJSON_ArrayForEach(ch, channels) {
            if (out->num_channels >= DC_MAX_CH) break;
            DcChannelMap *m = &out->channels[out->num_channels++];
            safe_strcpy(m->signal_name, json_str(ch, "signal_name"), DC_MAX_NAME);
            m->channel   = json_int(ch, "channel", -1);
            m->direction = json_int(ch, "direction", 0);
        }
    }

    /* Supplies */
    cJSON *supplies = cJSON_GetObjectItemCaseSensitive(root, "supplies");
    if (supplies && cJSON_IsArray(supplies)) {
        cJSON *sup;
        cJSON_ArrayForEach(sup, supplies) {
            if (out->num_supplies >= DC_MAX_SUPPLIES) break;
            DcSupplyAssign *s = &out->supplies[out->num_supplies++];
            safe_strcpy(s->core_name, json_str(sup, "core_name"), DC_MAX_NAME);
            s->voltage        = json_double(sup, "voltage", 0.0);
            s->sequence_order = json_int(sup, "sequence_order", 0);
            s->ramp_delay_ms  = json_double(sup, "ramp_delay_ms", 10.0);
        }
    }

    /* Bank voltages — keyed by bank name, matched to profile bank indices */
    cJSON *bv = cJSON_GetObjectItemCaseSensitive(root, "bank_voltages");
    if (bv && cJSON_IsObject(bv) && prof) {
        out->num_bank_voltages = prof->num_banks;
        for (int i = 0; i < prof->num_banks; i++) {
            cJSON *v = cJSON_GetObjectItemCaseSensitive(bv, prof->banks[i].name);
            out->bank_voltages[i] = (v && cJSON_IsNumber(v)) ? v->valuedouble : 0.0;
        }
    }

    /* Test steps */
    cJSON *steps = cJSON_GetObjectItemCaseSensitive(root, "steps");
    if (steps && cJSON_IsArray(steps)) {
        cJSON *step;
        cJSON_ArrayForEach(step, steps) {
            if (out->num_steps >= DC_MAX_STEPS) break;
            DcTestStep *ts = &out->steps[out->num_steps++];
            safe_strcpy(ts->pattern_name, json_str(step, "pattern_name"), DC_MAX_NAME);
            safe_strcpy(ts->pattern_file, json_str(step, "pattern_file"), DC_MAX_NAME);
            ts->loop_count = json_int(step, "loop_count", 1);
        }
    }

    /* Timing overrides */
    out->period_ns    = json_double(root, "period_ns", 0.0);
    out->drive_on_ns  = json_double(root, "drive_on_ns", 0.0);
    out->drive_off_ns = json_double(root, "drive_off_ns", 0.0);
    out->compare_ns   = json_double(root, "compare_ns", 0.0);

    cJSON_Delete(root);
    return DC_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * BUILT-IN PROFILE LOOKUP
 * ═══════════════════════════════════════════════════════════════ */

const char *dc_get_builtin_profile(const char *name)
{
    if (!name) return NULL;
    if (strcasecmp(name, "sonoma") == 0)
        return SONOMA_PROFILE_JSON;
    return NULL;
}

/* ═══════════════════════════════════════════════════════════════
 * FILE LOADERS
 * ═══════════════════════════════════════════════════════════════ */

static char *read_file(const char *path)
{
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (len <= 0) { fclose(f); return NULL; }
    char *buf = (char *)malloc(len + 1);
    if (!buf) { fclose(f); return NULL; }
    size_t read = fread(buf, 1, len, f);
    fclose(f);
    buf[read] = '\0';
    return buf;
}

int dc_load_profile_from_file(const char *path, DcTesterProfile *out)
{
    char *json = read_file(path);
    if (!json) return DC_ERR_FILE;
    int rc = dc_parse_profile(json, out);
    free(json);
    return rc;
}

int dc_load_device_from_file(const char *path, DcDeviceIR *out, const DcTesterProfile *prof)
{
    char *json = read_file(path);
    if (!json) return DC_ERR_FILE;
    int rc = dc_parse_device(json, out, prof);
    free(json);
    return rc;
}
