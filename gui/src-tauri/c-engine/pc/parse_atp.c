/*
 * parse_atp.c — ATP pattern file parser (hand-written, no regex)
 *
 * ATP format:
 *   (
 *     signal_name_0
 *     signal_name_1
 *     ...
 *   )
 *   # comment
 *   {
 *     > CycleName XLLHH0011...;
 *     repeat 100 > CycleName XLLHH0011...;
 *   }
 *
 * Also handles:
 *   - AVC-style lines: R1 timing_set XLLHH0011...
 *   - STIL \rN repeats: 110\r5 X0011 → 110XXXXX0011
 */

#include "pc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#define LINE_BUF 4096

/* ─── helpers ─────────────────────────────────────────────── */

static char *trim(char *s)
{
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    return s;
}

static int is_state_char(char c)
{
    return c == '0' || c == '1' ||
           c == 'L' || c == 'l' ||
           c == 'H' || c == 'h' ||
           c == 'X' || c == 'x' ||
           c == 'Z' || c == 'z' ||
           c == 'P' || c == 'p' ||
           c == 'N' || c == 'n' ||
           c == 'U' || c == 'u' ||
           c == 'D' || c == 'd' ||
           c == 'T' || c == 't' ||
           c == 'C' || c == 'c' ||
           c == '.';
}

/*
 * Expand STIL \rN notation in-place.
 * Example: "110\r5 X0011" → "110XXXXX0011"
 * The \rN means "repeat the NEXT state character N times".
 */
static int expand_stil_repeats(const char *src, char *dst, int dst_max)
{
    int di = 0;
    int si = 0;
    int slen = (int)strlen(src);

    while (si < slen && di < dst_max - 1) {
        if (src[si] == '\\' && si + 1 < slen && (src[si+1] == 'r' || src[si+1] == 'R')) {
            si += 2; /* skip \r */
            /* parse repeat count */
            int rpt = 0;
            while (si < slen && isdigit((unsigned char)src[si]))
                rpt = rpt * 10 + (src[si++] - '0');
            /* skip whitespace to find the character to repeat */
            while (si < slen && src[si] == ' ') si++;
            if (si < slen && is_state_char(src[si])) {
                char ch = src[si++];
                for (int r = 0; r < rpt && di < dst_max - 1; r++)
                    dst[di++] = ch;
            }
        } else {
            dst[di++] = src[si++];
        }
    }
    dst[di] = '\0';
    return di;
}

/*
 * Extract vector data from a line (strip spaces, non-state chars).
 * Returns the number of state characters extracted.
 */
static int extract_vector_chars(const char *src, char *out, int out_max)
{
    int n = 0;
    for (int i = 0; src[i] && n < out_max - 1; i++) {
        if (is_state_char(src[i]))
            out[n++] = src[i];
    }
    out[n] = '\0';
    return n;
}

/* ─── parser states ───────────────────────────────────────── */

typedef enum {
    PARSE_INIT,
    PARSE_HEADER,
    PARSE_BODY
} ParseState;

int pc_parse_atp(PcPattern *p, const char *path)
{
    FILE *f = fopen(path, "r");
    if (!f) {
        snprintf(p->errmsg, PC_MAX_ERR, "Cannot open: %s", path);
        return PC_ERR_FILE;
    }

    /* Set pattern name from filename */
    const char *fname = path;
    const char *sep;
    if ((sep = strrchr(path, '/')) != NULL) fname = sep + 1;
    if ((sep = strrchr(path, '\\')) != NULL && sep + 1 > fname) fname = sep + 1;
    strncpy(p->name, fname, PC_MAX_NAME - 1);

    /* Strip extension from name */
    char *dot = strrchr(p->name, '.');
    if (dot) *dot = '\0';

    char line[LINE_BUF];
    char expanded[LINE_BUF * 2];
    char vec_chars[PC_MAX_SIG + 16];
    ParseState state = PARSE_INIT;

    while (fgets(line, sizeof(line), f)) {
        char *s = trim(line);
        if (!*s) continue;

        switch (state) {
        case PARSE_INIT:
            if (s[0] == '(') {
                state = PARSE_HEADER;
                /* If there's a signal name on same line as '(', skip it */
            }
            break;

        case PARSE_HEADER:
            if (s[0] == ')') {
                state = PARSE_BODY;
            } else {
                /* Each line is a signal name */
                pc_pattern_add_signal(p, s);
            }
            break;

        case PARSE_BODY:
            if (s[0] == '#' || s[0] == '}') continue;
            if (s[0] == '{') continue;

            /* Try ATP format: [repeat N] > CycleName DATA; */
            {
                uint64_t repeat = 0;
                const char *cursor = s;

                /* Check for "repeat N" prefix */
                if (strncmp(cursor, "repeat", 6) == 0 && isspace((unsigned char)cursor[6])) {
                    cursor += 6;
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;
                    while (*cursor && isdigit((unsigned char)*cursor))
                        repeat = repeat * 10 + (uint64_t)(*cursor++ - '0');
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;
                }

                /* Check for '>' marker (ATP format) */
                if (*cursor == '>') {
                    cursor++;
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;
                    /* Skip cycle name */
                    while (*cursor && !isspace((unsigned char)*cursor)) cursor++;
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;

                    /* Rest is vector data (up to ';') */
                    char raw[LINE_BUF];
                    int ri = 0;
                    while (*cursor && *cursor != ';' && ri < LINE_BUF - 1)
                        raw[ri++] = *cursor++;
                    raw[ri] = '\0';

                    /* Expand \rN repeats */
                    expand_stil_repeats(raw, expanded, sizeof(expanded));

                    /* Extract state characters */
                    int nch = extract_vector_chars(expanded, vec_chars, sizeof(vec_chars));

                    if (nch > 0) {
                        PcVector v;
                        memset(&v, 0, sizeof(v));
                        v.repeat = repeat;
                        int limit = nch < p->num_signals ? nch : p->num_signals;
                        if (limit > PC_MAX_CH) limit = PC_MAX_CH;
                        for (int i = 0; i < limit; i++)
                            v.states[i] = (uint8_t)pc_char_to_state(vec_chars[i]);
                        /* Unmapped channels default to DONT_CARE (0 from memset maps to DRIVE_0,
                           but we want DONT_CARE for safety) */
                        for (int i = limit; i < PC_MAX_CH; i++)
                            v.states[i] = PS_DONT_CARE;
                        pc_pattern_add_vector(p, &v);
                    }
                    continue;
                }

                /* Try AVC format: R<repeat> <timing> <data> */
                if (s[0] == 'R' && isdigit((unsigned char)s[1])) {
                    cursor = s + 1;
                    repeat = 0;
                    while (*cursor && isdigit((unsigned char)*cursor))
                        repeat = repeat * 10 + (uint64_t)(*cursor++ - '0');
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;
                    /* Skip timing set name */
                    while (*cursor && !isspace((unsigned char)*cursor)) cursor++;
                    while (*cursor && isspace((unsigned char)*cursor)) cursor++;

                    expand_stil_repeats(cursor, expanded, sizeof(expanded));
                    int nch = extract_vector_chars(expanded, vec_chars, sizeof(vec_chars));

                    if (nch > 0) {
                        PcVector v;
                        memset(&v, 0, sizeof(v));
                        v.repeat = repeat;
                        int limit = nch < p->num_signals ? nch : p->num_signals;
                        if (limit > PC_MAX_CH) limit = PC_MAX_CH;
                        for (int i = 0; i < limit; i++)
                            v.states[i] = (uint8_t)pc_char_to_state(vec_chars[i]);
                        for (int i = limit; i < PC_MAX_CH; i++)
                            v.states[i] = PS_DONT_CARE;
                        pc_pattern_add_vector(p, &v);
                    }
                    continue;
                }

                /* Try raw space-separated format: X X 0 1 L H */
                {
                    int nch = extract_vector_chars(s, vec_chars, sizeof(vec_chars));
                    if (nch >= 2) {
                        PcVector v;
                        memset(&v, 0, sizeof(v));
                        v.repeat = 0;
                        int limit = nch < p->num_signals ? nch : p->num_signals;
                        if (limit > PC_MAX_CH) limit = PC_MAX_CH;
                        for (int i = 0; i < limit; i++)
                            v.states[i] = (uint8_t)pc_char_to_state(vec_chars[i]);
                        for (int i = limit; i < PC_MAX_CH; i++)
                            v.states[i] = PS_DONT_CARE;
                        pc_pattern_add_vector(p, &v);
                    }
                }
            }
            break;
        }
    }

    fclose(f);

    if (p->num_signals == 0) {
        snprintf(p->errmsg, PC_MAX_ERR, "No signals found in %s", path);
        return PC_ERR_PARSE;
    }
    if (p->num_vectors == 0) {
        snprintf(p->errmsg, PC_MAX_ERR, "No vectors found in %s", path);
        return PC_ERR_PARSE;
    }

    return PC_OK;
}
