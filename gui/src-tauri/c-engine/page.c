/*
 * page.c — Page management and buffer pool
 *
 * Every database operation goes through here. Pages are 4KB.
 * Buffer pool caches recently used pages with LRU eviction.
 * Free pages are tracked via a linked list stored in page headers.
 */

#define _POSIX_C_SOURCE 199309L
#include "lrm_db.h"
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <io.h>
#include <windows.h>
#else
#include <unistd.h>
#endif

/* ── Page Header Read/Write ─────────────────────────────── */

static void page_read_header(const Page *p, PageHeader *h) {
    memcpy(&h->page_type,  &p->data[0],  2);
    memcpy(&h->num_cells,  &p->data[2],  2);
    memcpy(&h->right_ptr,  &p->data[4],  4);
    memcpy(&h->next_page,  &p->data[8],  4);
    memcpy(&h->free_space, &p->data[12], 4);
}

static void page_write_header(Page *p, const PageHeader *h) {
    memcpy(&p->data[0],  &h->page_type,  2);
    memcpy(&p->data[2],  &h->num_cells,  2);
    memcpy(&p->data[4],  &h->right_ptr,  4);
    memcpy(&p->data[8],  &h->next_page,  4);
    memcpy(&p->data[12], &h->free_space, 4);
}

static void page_init(Page *p, PageType type) {
    memset(p->data, 0, PAGE_SIZE);
    PageHeader h = {0};
    h.page_type = (uint16_t)type;
    h.free_space = PAGE_SIZE - PAGE_HEADER_SZ;
    page_write_header(p, &h);
}

/* ── Disk I/O ───────────────────────────────────────────── */

static int disk_read_page(FILE *fp, uint32_t page_id, Page *p) {
    long offset = (long)page_id * PAGE_SIZE;
    if (fseek(fp, offset, SEEK_SET) != 0) return LRM_ERR_IO;
    size_t n = fread(p->data, 1, PAGE_SIZE, fp);
    if (n != PAGE_SIZE) {
        /* page beyond file — return zeroed page */
        if (feof(fp)) {
            memset(p->data, 0, PAGE_SIZE);
            return LRM_OK;
        }
        return LRM_ERR_IO;
    }
    return LRM_OK;
}

static int disk_write_page(FILE *fp, uint32_t page_id, const Page *p) {
    long offset = (long)page_id * PAGE_SIZE;
    if (fseek(fp, offset, SEEK_SET) != 0) return LRM_ERR_IO;
    size_t n = fwrite(p->data, 1, PAGE_SIZE, fp);
    if (n != PAGE_SIZE) return LRM_ERR_IO;
    fflush(fp);
    /* fsync: push past OS buffer cache to durable storage */
#ifdef _WIN32
    _commit(_fileno(fp));
#else
    fsync(fileno(fp));
#endif
    return LRM_OK;
}

/* ── Buffer Pool ────────────────────────────────────────── */

int pool_init(BufferPool *pool, FILE *fp) {
    memset(pool, 0, sizeof(BufferPool));
    pool->fp = fp;
    pool->tick = 1;

    /* determine total pages on disk */
    fseek(fp, 0, SEEK_END);
    long size = ftell(fp);
    pool->total_pages = (size > 0) ? (uint32_t)(size / PAGE_SIZE) : 0;

    return LRM_OK;
}

/* Find slot for page_id, or evict LRU to make room */
static int pool_slot(BufferPool *pool, uint32_t page_id) {
    /* check if already cached */
    for (int i = 0; i < POOL_SIZE; i++) {
        if (pool->occupied[i] && pool->page_ids[i] == page_id) {
            pool->access_tick[i] = pool->tick++;
            return i;
        }
    }

    /* find empty slot */
    for (int i = 0; i < POOL_SIZE; i++) {
        if (!pool->occupied[i]) {
            return i;
        }
    }

    /* evict LRU */
    uint32_t min_tick = UINT32_MAX;
    int victim = 0;
    for (int i = 0; i < POOL_SIZE; i++) {
        if (pool->access_tick[i] < min_tick) {
            min_tick = pool->access_tick[i];
            victim = i;
        }
    }

    /* flush victim if dirty */
    if (pool->dirty[victim]) {
        disk_write_page(pool->fp, pool->page_ids[victim], &pool->pages[victim]);
        pool->dirty[victim] = false;
    }

    pool->occupied[victim] = false;
    return victim;
}

Page *pool_get(BufferPool *pool, uint32_t page_id) {
    /* check cache first */
    for (int i = 0; i < POOL_SIZE; i++) {
        if (pool->occupied[i] && pool->page_ids[i] == page_id) {
            pool->access_tick[i] = pool->tick++;
            return &pool->pages[i];
        }
    }

    /* not cached — load from disk */
    int slot = pool_slot(pool, page_id);
    if (disk_read_page(pool->fp, page_id, &pool->pages[slot]) != LRM_OK) {
        return NULL;
    }

    pool->page_ids[slot] = page_id;
    pool->occupied[slot] = true;
    pool->dirty[slot] = false;
    pool->access_tick[slot] = pool->tick++;
    return &pool->pages[slot];
}

int pool_mark_dirty(BufferPool *pool, uint32_t page_id) {
    for (int i = 0; i < POOL_SIZE; i++) {
        if (pool->occupied[i] && pool->page_ids[i] == page_id) {
            pool->dirty[i] = true;
            return LRM_OK;
        }
    }
    return LRM_ERR_NOTFOUND;
}

int pool_flush(BufferPool *pool) {
    for (int i = 0; i < POOL_SIZE; i++) {
        if (pool->occupied[i] && pool->dirty[i]) {
            int rc = disk_write_page(pool->fp, pool->page_ids[i],
                                     &pool->pages[i]);
            if (rc != LRM_OK) return rc;
            pool->dirty[i] = false;
        }
    }
    return LRM_OK;
}

uint32_t pool_alloc_page(BufferPool *pool) {
    /* Check free list first (stored in page 0 of database header) */
    /* For now, just append new page */
    uint32_t new_id = pool->total_pages;
    pool->total_pages++;

    /* Get the page into the buffer pool and initialize it */
    int slot = pool_slot(pool, new_id);
    page_init(&pool->pages[slot], PAGE_FREE);
    pool->page_ids[slot] = new_id;
    pool->occupied[slot] = true;
    pool->dirty[slot] = true;
    pool->access_tick[slot] = pool->tick++;

    return new_id;
}

int pool_free_page(BufferPool *pool, uint32_t page_id) {
    Page *p = pool_get(pool, page_id);
    if (!p) return LRM_ERR_IO;
    page_init(p, PAGE_FREE);
    pool_mark_dirty(pool, page_id);
    return LRM_OK;
}

/* ── Utility ────────────────────────────────────────────── */

uint32_t lrm_checksum(const void *data, size_t len) {
    /* FNV-1a 32-bit */
    uint32_t hash = 2166136261u;
    const uint8_t *p = (const uint8_t *)data;
    for (size_t i = 0; i < len; i++) {
        hash ^= p[i];
        hash *= 16777619u;
    }
    return hash;
}

void lrm_timestamp(char *buf, size_t len) {
    time_t now = time(NULL);
    struct tm *t = gmtime(&now);
    snprintf(buf, len, "%04d-%02d-%02dT%02d:%02d:%02dZ",
             t->tm_year + 1900, t->tm_mon + 1, t->tm_mday,
             t->tm_hour, t->tm_min, t->tm_sec);
}

int64_t lrm_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    /* FILETIME is 100ns intervals since 1601-01-01.
       Convert to ms since Unix epoch (1970-01-01). */
    uint64_t t = ((uint64_t)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    t -= 116444736000000000ULL; /* 1601→1970 offset */
    return (int64_t)(t / 10000);
#else
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (int64_t)ts.tv_sec * 1000 + (int64_t)ts.tv_nsec / 1000000;
#endif
}
