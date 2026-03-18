/*
 * main.c — CLI entry point for pattern_converter
 *
 * Usage:
 *   pattern_converter <input_file> [options]
 *     -m, --map <path>     Pin map file
 *     -o, --output <path>  Output base name (default: input stem)
 *     -f, --format <fmt>   Input format: atp|stil|avc (default: auto)
 *     --crc                Append CRC32 to hex
 *     -v, --verbose        Verbose output
 *
 *   pattern_converter --config <device.json> [options]
 *     --profile <name|path>  Tester profile (default: sonoma)
 *     --output-dir <path>    Output directory for config files
 *     --gen <type>           Generate single file (pinmap|map|lvl|tim|tp|poweron|poweroff)
 *     -v, --verbose          Verbose output
 */

#include "pc.h"
#include "dc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define PC_VERSION "1.0.0"

static void usage(void)
{
    fprintf(stderr,
        "pattern_converter v%s\n"
        "\n"
        "Mode 1 — Pattern Conversion:\n"
        "  pattern_converter <input_file> [options]\n"
        "    -m, --map <path>       Pin map file\n"
        "    -o, --output <path>    Output base name\n"
        "    -f, --format <fmt>     atp|stil|avc (default: auto)\n"
        "    --crc                  Append CRC32 to hex\n"
        "    -v, --verbose          Verbose output\n"
        "\n"
        "Mode 2 — Device Config Generation:\n"
        "  pattern_converter --config <device.json> [options]\n"
        "  pattern_converter --csv <pins.csv> [options]\n"
        "    --profile <name|path>  Tester profile (default: sonoma)\n"
        "    --output-dir <path>    Output directory (default: .)\n"
        "    --gen <type>           Single file: pinmap|map|lvl|tim|tp|poweron|poweroff\n"
        "    -v, --verbose          Verbose output\n",
        PC_VERSION);
}

static InputFormat parse_format(const char *s)
{
    if (strcasecmp(s, "atp") == 0)  return FMT_ATP;
    if (strcasecmp(s, "stil") == 0) return FMT_STIL;
    if (strcasecmp(s, "avc") == 0)  return FMT_AVC;
    return FMT_AUTO;
}

/* Extract base name (no extension) from path */
static void get_base(const char *path, char *base, int max)
{
    const char *fname = path;
    const char *sep;
    if ((sep = strrchr(path, '/')) != NULL) fname = sep + 1;
    if ((sep = strrchr(path, '\\')) != NULL && sep + 1 > fname) fname = sep + 1;
    strncpy(base, fname, max - 1);
    base[max - 1] = '\0';
    char *dot = strrchr(base, '.');
    if (dot) *dot = '\0';
}

/* Extract directory from path */
static void get_dir(const char *path, char *dir, int max)
{
    const char *last_sep = NULL;
    for (const char *p = path; *p; p++)
        if (*p == '/' || *p == '\\') last_sep = p;

    if (last_sep) {
        int len = (int)(last_sep - path + 1);
        if (len >= max) len = max - 1;
        memcpy(dir, path, len);
        dir[len] = '\0';
    } else {
        dir[0] = '\0';
    }
}

/* ═══════════════════════════════════════════════════════════════
 * DEVICE CONFIG MODE
 * ═══════════════════════════════════════════════════════════════ */

static int parse_gen_type(const char *s)
{
    if (strcasecmp(s, "pinmap") == 0)   return DC_FILE_PINMAP;
    if (strcasecmp(s, "map") == 0)      return DC_FILE_MAP;
    if (strcasecmp(s, "lvl") == 0)      return DC_FILE_LVL;
    if (strcasecmp(s, "tim") == 0)      return DC_FILE_TIM;
    if (strcasecmp(s, "tp") == 0)       return DC_FILE_TP;
    if (strcasecmp(s, "poweron") == 0)  return DC_FILE_POWER_ON;
    if (strcasecmp(s, "poweroff") == 0) return DC_FILE_POWER_OFF;
    return -1;
}

static int run_config_mode(const char *config_path, const char *profile_name,
                           const char *output_dir, int gen_type, int verbose)
{
    DcTesterProfile prof;
    DcDeviceIR dev;

    /* Load profile */
    if (verbose) printf("Loading profile: %s\n", profile_name);
    const char *builtin = dc_get_builtin_profile(profile_name);
    int rc;
    if (builtin) {
        rc = dc_parse_profile(builtin, &prof);
    } else {
        rc = dc_load_profile_from_file(profile_name, &prof);
    }
    if (rc != DC_OK) {
        fprintf(stderr, "Profile error (%d): failed to load '%s'\n", rc, profile_name);
        return 1;
    }
    if (verbose)
        printf("  Profile: %s (%d channels, %d banks, %d cores)\n",
               prof.name, prof.total_channels, prof.num_banks, prof.num_cores);

    /* Load device config */
    if (verbose) printf("Loading device config: %s\n", config_path);
    rc = dc_load_device_from_file(config_path, &dev, &prof);
    if (rc != DC_OK) {
        fprintf(stderr, "Device config error (%d): failed to load '%s'\n", rc, config_path);
        return 1;
    }
    if (verbose)
        printf("  Device: %s (%d channels, %d supplies, %d steps)\n",
               dev.device_name, dev.num_channels, dev.num_supplies, dev.num_steps);

    /* Validate */
    char errmsg[DC_MAX_ERR];
    rc = dc_validate_device(&prof, &dev, errmsg, DC_MAX_ERR);
    if (rc != DC_OK) {
        fprintf(stderr, "Validation error: %s\n", errmsg);
        return 1;
    }

    /* Generate */
    if (gen_type >= 0) {
        switch (gen_type) {
        case DC_FILE_PINMAP:    rc = dc_gen_pinmap(&prof, &dev, output_dir); break;
        case DC_FILE_MAP:       rc = dc_gen_map(&prof, &dev, output_dir); break;
        case DC_FILE_LVL:       rc = dc_gen_lvl(&prof, &dev, output_dir); break;
        case DC_FILE_TIM:       rc = dc_gen_tim(&prof, &dev, output_dir); break;
        case DC_FILE_TP:        rc = dc_gen_tp(&prof, &dev, output_dir); break;
        case DC_FILE_POWER_ON:  rc = dc_gen_power_on(&prof, &dev, output_dir); break;
        case DC_FILE_POWER_OFF: rc = dc_gen_power_off(&prof, &dev, output_dir); break;
        default:                rc = DC_ERR_WRITE; break;
        }
    } else {
        if (verbose) printf("Generating all config files → %s\n", output_dir);
        rc = dc_gen_all(&prof, &dev, output_dir);
    }

    if (rc != DC_OK) {
        fprintf(stderr, "Generation failed (%d)\n", rc);
        return 1;
    }

    printf("OK: device config for %s → %s (%d channels, %d supplies, %d steps)\n",
           dev.device_name, output_dir, dev.num_channels, dev.num_supplies, dev.num_steps);
    return 0;
}

/* ═══════════════════════════════════════════════════════════════
 * MAIN
 * ═══════════════════════════════════════════════════════════════ */

int main(int argc, char **argv)
{
    if (argc < 2) { usage(); return 1; }

    const char *input_path = NULL;
    const char *map_path = NULL;
    const char *output_base = NULL;
    InputFormat format = FMT_AUTO;
    int append_crc = 0;
    int verbose = 0;

    /* Device config mode args */
    const char *config_path = NULL;
    const char *csv_path = NULL;
    const char *profile_name = "sonoma";
    const char *output_dir = ".";
    int gen_type = -1;  /* -1 = all */

    /* Parse args */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--config") == 0) {
            if (++i < argc) config_path = argv[i];
        } else if (strcmp(argv[i], "--csv") == 0) {
            if (++i < argc) csv_path = argv[i];
        } else if (strcmp(argv[i], "--profile") == 0) {
            if (++i < argc) profile_name = argv[i];
        } else if (strcmp(argv[i], "--output-dir") == 0) {
            if (++i < argc) output_dir = argv[i];
        } else if (strcmp(argv[i], "--gen") == 0) {
            if (++i < argc) gen_type = parse_gen_type(argv[i]);
        } else if (strcmp(argv[i], "-m") == 0 || strcmp(argv[i], "--map") == 0) {
            if (++i < argc) map_path = argv[i];
        } else if (strcmp(argv[i], "-o") == 0 || strcmp(argv[i], "--output") == 0) {
            if (++i < argc) output_base = argv[i];
        } else if (strcmp(argv[i], "-f") == 0 || strcmp(argv[i], "--format") == 0) {
            if (++i < argc) format = parse_format(argv[i]);
        } else if (strcmp(argv[i], "--crc") == 0) {
            append_crc = 1;
        } else if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--verbose") == 0) {
            verbose = 1;
        } else if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            usage(); return 0;
        } else if (!input_path) {
            input_path = argv[i];
        } else {
            fprintf(stderr, "Unknown argument: %s\n", argv[i]);
            usage(); return 1;
        }
    }

    /* CSV mode: parse CSV → DcDeviceIR → generate config files */
    if (csv_path) {
        DcTesterProfile prof;
        const char *builtin = dc_get_builtin_profile(profile_name);
        int rc;
        if (builtin) rc = dc_parse_profile(builtin, &prof);
        else rc = dc_load_profile_from_file(profile_name, &prof);
        if (rc != DC_OK) {
            fprintf(stderr, "Profile error (%d): failed to load '%s'\n", rc, profile_name);
            return 1;
        }
        if (verbose)
            printf("Profile: %s (%d channels)\n", prof.name, prof.total_channels);

        DcDeviceIR dev;
        char errmsg[DC_MAX_ERR];
        if (verbose) printf("Parsing CSV: %s\n", csv_path);
        rc = dc_parse_csv(csv_path, &dev, errmsg, DC_MAX_ERR);
        if (rc != DC_OK) {
            fprintf(stderr, "CSV error: %s\n", errmsg);
            return 1;
        }
        if (verbose)
            printf("  Device: %s (%d channels, %d supplies)\n",
                   dev.device_name, dev.num_channels, dev.num_supplies);

        rc = dc_validate_device(&prof, &dev, errmsg, DC_MAX_ERR);
        if (rc != DC_OK) {
            fprintf(stderr, "Validation error: %s\n", errmsg);
            return 1;
        }

        if (verbose) printf("Generating config files → %s\n", output_dir);
        rc = dc_gen_all(&prof, &dev, output_dir);
        if (rc != DC_OK) {
            fprintf(stderr, "Generation failed (%d)\n", rc);
            return 1;
        }
        printf("OK: CSV → %s (%d channels, %d supplies)\n",
               dev.device_name, dev.num_channels, dev.num_supplies);
        return 0;
    }

    /* JSON device config mode */
    if (config_path)
        return run_config_mode(config_path, profile_name, output_dir, gen_type, verbose);

    if (!input_path) { fprintf(stderr, "No input file.\n"); usage(); return 1; }

    /* Initialize pattern */
    PcPattern pat;
    pc_pattern_init(&pat, "");

    /* Parse input */
    if (verbose) printf("Parsing: %s\n", input_path);

    int rc;
    switch (format == FMT_AUTO ? FMT_ATP : format) {
    case FMT_ATP:
        rc = pc_parse_atp(&pat, input_path);
        break;
    default:
        fprintf(stderr, "Format not yet implemented\n");
        return 1;
    }

    if (rc != PC_OK) {
        fprintf(stderr, "Parse error (%d): %s\n", rc, pat.errmsg);
        pc_pattern_free(&pat);
        return 1;
    }

    if (verbose)
        printf("  %d signals, %d vectors\n", pat.num_signals, pat.num_vectors);

    /* Load pin map or apply identity */
    if (map_path) {
        if (verbose) printf("Loading pin map: %s\n", map_path);
        rc = pc_load_pinmap(&pat, map_path);
        if (rc != PC_OK) {
            fprintf(stderr, "Pin map error (%d): %s\n", rc, pat.errmsg);
            pc_pattern_free(&pat);
            return 1;
        }
    } else {
        pc_apply_identity_map(&pat);
    }

    /* Build output paths */
    char hex_path[768], seq_path[768];

    if (output_base) {
        /* -o given: use it directly (may include path) */
        snprintf(hex_path, sizeof(hex_path), "%s.hex", output_base);
        snprintf(seq_path, sizeof(seq_path), "%s.seq", output_base);
    } else {
        /* No -o: output next to input file */
        char dir[512] = "";
        char base[256];
        get_dir(input_path, dir, sizeof(dir));
        get_base(input_path, base, sizeof(base));
        snprintf(hex_path, sizeof(hex_path), "%s%s.hex", dir, base);
        snprintf(seq_path, sizeof(seq_path), "%s%s.seq", dir, base);
    }

    /* Generate hex */
    if (verbose) printf("Generating: %s\n", hex_path);
    rc = pc_gen_hex(&pat, hex_path, append_crc);
    if (rc != PC_OK) {
        fprintf(stderr, "Hex generation failed (%d)\n", rc);
        pc_pattern_free(&pat);
        return 1;
    }

    /* Generate seq */
    if (verbose) printf("Generating: %s\n", seq_path);
    char atp_name[PC_MAX_NAME + 8];
    get_base(input_path, atp_name, sizeof(atp_name) - 8);
    strcat(atp_name, ".atp");
    rc = pc_gen_seq(&pat, seq_path, atp_name);
    if (rc != PC_OK) {
        fprintf(stderr, "Seq generation failed (%d)\n", rc);
        pc_pattern_free(&pat);
        return 1;
    }

    printf("OK: %d vectors → %s (%zu bytes), %s\n",
           pat.num_vectors, hex_path,
           (size_t)pat.num_vectors * PC_HEX_VECTOR_SIZE,
           seq_path);

    pc_pattern_free(&pat);
    return 0;
}
