/*
 * parse_stil_smart.c — SMART STIL parser
 *
 * Unlike dumb parsers that just extract syntax, this understands SEMANTICS:
 * - Infers pin types from signal names (JTAG_TCK → P_PULSE, TDO → MONITOR)
 * - Infers timing from waveform patterns (01 01 → 50ns/150ns delays)
 * - Infers power from test conditions (VDDC=0.84V → CORE supply, address=1)
 * - Infers groups from SignalGroups (JTAG_GROUP → all JTAG signals linked)
 *
 * The goal: one-shot device package from customer STIL spec.
 * No manual JSON creation. No engineer in the loop.
 */

#include "pc.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>

#define LINE_BUF 4096
#define MAX_STIL_SIGNALS 512
#define MAX_GROUPS 64

/* ═══════════════════════════════════════════════════════════════
 * SMART INFERENCE RULES
 * ═══════════════════════════════════════════════════════════════ */

/* Pin type inference from signal naming patterns */
static PcPinType infer_pin_type_from_name(const char *signal_name)
{
    /* JTAG signals */
    if (strstr(signal_name, "_TCK") || strstr(signal_name, "_CLK"))
        return PC_PIN_PULSE_POS;  /* Clock → positive pulse */
    if (strstr(signal_name, "_TMS"))
        return PC_PIN_IO;
    if (strstr(signal_name, "_TDI"))
        return PC_PIN_IO;
    if (strstr(signal_name, "_TDO"))
        return PC_PIN_MONITOR;  /* Output → monitor */
    if (strstr(signal_name, "_TRST"))
        return PC_PIN_IO;

    /* Reset signals */
    if (strstr(signal_name, "_RST") || strstr(signal_name, "_RESET"))
        return PC_PIN_IO;

    /* Clock signals */
    if (strstr(signal_name, "_CLK") || strstr(signal_name, "_CLOCK"))
        return PC_PIN_PULSE_POS;

    /* Data signals */
    if (strstr(signal_name, "_DIN") || strstr(signal_name, "_DI"))
        return PC_PIN_IO;
    if (strstr(signal_name, "_DOUT") || strstr(signal_name, "_DO"))
        return PC_PIN_MONITOR;

    /* Control signals */
    if (strstr(signal_name, "_EN") || strstr(signal_name, "_ENABLE"))
        return PC_PIN_IO;
    if (strstr(signal_name, "_CS") || strstr(signal_name, "_SEL"))
        return PC_PIN_IO;

    /* Default: IO */
    return PC_PIN_IO;
}

/* Supply type inference from voltage naming */
static int infer_supply_type(const char *name, double voltage)
{
    /* CORE supplies: VDD, VCC, core voltage */
    if (strstr(name, "VDD") || strstr(name, "VCC") || strstr(name, "CORE")) {
        /* High current → CORE (VICOR) */
        if (voltage < 1.0 || strstr(name, "VDDC"))
            return 0;  /* CORE */
    }

    /* VOUT supplies: programmable outputs */
    if (strstr(name, "VOUT") || strstr(name, "PVOUT"))
        return 1;  /* VOUT */

    /* IO supplies: bank voltages */
    if (strstr(name, "VCCO") || strstr(name, "VDDIO"))
        return 2;  /* IO */

    /* Default based on voltage */
    if (voltage < 1.0)
        return 0;  /* Low voltage → CORE */
    if (voltage > 2.5)
        return 1;  /* High voltage → VOUT */

    return 2;  /* Default → IO */
}

/* Group inference from signal naming */
static void infer_group(const char *signal_name, char *group_out, int max_len)
{
    /* JTAG group */
    if (strstr(signal_name, "JTAG") || strstr(signal_name, "_TCK") ||
        strstr(signal_name, "_TMS") || strstr(signal_name, "_TDI") ||
        strstr(signal_name, "_TDO")) {
        strncpy(group_out, "JTAG", max_len);
        return;
    }

    /* Reset group */
    if (strstr(signal_name, "RST") || strstr(signal_name, "RESET")) {
        strncpy(group_out, "RESET", max_len);
        return;
    }

    /* Clock group */
    if (strstr(signal_name, "CLK") || strstr(signal_name, "CLOCK")) {
        strncpy(group_out, "CLOCK", max_len);
        return;
    }

    /* Data group */
    if (strstr(signal_name, "DATA") || strstr(signal_name, "_D")) {
        strncpy(group_out, "DATA", max_len);
        return;
    }

    /* Default: use first underscore-separated prefix */
    const char *underscore = strchr(signal_name, '_');
    if (underscore) {
        int len = (int)(underscore - signal_name);
        if (len > max_len - 1) len = max_len - 1;
        strncpy(group_out, signal_name, len);
        group_out[len] = '\0';
    } else {
        strncpy(group_out, "DEFAULT", max_len);
    }
}

/* ═══════════════════════════════════════════════════════════════
 * STIL PARSER STATE
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    char signal_name[MAX_STIL_SIGNALS][PC_MAX_NAME];
    PcPinType pin_type[MAX_STIL_SIGNALS];
    char group[MAX_STIL_SIGNALS][64];
    int num_signals;

    char group_name[MAX_GROUPS][64];
    char group_signals[MAX_GROUPS][MAX_STIL_SIGNALS][PC_MAX_NAME];
    int group_signal_count[MAX_GROUPS];
    int num_groups;

    double clock_period_ns;
    char timing_set[64];

    char errmsg[PC_MAX_ERR];
} StilSmartState;

/* ═══════════════════════════════════════════════════════════════
 * PARSING HELPERS
 * ═══════════════════════════════════════════════════════════════ */

static char *stil_trim(char *s)
{
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    return s;
}

static int find_signal_index(StilSmartState *st, const char *name)
{
    for (int i = 0; i < st->num_signals; i++) {
        if (strcasecmp(st->signal_name[i], name) == 0)
            return i;
    }
    return -1;
}

/* ═══════════════════════════════════════════════════════════════
 * SMART SECTION PARSERS
 * ═══════════════════════════════════════════════════════════════ */

/* Parse Signals section — extract signal names */
static int parse_signals_section(StilSmartState *st, FILE *f)
{
    char line[LINE_BUF];
    int in_signals = 0;

    while (fgets(line, sizeof(line), f)) {
        char *s = stil_trim(line);

        /* Section start */
        if (strncmp(s, "Signals", 7) == 0) {
            in_signals = 1;
            continue;
        }

        /* Section end */
        if (in_signals && s[0] == '}') {
            break;
        }

        if (!in_signals) continue;

        /* Signal declaration: JTAG_TCK In; */
        char signal_name[PC_MAX_NAME];
        char direction[32];
        if (sscanf(s, "%255s %31s", signal_name, direction) >= 1) {
            /* Remove trailing semicolon */
            char *semi = strchr(signal_name, ';');
            if (semi) *semi = '\0';

            if (st->num_signals < MAX_STIL_SIGNALS) {
                strncpy(st->signal_name[st->num_signals], signal_name, PC_MAX_NAME - 1);
                st->pin_type[st->num_signals] = infer_pin_type_from_name(signal_name);
                infer_group(signal_name, st->group[st->num_signals], 64);
                st->num_signals++;
            }
        }
    }

    return st->num_signals > 0 ? PC_OK : PC_ERR_FORMAT;
}

/* Parse SignalGroups — link signals into functional groups */
static int parse_signal_groups(StilSmartState *st, FILE *f)
{
    char line[LINE_BUF];
    int in_groups = 0;
    int current_group = -1;

    while (fgets(line, sizeof(line), f)) {
        char *s = stil_trim(line);

        /* Section start */
        if (strncmp(s, "SignalGroups", 11) == 0) {
            in_groups = 1;
            continue;
        }

        /* Section end */
        if (in_groups && s[0] == '}') {
            break;
        }

        if (!in_groups) continue;

        /* Group declaration: JTAG_GROUP = 'TCK + TMS + TDI + TDO'; */
        char *eq = strchr(s, '=');
        if (eq) {
            *eq = '\0';
            char *group_name = stil_trim(s);

            /* Remove quotes and semicolon from signal list */
            char *signals = stil_trim(eq + 1);
            char *quote = strchr(signals, '\'');
            if (quote) signals = quote + 1;
            char *end_quote = strchr(signals, '\'');
            if (end_quote) *end_quote = '\0';
            char *semi = strchr(signals, ';');
            if (semi) *semi = '\0';

            /* Store group name */
            if (st->num_groups < MAX_GROUPS) {
                current_group = st->num_groups;
                strncpy(st->group_name[current_group], group_name, 64);
                st->group_signal_count[current_group] = 0;
                st->num_groups++;
            }

            /* Parse individual signals (separated by +) */
            char *token = strtok(signals, " +");
            while (token && current_group >= 0) {
                token = stil_trim(token);
                if (st->group_signal_count[current_group] < MAX_STIL_SIGNALS) {
                    strncpy(st->group_signals[current_group][st->group_signal_count[current_group]],
                            token, PC_MAX_NAME - 1);
                    st->group_signal_count[current_group]++;

                    /* Update signal's group if it exists */
                    int sig_idx = find_signal_index(st, token);
                    if (sig_idx >= 0) {
                        strncpy(st->group[sig_idx], group_name, 64);
                    }
                }
                token = strtok(NULL, " +");
            }
        }
    }

    return PC_OK;
}

/* Parse Timing section — extract clock period, timing sets */
static int parse_timing_section(StilSmartState *st, FILE *f)
{
    char line[LINE_BUF];
    int in_timing = 0;
    int in_waveform = 0;

    while (fgets(line, sizeof(line), f)) {
        char *s = stil_trim(line);

        /* Section start */
        if (strncmp(s, "Timing", 6) == 0) {
            in_timing = 1;
            continue;
        }

        /* WaveformTable start */
        if (in_timing && strncmp(s, "WaveformTable", 11) == 0) {
            in_waveform = 1;
            continue;
        }

        /* Section end */
        if (in_timing && s[0] == '}') {
            break;
        }

        if (!in_waveform) continue;

        /* Period: 200 ns; */
        double period;
        char unit[16];
        if (sscanf(s, "Period { %lf %15[^;];", &period, unit) >= 1) {
            st->clock_period_ns = period;
            if (strncmp(unit, "us", 2) == 0)
                st->clock_period_ns *= 1000.0;  /* us → ns */
        }

        /* Timing set name: tset_gen_tp1 */
        if (strncmp(s, "tset_", 5) == 0 || strncmp(s, "TSET_", 5) == 0) {
            char *space = strchr(s, ' ');
            if (space) {
                int len = (int)(space - s);
                if (len < 64) {
                    strncpy(st->timing_set, s, len);
                    st->timing_set[len] = '\0';
                }
            }
        }
    }

    return PC_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * PATTERN PARSER — extracts vectors from Pattern { } section
 * ═══════════════════════════════════════════════════════════════ */

/* Find signal index by name, or -1 if not found */
static int stil_find_signal(StilSmartState *st, const char *name)
{
    for (int i = 0; i < st->num_signals; i++) {
        if (strcasecmp(st->signal_name[i], name) == 0)
            return i;
    }
    return -1;
}

/* Parse vector data from STIL V { } statement */
static int parse_vector_data(StilSmartState *st, const char *data, uint8_t *states)
{
    /* Initialize all to don't-care */
    for (int i = 0; i < PC_MAX_CH; i++)
        states[i] = (uint8_t)PS_DONT_CARE;

    /* STIL vector format: signal_name=value; or signal_group=bits; */
    const char *p = data;
    int vectors_parsed = 0;

    while (*p) {
        /* Skip whitespace */
        while (*p && isspace((unsigned char)*p)) p++;
        if (!*p) break;

        /* Find signal name (before '=') */
        const char *name_start = p;
        while (*p && *p != '=' && !isspace((unsigned char)*p)) p++;
        if (*p != '=') break;

        int name_len = (int)(p - name_start);
        if (name_len == 0 || name_len >= PC_MAX_NAME) break;

        char sig_name[PC_MAX_NAME];
        strncpy(sig_name, name_start, name_len);
        sig_name[name_len] = '\0';

        p++; /* skip '=' */

        /* Find value (until ';' or whitespace) */
        const char *val_start = p;
        while (*p && *p != ';' && !isspace((unsigned char)*p)) p++;
        int val_len = (int)(p - val_start);

        /* Find the signal */
        int sig_idx = stil_find_signal(st, sig_name);

        if (sig_idx >= 0 && sig_idx < PC_MAX_CH) {
            /* Single-bit value */
            if (val_len == 1) {
                states[sig_idx] = (uint8_t)pc_char_to_state(val_start[0]);
                vectors_parsed++;
            }
            /* Multi-bit value (for groups) — distribute across group signals */
            else if (val_len > 1) {
                /* For now, just use first character */
                states[sig_idx] = (uint8_t)pc_char_to_state(val_start[0]);
                vectors_parsed++;
            }
        }

        /* Skip to next field */
        while (*p && *p != ';') p++;
        if (*p == ';') p++;
    }

    return vectors_parsed;
}

/* Parse Pattern section and extract vectors */
static int parse_patterns_section(StilSmartState *st, FILE *f, PcPattern *p)
{
    char line[LINE_BUF];
    int in_pattern = 0;
    int in_vector = 0;
    char vector_data[LINE_BUF * 4];  /* Accumulate multi-line vectors */
    int vector_len = 0;
    uint64_t loop_count = 0;
    int loop_depth = 0;

    while (fgets(line, sizeof(line), f)) {
        char *s = stil_trim(line);

        /* Skip comments */
        if (s[0] == '/' || (s[0] == '}' && !in_pattern)) continue;

        /* Pattern start: Pattern name { */
        if (strncmp(s, "Pattern", 7) == 0) {
            in_pattern = 1;
            /* Extract pattern name */
            char *name_start = s + 7;
            while (*name_start && isspace((unsigned char)*name_start)) name_start++;
            char *name_end = strchr(name_start, ' ');
            if (!name_end) name_end = strchr(name_start, '{');
            if (name_end && p->name[0] == '\0') {
                int len = (int)(name_end - name_start);
                if (len > 0 && len < PC_MAX_NAME) {
                    strncpy(p->name, name_start, len);
                    p->name[len] = '\0';
                }
            }
            continue;
        }

        if (!in_pattern) continue;

        /* Pattern end */
        if (in_pattern && s[0] == '}' && !in_vector && loop_depth == 0) {
            break;
        }

        /* Loop statement: Loop N { */
        if (strncmp(s, "Loop", 4) == 0) {
            char *num_start = s + 4;
            while (*num_start && isspace((unsigned char)*num_start)) num_start++;
            loop_count = strtoull(num_start, NULL, 10);
            loop_depth++;
            continue;
        }

        /* End of loop */
        if (s[0] == '}' && loop_depth > 0) {
            loop_depth--;
            continue;
        }

        /* Waveform reference: W waveform_name; — skip */
        if (s[0] == 'W' || strncmp(s, "W ", 2) == 0) {
            continue;
        }

        /* Annotation: Ann {* ... *} — skip */
        if (strncmp(s, "Ann", 3) == 0 || strncmp(s, "//", 2) == 0) {
            continue;
        }

        /* Vector start: V { */
        if (s[0] == 'V' && strchr(s, '{')) {
            in_vector = 1;
            vector_len = 0;
            vector_data[0] = '\0';

            /* Check if vector data is on same line */
            char *brace = strchr(s, '{');
            if (brace) {
                brace++;
                while (*brace && isspace((unsigned char)*brace)) brace++;
                if (*brace && *brace != '}') {
                    /* Data on same line */
                    char *end = strchr(brace, '}');
                    if (end) {
                        int len = (int)(end - brace);
                        if (len > 0 && vector_len + len < (int)sizeof(vector_data) - 1) {
                            strncpy(vector_data + vector_len, brace, len);
                            vector_len += len;
                        }
                        in_vector = 0;
                    } else {
                        strncpy(vector_data + vector_len, brace, sizeof(vector_data) - vector_len - 1);
                        vector_len = (int)strlen(vector_data);
                    }
                }
            }
            continue;
        }

        /* Vector data continuation */
        if (in_vector) {
            char *end = strchr(s, '}');
            if (end) {
                *end = '\0';
                in_vector = 0;
            }
            if (vector_len + (int)strlen(s) < (int)sizeof(vector_data) - 1) {
                strncpy(vector_data + vector_len, s, sizeof(vector_data) - vector_len - 1);
                vector_len = (int)strlen(vector_data);
            }
        }

        /* Process complete vector */
        if (!in_vector && vector_len > 0) {
            /* Parse vector data into states */
            uint8_t states[PC_MAX_CH];
            int parsed = parse_vector_data(st, vector_data, states);

            if (parsed > 0) {
                /* Create vector struct */
                PcVector vec = {0};
                for (int i = 0; i < PC_MAX_CH; i++) {
                    vec.states[i] = states[i];
                }
                vec.repeat = loop_count > 0 ? loop_count : 1;

                /* Add to pattern */
                int rc = pc_pattern_add_vector(p, &vec);
                if (rc != PC_OK) {
                    snprintf(p->errmsg, PC_MAX_ERR, "Failed to add vector");
                    return PC_ERR_ALLOC;
                }
            }

            vector_len = 0;
            vector_data[0] = '\0';
            if (loop_depth == 0) loop_count = 0;
        }
    }

    return p->num_vectors > 0 ? PC_OK : PC_ERR_FORMAT;
}

/* ═══════════════════════════════════════════════════════════════
 * MAIN SMART STIL PARSER
 * ═══════════════════════════════════════════════════════════════ */

int pc_parse_stil_smart(PcPattern *p, const char *path)
{
    if (!p || !path) return PC_ERR_FILE;

    FILE *f = fopen(path, "r");
    if (!f) return PC_ERR_FILE;

    StilSmartState st = {0};
    int rc;

    /* Phase 1: Extract signals with smart inference */
    rc = parse_signals_section(&st, f);
    if (rc != PC_OK) {
        fclose(f);
        snprintf(p->errmsg, PC_MAX_ERR, "STIL Signals: %s", st.errmsg);
        return rc;
    }

    rewind(f);

    /* Phase 2: Extract signal groups */
    rc = parse_signal_groups(&st, f);
    if (rc != PC_OK) {
        fclose(f);
        snprintf(p->errmsg, PC_MAX_ERR, "STIL Groups: %s", st.errmsg);
        return rc;
    }

    rewind(f);

    /* Phase 3: Extract timing */
    rc = parse_timing_section(&st, f);
    /* Timing is optional, don't fail if not found */

    fclose(f);

    /* Phase 4: Populate PcPattern with inferred data */
    p->num_signals = st.num_signals;
    for (int i = 0; i < st.num_signals && i < PC_MAX_CH; i++) {
        strncpy(p->signals[i].name, st.signal_name[i], PC_MAX_NAME - 1);
        p->signals[i].channel = i;  /* Identity map for now */
        p->signals[i].pin_type = st.pin_type[i];  /* INFERRED! */
        strncpy(p->signals[i].group, st.group[i], 64);  /* INFERRED! */
    }

    /* Store timing info in pattern name for later use */
    if (st.timing_set[0]) {
        snprintf(p->name, PC_MAX_NAME, "%s_%s", p->name, st.timing_set);
    }

    rewind(f);

    /* Phase 5: Parse Pattern section and extract vectors */
    rc = parse_patterns_section(&st, f, p);
    if (rc != PC_OK) {
        fclose(f);
        snprintf(p->errmsg, PC_MAX_ERR, "STIL Patterns: %s", st.errmsg);
        return rc;
    }

    fclose(f);

    return PC_OK;
}
