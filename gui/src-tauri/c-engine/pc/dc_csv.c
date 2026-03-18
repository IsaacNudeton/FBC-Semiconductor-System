/*
 * dc_csv.c — CSV-to-device config parser
 *
 * Parses a CSV file exported from customer Excel/datasheet into DcDeviceIR.
 * Auto-detects column headers — flexible enough for real-world spreadsheets.
 *
 * Expected CSV columns (auto-detected, case-insensitive, order doesn't matter):
 *   signal/pin/name     → signal_name
 *   channel/gpio/pin#   → channel number
 *   direction/dir/io    → direction (IO/I/O/In/Out/Bidir)
 *   voltage/vio/level   → bank voltage
 *   group/bank          → signal group (optional)
 *
 * Supply rows detected by: signal name starting with "CORE" or "VDD" or "VOUT"
 * or a column named "supply/core/power".
 *
 * Delimiter: comma (,), semicolon (;), or tab (\t) — auto-detected.
 */

#include "dc.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>

#define CSV_MAX_LINE 4096
#define CSV_MAX_COLS 32
#define CSV_MAX_FIELD 256

/* ═══════════════════════════════════════════════════════════════
 * HELPERS
 * ═══════════════════════════════════════════════════════════════ */

static char *csv_trim(char *s)
{
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    /* Strip quotes */
    if (*s == '"' && end > s + 1 && *(end - 1) == '"') {
        s++;
        *(end - 1) = '\0';
    }
    return s;
}

static int csv_strcasecmp(const char *a, const char *b)
{
    while (*a && *b) {
        int ca = tolower((unsigned char)*a);
        int cb = tolower((unsigned char)*b);
        if (ca != cb) return ca - cb;
        a++; b++;
    }
    return (unsigned char)*a - (unsigned char)*b;
}

static int csv_contains_i(const char *haystack, const char *needle)
{
    if (!haystack || !needle) return 0;
    size_t hlen = strlen(haystack);
    size_t nlen = strlen(needle);
    if (nlen > hlen) return 0;
    for (size_t i = 0; i <= hlen - nlen; i++) {
        int match = 1;
        for (size_t j = 0; j < nlen; j++) {
            if (tolower((unsigned char)haystack[i + j]) !=
                tolower((unsigned char)needle[j])) {
                match = 0;
                break;
            }
        }
        if (match) return 1;
    }
    return 0;
}

/* Detect delimiter: comma, semicolon, or tab */
static char detect_delimiter(const char *line)
{
    int commas = 0, semicolons = 0, tabs = 0;
    int in_quotes = 0;
    for (const char *p = line; *p; p++) {
        if (*p == '"') in_quotes = !in_quotes;
        if (in_quotes) continue;
        if (*p == ',') commas++;
        if (*p == ';') semicolons++;
        if (*p == '\t') tabs++;
    }
    if (tabs >= commas && tabs >= semicolons && tabs > 0) return '\t';
    if (semicolons > commas && semicolons > 0) return ';';
    return ',';
}

/* Split line by delimiter, respecting quoted fields */
static int csv_split(char *line, char delim, char *fields[], int max_fields)
{
    int count = 0;
    char *p = line;
    while (*p && count < max_fields) {
        /* Skip leading whitespace */
        while (*p == ' ') p++;

        if (*p == '"') {
            /* Quoted field */
            p++;
            fields[count] = p;
            while (*p && !(*p == '"' && (*(p + 1) == delim || *(p + 1) == '\0' ||
                           *(p + 1) == '\r' || *(p + 1) == '\n')))
                p++;
            if (*p == '"') *p++ = '\0';
            if (*p == delim) p++;
        } else {
            /* Unquoted field */
            fields[count] = p;
            while (*p && *p != delim && *p != '\r' && *p != '\n')
                p++;
            if (*p == delim) { *p++ = '\0'; }
            else if (*p) { *p++ = '\0'; }
        }
        count++;
    }
    return count;
}

/* Parse direction string → int */
static int parse_direction(const char *s)
{
    if (!s || !*s) return 0;
    char *t = csv_trim((char *)s);
    if (csv_strcasecmp(t, "I") == 0 || csv_strcasecmp(t, "In") == 0 ||
        csv_strcasecmp(t, "Input") == 0 || csv_strcasecmp(t, "IN") == 0)
        return 1;
    if (csv_strcasecmp(t, "O") == 0 || csv_strcasecmp(t, "Out") == 0 ||
        csv_strcasecmp(t, "Output") == 0 || csv_strcasecmp(t, "OUT") == 0)
        return 2;
    /* IO, Bidir, Bidirectional, B → 0 */
    return 0;
}

/* Check if signal name looks like a power supply */
static int is_supply_name(const char *name)
{
    return (csv_contains_i(name, "CORE") ||
            csv_contains_i(name, "VDD")  ||
            csv_contains_i(name, "VOUT") ||
            csv_contains_i(name, "VCC")  ||
            csv_contains_i(name, "SUPPLY"));
}

/* ═══════════════════════════════════════════════════════════════
 * COLUMN DETECTION
 * ═══════════════════════════════════════════════════════════════ */

typedef enum {
    COL_UNKNOWN   = 0,
    COL_SIGNAL    = 1,
    COL_CHANNEL   = 2,
    COL_DIRECTION = 3,
    COL_VOLTAGE   = 4,
    COL_GROUP     = 5,
    COL_SUPPLY    = 6,
    COL_SEQ_ORDER = 7,
    COL_RAMP_MS   = 8,
} CsvColType;

static CsvColType detect_column(const char *header)
{
    if (!header || !*header) return COL_UNKNOWN;

    /* Signal name */
    if (csv_contains_i(header, "signal") || csv_contains_i(header, "pin_name") ||
        csv_contains_i(header, "pin name") || csv_contains_i(header, "net") ||
        csv_strcasecmp(header, "name") == 0 || csv_strcasecmp(header, "pin") == 0)
        return COL_SIGNAL;

    /* Channel/GPIO number */
    if (csv_contains_i(header, "channel") || csv_contains_i(header, "gpio") ||
        csv_contains_i(header, "pin#") || csv_contains_i(header, "pin_num") ||
        csv_contains_i(header, "pin number") || csv_strcasecmp(header, "ch") == 0)
        return COL_CHANNEL;

    /* Direction */
    if (csv_contains_i(header, "direction") || csv_strcasecmp(header, "dir") == 0 ||
        csv_strcasecmp(header, "io") == 0 || csv_contains_i(header, "type"))
        return COL_DIRECTION;

    /* Voltage */
    if (csv_contains_i(header, "voltage") || csv_contains_i(header, "vio") ||
        csv_contains_i(header, "level") || csv_strcasecmp(header, "v") == 0)
        return COL_VOLTAGE;

    /* Group/Bank */
    if (csv_contains_i(header, "group") || csv_contains_i(header, "bank") ||
        csv_contains_i(header, "domain"))
        return COL_GROUP;

    /* Supply/Core */
    if (csv_contains_i(header, "supply") || csv_contains_i(header, "core") ||
        csv_contains_i(header, "power"))
        return COL_SUPPLY;

    /* Sequence order */
    if (csv_contains_i(header, "sequence") || csv_contains_i(header, "order") ||
        csv_contains_i(header, "seq"))
        return COL_SEQ_ORDER;

    /* Ramp delay */
    if (csv_contains_i(header, "ramp") || csv_contains_i(header, "delay"))
        return COL_RAMP_MS;

    return COL_UNKNOWN;
}

/* ═══════════════════════════════════════════════════════════════
 * CSV PARSER
 * ═══════════════════════════════════════════════════════════════ */

int dc_parse_csv(const char *path, DcDeviceIR *dev, char *errmsg, int errmax)
{
    memset(dev, 0, sizeof(*dev));

    FILE *f = fopen(path, "r");
    if (!f) {
        snprintf(errmsg, errmax, "Cannot open CSV: %s", path);
        return DC_ERR_FILE;
    }

    char line[CSV_MAX_LINE];
    char *fields[CSV_MAX_COLS];
    int col_map[CSV_MAX_COLS];
    int num_cols = 0;
    char delim = ',';
    int header_found = 0;

    /* Extract device name from filename */
    const char *fname = path;
    const char *sep;
    if ((sep = strrchr(path, '/')) != NULL) fname = sep + 1;
    if ((sep = strrchr(path, '\\')) != NULL && sep + 1 > fname) fname = sep + 1;
    strncpy(dev->device_name, fname, DC_MAX_NAME - 1);
    char *dot = strrchr(dev->device_name, '.');
    if (dot) *dot = '\0';

    while (fgets(line, sizeof(line), f)) {
        /* Strip newline */
        char *nl = strchr(line, '\n');
        if (nl) *nl = '\0';
        nl = strchr(line, '\r');
        if (nl) *nl = '\0';

        /* Skip empty lines */
        char *trimmed = csv_trim(line);
        if (!*trimmed) continue;

        /* Skip comment lines */
        if (trimmed[0] == '#') continue;

        if (!header_found) {
            /* Auto-detect delimiter from first non-empty line */
            delim = detect_delimiter(trimmed);

            /* Parse header */
            char header_copy[CSV_MAX_LINE];
            strncpy(header_copy, trimmed, sizeof(header_copy) - 1);
            header_copy[sizeof(header_copy) - 1] = '\0';
            num_cols = csv_split(header_copy, delim, fields, CSV_MAX_COLS);

            int signal_col_found = 0;
            for (int i = 0; i < num_cols; i++) {
                char *h = csv_trim(fields[i]);
                col_map[i] = detect_column(h);
                if (col_map[i] == COL_SIGNAL) signal_col_found = 1;
            }

            /* Only accept as header if we found a signal column */
            if (signal_col_found) {
                header_found = 1;
                continue;
            }

            /* If first row has no recognizable headers, try treating
               first column as signal name if it's alphabetic */
            if (isalpha((unsigned char)trimmed[0])) {
                /* Assume: signal, channel, direction (positional) */
                col_map[0] = COL_SIGNAL;
                if (num_cols > 1) col_map[1] = COL_CHANNEL;
                if (num_cols > 2) col_map[2] = COL_DIRECTION;
                if (num_cols > 3) col_map[3] = COL_VOLTAGE;
                header_found = 1;
                /* Don't continue — process this line as data */
            } else {
                continue; /* Skip non-header, non-data lines */
            }
        }

        /* Parse data row */
        char row_copy[CSV_MAX_LINE];
        strncpy(row_copy, trimmed, sizeof(row_copy) - 1);
        row_copy[sizeof(row_copy) - 1] = '\0';
        int n = csv_split(row_copy, delim, fields, CSV_MAX_COLS);

        /* Extract field values */
        const char *signal_name = NULL;
        int channel = -1;
        int direction = 0;
        double voltage = 0.0;
        const char *group = NULL;
        int is_supply = 0;
        int seq_order = 0;
        double ramp_ms = 10.0;

        for (int i = 0; i < n && i < num_cols; i++) {
            char *val = csv_trim(fields[i]);
            if (!*val) continue;

            switch (col_map[i]) {
            case COL_SIGNAL:
                signal_name = val;
                if (is_supply_name(val)) is_supply = 1;
                break;
            case COL_CHANNEL:
                channel = atoi(val);
                break;
            case COL_DIRECTION:
                direction = parse_direction(val);
                break;
            case COL_VOLTAGE:
                voltage = atof(val);
                break;
            case COL_GROUP:
                group = val;
                break;
            case COL_SUPPLY:
                is_supply = 1;
                signal_name = val;
                break;
            case COL_SEQ_ORDER:
                seq_order = atoi(val);
                break;
            case COL_RAMP_MS:
                ramp_ms = atof(val);
                break;
            default:
                break;
            }
        }

        if (!signal_name || !*signal_name) continue;

        if (is_supply && voltage > 0) {
            /* Add as supply */
            if (dev->num_supplies < DC_MAX_SUPPLIES) {
                DcSupplyAssign *s = &dev->supplies[dev->num_supplies++];
                strncpy(s->core_name, signal_name, DC_MAX_NAME - 1);
                s->voltage = voltage;
                s->sequence_order = seq_order > 0 ? seq_order : dev->num_supplies;
                s->ramp_delay_ms = ramp_ms;
            }
        } else if (channel >= 0) {
            /* Add as channel */
            if (dev->num_channels < DC_MAX_CH) {
                DcChannelMap *ch = &dev->channels[dev->num_channels];
                strncpy(ch->signal_name, signal_name, DC_MAX_NAME - 1);
                ch->channel = channel;
                ch->direction = direction;
                dev->num_channels++;

                /* Track bank voltages */
                if (voltage > 0 && group) {
                    /* Store voltage for this group — simplified: use channel index */
                    for (int b = 0; b < DC_MAX_BANKS; b++) {
                        if (dev->bank_voltages[b] == 0.0) {
                            dev->bank_voltages[b] = voltage;
                            if (b >= dev->num_bank_voltages)
                                dev->num_bank_voltages = b + 1;
                            break;
                        }
                        if (dev->bank_voltages[b] == voltage) break;
                    }
                }
            }
        }
    }

    fclose(f);

    if (dev->num_channels == 0 && dev->num_supplies == 0) {
        snprintf(errmsg, errmax, "No channels or supplies found in CSV: %s", path);
        return DC_ERR_PARSE;
    }

    return DC_OK;
}
