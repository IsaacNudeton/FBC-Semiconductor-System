/*
 * parse_pinmap.c — Pin map parser (3 formats, auto-detected)
 *
 * Format 1 — Board pin (Sonoma):
 *   B13_GPIO0 PAD_A_RSTN;
 *   B33_GPIO48 signal_name;
 *
 * Format 2 — Direct GPIO index:
 *   0 signal_name
 *   34 another_signal
 *
 * Format 3 — burnIn.cfg (VelocityCAE):
 *   PINLIST
 *   ##ATE_PINNAME  DOMAIN  TYPE  SLOT  CHANNEL  SIM_PINNAMES
 *   JTAG_TCK       default IO    1     21       jtag_tck
 *   END PINLIST
 */

#include "pc.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>

#define LINE_BUF 1024

/* ─── GPIO name → channel number ──────────────────────────── */

static int gpio_name_to_channel(const char *name)
{
    /* B13_GPIO0 through B13_GPIO47 → channels 0-47 */
    if (strncmp(name, "B13_GPIO", 8) == 0)
        return atoi(name + 8);
    /* B33_GPIO48 through B33_GPIO95 → channels 48-95 */
    if (strncmp(name, "B33_GPIO", 8) == 0)
        return atoi(name + 8);
    /* B34_GPIO96 through B34_GPIO127 → channels 96-127 */
    if (strncmp(name, "B34_GPIO", 8) == 0)
        return atoi(name + 8);
    return -1;
}

static char *trim(char *s)
{
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    return s;
}

/* ─── find signal by name ─────────────────────────────────── */

static int find_signal(PcPattern *p, const char *name)
{
    for (int i = 0; i < p->num_signals; i++) {
        /* Case-insensitive comparison */
        if (strcasecmp(p->signals[i].name, name) == 0)
            return i;
    }
    return -1;
}

/* ═══════════════════════════════════════════════════════════════
 * MAIN LOADER
 * ═══════════════════════════════════════════════════════════════ */

int pc_load_pinmap(PcPattern *p, const char *path)
{
    FILE *f = fopen(path, "r");
    if (!f) {
        snprintf(p->errmsg, PC_MAX_ERR, "Cannot open pin map: %s", path);
        return PC_ERR_FILE;
    }

    char line[LINE_BUF];
    int mapped = 0;
    int in_pinlist = 0;

    while (fgets(line, sizeof(line), f)) {
        char *s = trim(line);
        if (!*s || s[0] == '#') continue;

        /* burnIn.cfg: detect PINLIST section */
        if (strncmp(s, "PINLIST", 7) == 0) { in_pinlist = 1; continue; }
        if (strncmp(s, "END", 3) == 0 && strstr(s, "PINLIST")) { in_pinlist = 0; continue; }

        if (in_pinlist) {
            /* Format 3: ATE_NAME DOMAIN TYPE SLOT CHANNEL SIM_NAMES */
            char ate_name[PC_MAX_NAME];
            char domain[64], type[16];
            int slot, channel;
            char sim_name[PC_MAX_NAME];
            if (sscanf(s, "%255s %63s %15s %d %d %255s",
                       ate_name, domain, type, &slot, &channel, sim_name) >= 5) {
                /* Channel might be dash-separated for multi-DUT; take first */
                /* Just use the parsed channel number */
                if (channel >= 0 && channel < PC_MAX_CH) {
                    int idx = find_signal(p, ate_name);
                    if (idx < 0) idx = find_signal(p, sim_name);
                    if (idx >= 0) {
                        p->signals[idx].channel = channel;
                        mapped++;
                    }
                }
            }
            continue;
        }

        /* Auto-detect: first token numeric → Format 2, else Format 1 */
        char tok1[PC_MAX_NAME], tok2[PC_MAX_NAME];
        if (sscanf(s, "%255s %255s", tok1, tok2) < 2) continue;

        /* Strip trailing semicolons */
        char *semi = strchr(tok2, ';');
        if (semi) *semi = '\0';
        semi = strchr(tok1, ';');
        if (semi) *semi = '\0';

        if (isdigit((unsigned char)tok1[0])) {
            /* Format 2: GPIO_INDEX SIGNAL_NAME */
            int channel = atoi(tok1);
            if (channel >= 0 && channel < PC_MAX_CH) {
                int idx = find_signal(p, tok2);
                if (idx >= 0) {
                    p->signals[idx].channel = channel;
                    mapped++;
                }
            }
        } else {
            /* Format 1: BOARD_PIN SIGNAL_NAME */
            int channel = gpio_name_to_channel(tok1);
            if (channel >= 0 && channel < PC_MAX_CH) {
                int idx = find_signal(p, tok2);
                if (idx >= 0) {
                    p->signals[idx].channel = channel;
                    mapped++;
                }
            }
            /* Also skip known non-vector entries (VOUT, IOUT, ADC, XADC) */
        }
    }

    fclose(f);

    if (mapped == 0) {
        snprintf(p->errmsg, PC_MAX_ERR, "No signals mapped from %s", path);
        return PC_ERR_PINMAP;
    }

    p->mapped = 1;
    return PC_OK;
}
