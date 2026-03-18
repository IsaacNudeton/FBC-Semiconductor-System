/*
 * crc32.c — Table-based CRC32 (IEEE 802.3, same as zlib)
 *
 * Zero dependencies. Compatible with Python's zlib.crc32().
 */

#include "pc.h"

/* Build table at first use — avoids a 1KB static table that could have typos */
static uint32_t crc_table[256];
static int      crc_table_ready = 0;

static void crc_init_table(void)
{
    for (uint32_t i = 0; i < 256; i++) {
        uint32_t c = i;
        for (int j = 0; j < 8; j++)
            c = (c & 1) ? (0xEDB88320 ^ (c >> 1)) : (c >> 1);
        crc_table[i] = c;
    }
    crc_table_ready = 1;
}

uint32_t pc_crc32(const void *data, size_t len)
{
    if (!crc_table_ready) crc_init_table();

    const uint8_t *p = (const uint8_t *)data;
    uint32_t crc = 0xFFFFFFFF;
    for (size_t i = 0; i < len; i++)
        crc = crc_table[(crc ^ p[i]) & 0xFF] ^ (crc >> 8);
    return crc ^ 0xFFFFFFFF;
}
