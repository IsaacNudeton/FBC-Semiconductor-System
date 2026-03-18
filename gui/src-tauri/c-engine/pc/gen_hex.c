/*
 * gen_hex.c — .hex binary generator
 *
 * Writes 40 bytes per vector:
 *   [0:8]    VALUE low  (channels 0-63, LE)
 *   [8:16]   VALUE high (channels 64-127, LE)
 *   [16:24]  OEN low    (channels 0-63, LE)
 *   [24:32]  OEN high   (channels 64-127, LE)
 *   [32:40]  REPEAT     (uint64_t, LE)
 *
 * Optional CRC32 appended at end (4 bytes, LE).
 */

#include "pc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
 * Build a remapped vector: if the pattern has a pin map,
 * remap signal positions to channel positions.
 * If no pin map (identity), states[i] goes to channel i directly.
 */
static void build_mapped_vector(const PcPattern *p, int vec_idx, PcVector *out)
{
    const PcVector *src = &p->vectors[vec_idx];
    memset(out->states, PS_DONT_CARE, PC_MAX_CH);
    out->repeat = src->repeat;

    for (int si = 0; si < p->num_signals && si < PC_MAX_CH; si++) {
        int ch = p->signals[si].channel;
        if (ch >= 0 && ch < PC_MAX_CH)
            out->states[ch] = src->states[si];
    }
}

int pc_gen_hex(const PcPattern *p, const char *path, int append_crc)
{
    if (!p || !path) return PC_ERR_FILE;
    if (p->num_vectors == 0) return PC_ERR_FORMAT;

    FILE *f = fopen(path, "wb");
    if (!f) return PC_ERR_FILE;

    /* Allocate buffer for CRC computation if needed */
    size_t total_size = (size_t)p->num_vectors * PC_HEX_VECTOR_SIZE;
    uint8_t *buffer = NULL;

    if (append_crc) {
        buffer = (uint8_t *)malloc(total_size);
        if (!buffer) { fclose(f); return PC_ERR_ALLOC; }
    }

    for (int vi = 0; vi < p->num_vectors; vi++) {
        PcVector mapped;

        if (p->mapped) {
            build_mapped_vector(p, vi, &mapped);
        } else {
            /* No pin map: identity (signal index = channel) */
            mapped = p->vectors[vi];
        }

        PcHexVector hv;
        pc_encode_vector(&mapped, &hv);

        /* Write as raw bytes (struct is naturally packed as 5 x uint64_t) */
        uint8_t raw[PC_HEX_VECTOR_SIZE];
        memcpy(raw + 0,  &hv.value_lo, 8);
        memcpy(raw + 8,  &hv.value_hi, 8);
        memcpy(raw + 16, &hv.oen_lo,   8);
        memcpy(raw + 24, &hv.oen_hi,   8);
        memcpy(raw + 32, &hv.repeat,   8);

        if (append_crc)
            memcpy(buffer + (size_t)vi * PC_HEX_VECTOR_SIZE, raw, PC_HEX_VECTOR_SIZE);

        if (fwrite(raw, 1, PC_HEX_VECTOR_SIZE, f) != PC_HEX_VECTOR_SIZE) {
            fclose(f);
            if (buffer) free(buffer);
            return PC_ERR_WRITE;
        }
    }

    if (append_crc) {
        uint32_t crc = pc_crc32(buffer, total_size);
        uint8_t crc_bytes[4];
        crc_bytes[0] = (uint8_t)(crc & 0xFF);
        crc_bytes[1] = (uint8_t)((crc >> 8) & 0xFF);
        crc_bytes[2] = (uint8_t)((crc >> 16) & 0xFF);
        crc_bytes[3] = (uint8_t)((crc >> 24) & 0xFF);
        fwrite(crc_bytes, 1, 4, f);
        free(buffer);
    }

    fclose(f);
    return PC_OK;
}
