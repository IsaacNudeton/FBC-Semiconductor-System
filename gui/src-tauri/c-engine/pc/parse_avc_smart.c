/*
 * parse_avc_smart.c — SMART AVC parser
 *
 * Unlike dumb parsers that just extract pin states, this understands SEMANTICS:
 * - Infers test conditions from timing set names (tset_gen_tp1 → temp=tp1)
 * - Infers clock pins from 'C' positions in vectors
 * - Infers drive/sense from pin state patterns
 * - Infers repeat counts and their meaning (R60000 = burn-in stress)
 *
 * The goal: extract device behavior from AVC, not just syntax.
 */

#include "pc.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>

#define LINE_BUF 4096
#define MAX_AVC_VECTORS 1024

/* ═══════════════════════════════════════════════════════════════
 * SMART INFERENCE RULES
 * ═══════════════════════════════════════════════════════════════ */

/* Temperature inference from timing set naming */
static int infer_temp_from_timing_set(const char *timing_set, double *temp_out)
{
    /* Common Sonoma timing set naming conventions */
    if (strstr(timing_set, "tp1") || strstr(timing_set, "TP1")) {
        *temp_out = 100.0;  /* tp1 typically = 100°C */
        return 1;
    }
    if (strstr(timing_set, "tp2") || strstr(timing_set, "TP2")) {
        *temp_out = 125.0;  /* tp2 typically = 125°C */
        return 1;
    }
    if (strstr(timing_set, "tp3") || strstr(timing_set, "TP3")) {
        *temp_out = 150.0;  /* tp3 typically = 150°C */
        return 1;
    }
    if (strstr(timing_set, "room") || strstr(timing_set, "ROOM")) {
        *temp_out = 25.0;
        return 1;
    }
    if (strstr(timing_set, "cold") || strstr(timing_set, "COLD")) {
        *temp_out = -40.0;
        return 1;
    }
    return 0;  /* Unknown */
}

/* Test type inference from vector patterns */
static int infer_test_type(const char *timing_set)
{
    if (strstr(timing_set, "burn") || strstr(timing_set, "BURN"))
        return 1;  /* Burn-in */
    if (strstr(timing_set, "func") || strstr(timing_set, "FUNC"))
        return 2;  /* Functional */
    if (strstr(timing_set, "scan") || strstr(timing_set, "SCAN"))
        return 3;  /* Scan test */
    if (strstr(timing_set, "atpg") || strstr(timing_set, "ATPG"))
        return 4;  /* ATPG */
    return 0;  /* Unknown */
}

/* Pin role inference from state patterns across vectors */
typedef struct {
    int drive_count;
    int sense_count;
    int clock_count;
    int always_high;
    int always_low;
} PinBehavior;

static void analyze_pin_behavior(PinBehavior *behav, const char *states, int num_vectors)
{
    memset(behav, 0, sizeof(*behav));

    for (int i = 0; i < num_vectors && i < MAX_AVC_VECTORS; i++) {
        char c = states[i];
        switch (c) {
            case '0': behav->always_low++; break;
            case '1': behav->always_high++; break;
            case 'C': case 'c': behav->clock_count++; break;
            case 'L': case 'H': behav->sense_count++; break;
            default: break;
        }
    }

    /* Drive = mostly 0/1, never C */
    if (behav->clock_count == 0 && (behav->always_high + behav->always_low) > num_vectors * 0.8)
        behav->drive_count = 1;
}

/* ═══════════════════════════════════════════════════════════════
 * AVC PARSER STATE
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    char timing_sets[MAX_AVC_VECTORS][64];
    int repeat_counts[MAX_AVC_VECTORS];
    char vector_data[MAX_AVC_VECTORS][512];  /* Pin states as string */
    int num_vectors;

    int pin_count;
    PinBehavior pin_behavior[256];  /* Per-pin behavior analysis */

    double inferred_temp;
    int inferred_test_type;

    char errmsg[PC_MAX_ERR];
} AvcSmartState;

/* ═══════════════════════════════════════════════════════════════
 * PARSING HELPERS
 * ═══════════════════════════════════════════════════════════════ */

static char *avc_trim(char *s)
{
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    return s;
}

static int is_vector_line(const char *line)
{
    /* Vector lines start with R followed by digit */
    if (line[0] != 'R') return 0;
    return isdigit((unsigned char)line[1]);
}

/* ═══════════════════════════════════════════════════════════════
 * SMART AVC PARSER
 * ═══════════════════════════════════════════════════════════════ */

int pc_parse_avc_smart(PcPattern *p, const char *path)
{
    if (!p || !path) return PC_ERR_FILE;

    FILE *f = fopen(path, "r");
    if (!f) return PC_ERR_FILE;

    AvcSmartState st = {0};
    char line[LINE_BUF];
    int in_format = 0;
    int pin_count = 0;

    /* Phase 1: Parse all vectors */
    while (fgets(line, sizeof(line), f)) {
        char *s = avc_trim(line);

        /* Skip comments */
        if (s[0] == '#' || s[0] == '/') continue;

        /* FORMAT header */
        if (strncmp(s, "FORMAT", 6) == 0) {
            in_format = 1;
            continue;
        }

        /* Pin count from FORMAT line */
        if (in_format && !is_vector_line(s)) {
            /* Count tokens (pin names) */
            char *token = strtok(s, " \t;");
            while (token) {
                pin_count++;
                token = strtok(NULL, " \t;");
            }
            in_format = 0;
            continue;
        }

        /* Vector line: R{count} {timing_set} {pin_states} ; comment */
        if (is_vector_line(s)) {
            if (st.num_vectors >= MAX_AVC_VECTORS) {
                fclose(f);
                snprintf(p->errmsg, PC_MAX_ERR, "Too many vectors (max %d)", MAX_AVC_VECTORS);
                return PC_ERR_FORMAT;
            }

            /* Parse repeat count */
            char *space = strchr(s, ' ');
            if (!space) continue;

            st.repeat_counts[st.num_vectors] = atoi(s + 1);  /* Skip 'R' */

            /* Parse timing set */
            char *ts_start = avc_trim(space + 1);
            char *ts_end = strchr(ts_start, ' ');
            if (ts_end) {
                int ts_len = (int)(ts_end - ts_start);
                if (ts_len < 64) {
                    strncpy(st.timing_sets[st.num_vectors], ts_start, ts_len);
                    st.timing_sets[st.num_vectors][ts_len] = '\0';
                }
            }

            /* Parse pin states (until ; or end) */
            char *states = ts_end ? avc_trim(ts_end + 1) : ts_start;
            char *semicolon = strchr(states, ';');
            if (semicolon) *semicolon = '\0';

            /* Store pin states */
            strncpy(st.vector_data[st.num_vectors], states, 511);
            st.vector_data[st.num_vectors][511] = '\0';

            st.num_vectors++;
        }
    }

    fclose(f);

    if (st.num_vectors == 0) {
        snprintf(p->errmsg, PC_MAX_ERR, "No vectors found in AVC file");
        return PC_ERR_FORMAT;
    }

    /* Phase 2: Analyze pin behavior across all vectors */
    st.pin_count = pin_count > 0 ? pin_count : (int)strlen(st.vector_data[0]);

    for (int pin = 0; pin < st.pin_count && pin < 256; pin++) {
        char pin_states[MAX_AVC_VECTORS];
        for (int v = 0; v < st.num_vectors && v < MAX_AVC_VECTORS; v++) {
            pin_states[v] = (pin < (int)strlen(st.vector_data[v]))
                ? st.vector_data[v][pin]
                : 'X';
        }
        analyze_pin_behavior(&st.pin_behavior[pin], pin_states, st.num_vectors);
    }

    /* Phase 3: Infer test conditions from first timing set */
    if (st.num_vectors > 0) {
        infer_temp_from_timing_set(st.timing_sets[0], &st.inferred_temp);
        st.inferred_test_type = infer_test_type(st.timing_sets[0]);
    }

    /* Phase 4: Populate PcPattern with inferred data */
    p->num_vectors = st.num_vectors;
    p->num_signals = st.pin_count;

    /* Create signals with INFERRED pin types */
    for (int pin = 0; pin < st.pin_count && pin < PC_MAX_CH; pin++) {
        char signal_name[PC_MAX_NAME];
        snprintf(signal_name, sizeof(signal_name), "GPIO_%d", pin);
        strncpy(p->signals[pin].name, signal_name, PC_MAX_NAME - 1);
        p->signals[pin].channel = pin;

        /* INFER pin type from behavior analysis */
        PinBehavior *behav = &st.pin_behavior[pin];
        if (behav->clock_count > st.num_vectors * 0.1) {
            p->signals[pin].pin_type = PC_PIN_PULSE_POS;  /* Clock */
        } else if (behav->sense_count > st.num_vectors * 0.5) {
            p->signals[pin].pin_type = PC_PIN_MONITOR;  /* Mostly sense */
        } else if (behav->drive_count) {
            p->signals[pin].pin_type = PC_PIN_IO;  /* Drive */
        } else {
            p->signals[pin].pin_type = PC_PIN_IO;  /* Default */
        }

        /* Store inferred group */
        if (behav->clock_count > 0)
            strncpy(p->signals[pin].group, "CLOCK", 64);
        else if (behav->sense_count > behav->drive_count)
            strncpy(p->signals[pin].group, "SENSE", 64);
        else
            strncpy(p->signals[pin].group, "DRIVE", 64);
    }

    /* Store pattern name with inferred test info */
    snprintf(p->name, PC_MAX_NAME, "AVC_%s_T%.0fC",
             st.timing_sets[0], st.inferred_temp);

    /* ═══════════════════════════════════════════════════════════════
     * Phase 5: Convert string vectors → PcVector structs
     * This is the critical step that was missing!
     * ═══════════════════════════════════════════════════════════════ */

    for (int v = 0; v < st.num_vectors; v++) {
        PcVector vec = {0};
        vec.repeat = (uint64_t)st.repeat_counts[v];

        /* Convert each character to PinState */
        const char *vec_str = st.vector_data[v];
        int len = (int)strlen(vec_str);

        for (int ch = 0; ch < len && ch < PC_MAX_CH; ch++) {
            vec.states[ch] = (uint8_t)pc_char_to_state(vec_str[ch]);
        }
        /* Fill remaining channels with don't-care */
        for (int ch = len; ch < PC_MAX_CH; ch++) {
            vec.states[ch] = (uint8_t)PS_DONT_CARE;
        }

        /* Add to pattern */
        int rc = pc_pattern_add_vector(p, &vec);
        if (rc != PC_OK) {
            snprintf(p->errmsg, PC_MAX_ERR, "Failed to add vector %d", v);
            return PC_ERR_ALLOC;
        }
    }

    return PC_OK;
}
