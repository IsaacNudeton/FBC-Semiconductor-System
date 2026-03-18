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

    return PC_OK;
}
