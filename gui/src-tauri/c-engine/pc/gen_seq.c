/*
 * gen_seq.c — .seq text generator
 *
 * Format: "START_INDEX LAST_INDEX PATTERN_NAME\n"
 *
 * Example: "0 4 Calibration.atp\n" (5 vectors, indices 0-4)
 */

#include "pc.h"
#include <stdio.h>
#include <string.h>

int pc_gen_seq(const PcPattern *p, const char *path, const char *atp_name)
{
    if (!p || !path) return PC_ERR_FILE;
    if (p->num_vectors == 0) return PC_ERR_FORMAT;

    FILE *f = fopen(path, "w");
    if (!f) return PC_ERR_FILE;

    /* Use provided atp_name, or derive from pattern name */
    char name_buf[PC_MAX_NAME + 8];
    if (atp_name && *atp_name) {
        strncpy(name_buf, atp_name, sizeof(name_buf) - 1);
        name_buf[sizeof(name_buf) - 1] = '\0';
    } else {
        snprintf(name_buf, sizeof(name_buf), "%s.atp", p->name);
    }

    fprintf(f, "0 %d %s\n", p->num_vectors - 1, name_buf);
    fclose(f);
    return PC_OK;
}
