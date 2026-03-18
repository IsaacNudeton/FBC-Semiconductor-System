/*
 * table.c — Record storage and retrieval with constraints
 *
 * Records are fixed-size (determined by table schema) stored in
 * PAGE_DATA pages. Each page holds floor((4096-16) / record_size) records.
 * Slots within a page are tracked by a bitmap at the start of the data area.
 *
 * Page data layout:
 *   [PageHeader: 16 bytes]
 *   [Slot bitmap: ceil(slots_per_page / 8) bytes]
 *   [Record 0]
 *   [Record 1]
 *   ...
 *
 * Indexes (B-trees) map key → RecordPtr (page_id, slot).
 * Primary key index is always index[0].
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#ifdef _WIN32
#include <io.h>
#include <sys/locking.h>
#else
#include <sys/file.h>
#include <unistd.h>
#endif

/* ── Lock File ──────────────────────────────────────────── */

/* Acquire exclusive lock on DB — prevents second process from opening same file.
 * Creates <path>.lock, opens it, and holds an OS-level advisory lock.
 * Returns 0 on success, -1 if another process holds the lock. */
static int db_lock_acquire(Database *db, const char *path) {
    snprintf(db->lock_path, sizeof(db->lock_path), "%s.lock", path);
    db->lock_fp = fopen(db->lock_path, "w+b");
    if (!db->lock_fp) return LRM_ERR_IO;
    /* Write a byte so lock region is valid */
    fputc('L', db->lock_fp);
    fflush(db->lock_fp);
    fseek(db->lock_fp, 0, SEEK_SET);

#ifdef _WIN32
    int fd = _fileno(db->lock_fp);
    if (_locking(fd, _LK_NBLCK, 1) != 0) {
        fclose(db->lock_fp);
        db->lock_fp = NULL;
        return LRM_ERR_IO; /* another process holds the lock */
    }
#else
    int fd = fileno(db->lock_fp);
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) {
        fclose(db->lock_fp);
        db->lock_fp = NULL;
        return LRM_ERR_IO; /* another process holds the lock */
    }
#endif
    return LRM_OK;
}

static void db_lock_release(Database *db) {
    if (db->lock_fp) {
#ifdef _WIN32
        int fd = _fileno(db->lock_fp);
        _locking(fd, _LK_UNLCK, 1);
#else
        int fd = fileno(db->lock_fp);
        flock(fd, LOCK_UN);
#endif
        fclose(db->lock_fp);
        db->lock_fp = NULL;
        remove(db->lock_path);
    }
}

/* ── Internal helpers ───────────────────────────────────── */

/* Forward declare btree key encoding */
extern void btree_encode_i64(int64_t val, uint8_t *buf);
extern int64_t btree_decode_i64(const uint8_t *buf);

/* Forward declaration */
static uint32_t build_index_key(const TableDef *t, const IndexDef *idx,
                                const void *record, uint8_t *key_buf);

TableDef *find_table(Database *db, const char *name) {
    for (uint32_t i = 0; i < db->num_tables; i++) {
        if (strcmp(db->tables[i].name, name) == 0) {
            return &db->tables[i];
        }
    }
    return NULL;
}

static uint32_t find_table_index(Database *db, const TableDef *t) {
    for (uint32_t i = 0; i < db->num_tables; i++) {
        if (&db->tables[i] == t) return i;
    }
    return 0;
}

/* How many records fit per data page */
static uint32_t slots_per_page(uint32_t record_size) {
    /* bitmap at start: ceil(n/8) bytes. We solve:
     * PAGE_HEADER_SZ + ceil(n/8) + n * record_size <= PAGE_SIZE
     * Approximate: n = (PAGE_SIZE - PAGE_HEADER_SZ) / (record_size + 0.125) */
    uint32_t avail = PAGE_SIZE - PAGE_HEADER_SZ;
    uint32_t n = avail / (record_size + 1);  /* conservative */
    if (n == 0) n = 1;
    while (n > 0) {
        uint32_t bitmap_bytes = (n + 7) / 8;
        if (PAGE_HEADER_SZ + bitmap_bytes + n * record_size <= PAGE_SIZE)
            return n;
        n--;
    }
    return 1;
}

static uint32_t bitmap_offset(void) {
    return PAGE_HEADER_SZ;
}

static uint32_t record_offset(uint32_t record_size, uint32_t slots, uint32_t slot) {
    uint32_t bitmap_bytes = (slots + 7) / 8;
    return PAGE_HEADER_SZ + bitmap_bytes + slot * record_size;
}

static bool slot_is_used(const Page *p, uint32_t slot) {
    uint32_t byte_idx = bitmap_offset() + slot / 8;
    uint8_t bit = 1 << (slot % 8);
    return (p->data[byte_idx] & bit) != 0;
}

static void slot_set(Page *p, uint32_t slot, bool used) {
    uint32_t byte_idx = bitmap_offset() + slot / 8;
    uint8_t bit = 1 << (slot % 8);
    if (used)
        p->data[byte_idx] |= bit;
    else
        p->data[byte_idx] &= ~bit;
}

/* Extract primary key value from a record (assumes first int64 column is PK) */
static int64_t get_pk(const TableDef *t, const void *record) {
    for (uint32_t i = 0; i < t->num_cols; i++) {
        if (t->cols[i].is_primary) {
            int64_t val;
            memcpy(&val, (const uint8_t *)record + t->cols[i].offset,
                   sizeof(int64_t));
            return val;
        }
    }
    return 0;
}

static void set_pk(const TableDef *t, void *record, int64_t val) {
    for (uint32_t i = 0; i < t->num_cols; i++) {
        if (t->cols[i].is_primary) {
            memcpy((uint8_t *)record + t->cols[i].offset, &val,
                   sizeof(int64_t));
            return;
        }
    }
}

/* Get column value as bytes for indexing */
static void get_col_bytes(const TableDef *t, const void *record,
                          uint32_t col_idx, void *buf, uint32_t buf_size) {
    const ColDef *col = &t->cols[col_idx];
    const uint8_t *src = (const uint8_t *)record + col->offset;

    if (col->type == COL_INT64 || col->type == COL_INT32) {
        int64_t val = 0;
        if (col->type == COL_INT64)
            memcpy(&val, src, 8);
        else {
            int32_t v32;
            memcpy(&v32, src, 4);
            val = v32;
        }
        btree_encode_i64(val, buf);
    } else {
        /* text — copy as-is, zero-padded */
        memset(buf, 0, buf_size);
        uint32_t copy_len = col->size < buf_size ? col->size : buf_size;
        memcpy(buf, src, copy_len);
    }
}

/* ── Constraint Checking ────────────────────────────────── */

static int check_not_null(const TableDef *t, const void *record) {
    for (uint32_t i = 0; i < t->num_cols; i++) {
        if (!t->cols[i].not_null) continue;
        if (t->cols[i].is_primary && t->cols[i].auto_inc) continue;

        const uint8_t *p = (const uint8_t *)record + t->cols[i].offset;
        if (t->cols[i].type == COL_TEXT) {
            /* null = empty string */
            if (p[0] == '\0') return LRM_ERR_NULL;
        }
        /* int/bool: 0 is valid, can't really be "null" in C struct */
    }
    return LRM_OK;
}

static int check_constraints(const TableDef *t, const void *record) {
    for (uint32_t i = 0; i < t->num_checks; i++) {
        if (!t->checks[i](record, t->record_size)) {
            return LRM_ERR_CHECK;
        }
    }
    return LRM_OK;
}

static int check_unique(Database *db, TableDef *t, const void *record,
                        int64_t exclude_pk) {
    /* For each unique index, check if the key already exists */
    for (uint32_t i = 0; i < t->num_indexes; i++) {
        IndexDef *idx = &t->indexes[i];
        if (idx->type != IDX_UNIQUE && idx->type != IDX_PRIMARY) continue;

        uint8_t key_buf[1024];
        memset(key_buf, 0, 1024);

        /* Build composite key */
        uint32_t off = 0;
        for (uint32_t c = 0; c < idx->num_cols; c++) {
            uint32_t col_idx = idx->col_indices[c];
            uint32_t col_size = t->cols[col_idx].size;
            if (t->cols[col_idx].type == COL_INT64 ||
                t->cols[col_idx].type == COL_INT32) {
                col_size = 8; /* encoded as i64 */
            }
            get_col_bytes(t, record, col_idx, key_buf + off, col_size);
            off += col_size;
        }

        RecordPtr rp;
        int rc = btree_find(&db->pool, &idx->btree, key_buf, &rp);
        if (rc == LRM_OK) {
            /* found — check if it's the same record (for updates) */
            if (exclude_pk != 0) {
                Page *p = pool_get(&db->pool, rp.page_id);
                if (p) {
                    uint32_t slots = slots_per_page(t->record_size);
                    uint32_t roff = record_offset(t->record_size, slots, rp.slot);
                    int64_t existing_pk = 0;
                    memcpy(&existing_pk, p->data + roff + t->cols[0].offset,
                           sizeof(int64_t));
                    if (existing_pk == exclude_pk) continue;
                }
            }
            return LRM_ERR_EXISTS;
        }
    }
    return LRM_OK;
}

/* ── Table Catalog (page 0) ─────────────────────────────── */

/*
 * Page 0 layout:
 *   [0..3]   magic
 *   [4..7]   version
 *   [8..9]   num_tables (uint16_t)
 *   [10..15] reserved
 *   [16..]   catalog entries (CATALOG_ENTRY_SZ bytes each)
 *
 * Each catalog entry:
 *   [0..63]  table name (64 bytes, null-padded)
 *   [64..67] first_page
 *   [68..71] row_count
 *   [72..79] auto_inc_next (int64_t)
 *   [80..83] num_indexes
 *   [84..115] index root_page[8] (uint32_t × 8)
 *   [116..147] index key_size[8] (uint32_t × 8)
 *   Total: 148 bytes per entry.  21 tables × 148 = 3108 → fits in 4080 avail.
 */

#define CATALOG_OFFSET      16
#define CATALOG_ENTRY_SZ   148
#define CATALOG_NAME_SZ     64

/* Find a table in the on-disk catalog. Returns entry offset or 0 if not found. */
static uint32_t catalog_find(BufferPool *pool, const char *name) {
    Page *hdr = pool_get(pool, 0);
    if (!hdr) return 0;

    uint32_t num_tables;
    memcpy(&num_tables, hdr->data + 8, 4);

    for (uint32_t i = 0; i < num_tables; i++) {
        uint32_t off = CATALOG_OFFSET + i * CATALOG_ENTRY_SZ;
        if (strncmp((const char *)(hdr->data + off), name, CATALOG_NAME_SZ) == 0) {
            return off;
        }
    }
    return 0;
}

/* Write table metadata to the catalog on page 0. */
static int catalog_save(Database *db, const TableDef *def, uint32_t table_idx) {
    Page *hdr = pool_get(&db->pool, 0);
    if (!hdr) return LRM_ERR_IO;

    uint32_t off = CATALOG_OFFSET + table_idx * CATALOG_ENTRY_SZ;

    /* name */
    memset(hdr->data + off, 0, CATALOG_NAME_SZ);
    strncpy((char *)(hdr->data + off), def->name, CATALOG_NAME_SZ - 1);

    /* first_page, row_count, auto_inc_next, num_indexes */
    memcpy(hdr->data + off + 64, &def->first_page, 4);
    memcpy(hdr->data + off + 68, &def->row_count, 4);
    memcpy(hdr->data + off + 72, &def->auto_inc_next, 8);
    memcpy(hdr->data + off + 80, &def->num_indexes, 4);

    /* index root pages + key sizes */
    for (uint32_t i = 0; i < MAX_INDEXES; i++) {
        uint32_t root = (i < def->num_indexes) ? def->indexes[i].btree.root_page : 0;
        uint32_t ksz  = (i < def->num_indexes) ? def->indexes[i].btree.key_size : 0;
        memcpy(hdr->data + off + 84 + i * 4, &root, 4);
        memcpy(hdr->data + off + 116 + i * 4, &ksz, 4);
    }

    /* update num_tables in header (uint32_t at offset 8) */
    uint32_t num = table_idx + 1;
    uint32_t cur;
    memcpy(&cur, hdr->data + 8, 4);
    if (num > cur)
        memcpy(hdr->data + 8, &num, 4);

    pool_mark_dirty(&db->pool, 0);
    return LRM_OK;
}

/* ── Table Registration ─────────────────────────────────── */

/* Compute key_size for an index based on column definitions */
static uint32_t compute_key_size(const TableDef *def, const IndexDef *ix) {
    uint32_t key_size = 0;
    for (uint32_t c = 0; c < ix->num_cols; c++) {
        uint32_t col_idx = ix->col_indices[c];
        if (def->cols[col_idx].type == COL_INT64 ||
            def->cols[col_idx].type == COL_INT32) {
            key_size += 8;
        } else {
            key_size += def->cols[col_idx].size;
        }
    }
    if (ix->type == IDX_NORMAL) {
        key_size += 8; /* append PK for uniqueness */
    }
    return key_size;
}

int table_register(Database *db, TableDef *def) {
    if (db->num_tables >= MAX_TABLES) return LRM_ERR_SCHEMA;

    /* compute record_size from columns */
    uint32_t total = 0;
    for (uint32_t i = 0; i < def->num_cols; i++) {
        def->cols[i].offset = total;
        total += def->cols[i].size;
    }
    def->record_size = total;

    /* Check if table already exists in on-disk catalog (reopen path) */
    uint32_t cat_off = catalog_find(&db->pool, def->name);
    if (cat_off != 0) {
        /* ── RELOAD existing table metadata ─── */
        Page *hdr = pool_get(&db->pool, 0);
        if (!hdr) return LRM_ERR_IO;

        memcpy(&def->first_page,    hdr->data + cat_off + 64, 4);
        memcpy(&def->row_count,     hdr->data + cat_off + 68, 4);
        memcpy(&def->auto_inc_next, hdr->data + cat_off + 72, 8);

        uint32_t stored_num_idx;
        memcpy(&stored_num_idx, hdr->data + cat_off + 80, 4);

        /* Disk-authoritative: if schema changed, refuse to load stale data */
        if (stored_num_idx != def->num_indexes) {
            fprintf(stderr, "[catalog] schema mismatch for '%s': "
                    "disk has %u indexes, code expects %u\n",
                    def->name, stored_num_idx, def->num_indexes);
            return LRM_ERR_CORRUPT;
        }

        for (uint32_t i = 0; i < stored_num_idx; i++) {
            uint32_t root, ksz;
            memcpy(&root, hdr->data + cat_off + 84 + i * 4, 4);
            memcpy(&ksz,  hdr->data + cat_off + 116 + i * 4, 4);
            def->indexes[i].btree.root_page = root;
            def->indexes[i].btree.key_size = ksz;
            def->indexes[i].btree.val_size = sizeof(RecordPtr);
        }

        memcpy(&db->tables[db->num_tables], def, sizeof(TableDef));
        db->num_tables++;
        return LRM_OK;
    }

    /* ── CREATE new table ─── */

    /* allocate first data page */
    def->first_page = pool_alloc_page(&db->pool);
    Page *p = pool_get(&db->pool, def->first_page);
    if (!p) return LRM_ERR_IO;
    memset(p->data, 0, PAGE_SIZE);
    PageHeader ph = {0};
    ph.page_type = PAGE_DATA;
    ph.free_space = PAGE_SIZE - PAGE_HEADER_SZ;
    memcpy(p->data, &ph, sizeof(PageHeader));
    pool_mark_dirty(&db->pool, def->first_page);

    /* create B-tree indexes */
    for (uint32_t i = 0; i < def->num_indexes; i++) {
        IndexDef *idx = &def->indexes[i];
        uint32_t key_size = compute_key_size(def, idx);
        int rc = btree_create(&db->pool, &idx->btree,
                              key_size, sizeof(RecordPtr));
        if (rc != LRM_OK) return rc;
    }

    def->row_count = 0;
    def->auto_inc_next = 1;

    uint32_t table_idx = db->num_tables;
    memcpy(&db->tables[table_idx], def, sizeof(TableDef));
    db->num_tables++;

    /* Save to on-disk catalog */
    return catalog_save(db, def, table_idx);
}

/* ── Find a free slot in data pages ─────────────────────── */

static int find_free_slot(Database *db, TableDef *t,
                          uint32_t *out_page_id, uint32_t *out_slot) {
    uint32_t slots = slots_per_page(t->record_size);
    uint32_t page_id = t->first_page;

    /* scan data pages for a free slot */
    for (;;) {
        Page *p = pool_get(&db->pool, page_id);
        if (!p) return LRM_ERR_IO;

        for (uint32_t s = 0; s < slots; s++) {
            if (!slot_is_used(p, s)) {
                *out_page_id = page_id;
                *out_slot = s;
                return LRM_OK;
            }
        }

        /* check next_page in header */
        PageHeader ph;
        memcpy(&ph.next_page, &p->data[8], 4);
        if (ph.next_page != 0) {
            page_id = ph.next_page;
        } else {
            /* allocate new data page and link it */
            uint32_t new_id = pool_alloc_page(&db->pool);
            Page *np = pool_get(&db->pool, new_id);
            if (!np) return LRM_ERR_IO;
            memset(np->data, 0, PAGE_SIZE);
            uint16_t ptype = PAGE_DATA;
            memcpy(np->data, &ptype, 2);
            pool_mark_dirty(&db->pool, new_id);

            /* link from current page */
            p = pool_get(&db->pool, page_id); /* re-get after possible eviction */
            if (!p) return LRM_ERR_IO;
            memcpy(&p->data[8], &new_id, 4);
            pool_mark_dirty(&db->pool, page_id);

            *out_page_id = new_id;
            *out_slot = 0;
            return LRM_OK;
        }
    }
}

/* ── Insert ─────────────────────────────────────────────── */

int table_insert(Database *db, const char *table_name, const void *record) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    /* make a mutable copy for auto-inc */
    uint8_t rec[4096];
    memcpy(rec, record, t->record_size);

    /* auto-increment PK */
    for (uint32_t i = 0; i < t->num_cols; i++) {
        if (t->cols[i].is_primary && t->cols[i].auto_inc) {
            int64_t pk = get_pk(t, rec);
            if (pk == 0) {
                set_pk(t, rec, t->auto_inc_next);
                t->auto_inc_next++;
            } else if (pk >= t->auto_inc_next) {
                t->auto_inc_next = pk + 1;
            }
        }
    }

    /* check NOT NULL */
    int rc = check_not_null(t, rec);
    if (rc != LRM_OK) return rc;

    /* check CHECK constraints */
    rc = check_constraints(t, rec);
    if (rc != LRM_OK) return rc;

    /* check UNIQUE constraints */
    rc = check_unique(db, t, rec, 0);
    if (rc != LRM_OK) return rc;

    /* find free slot */
    uint32_t page_id, slot;
    rc = find_free_slot(db, t, &page_id, &slot);
    if (rc != LRM_OK) return rc;

    /* write record */
    uint32_t slots = slots_per_page(t->record_size);
    Page *p = pool_get(&db->pool, page_id);
    if (!p) return LRM_ERR_IO;

    uint32_t roff = record_offset(t->record_size, slots, slot);
    memcpy(p->data + roff, rec, t->record_size);
    slot_set(p, slot, true);
    pool_mark_dirty(&db->pool, page_id);

    /* update all indexes */
    RecordPtr rp = { .page_id = page_id, .slot = (uint16_t)slot };

    for (uint32_t i = 0; i < t->num_indexes; i++) {
        IndexDef *idx = &t->indexes[i];
        uint8_t key_buf[1024];
        build_index_key(t, idx, rec, key_buf);
        rc = btree_insert(&db->pool, &idx->btree, key_buf, &rp);
        if (rc != LRM_OK) return rc;
    }

    t->row_count++;

    /* copy PK back to caller's record */
    int64_t assigned_pk = get_pk(t, rec);
    memcpy((uint8_t *)record + t->cols[0].offset, &assigned_pk, sizeof(int64_t));

    /* persist catalog (row_count, auto_inc changed) */
    catalog_save(db, t, find_table_index(db, t));

    return LRM_OK;
}

/* ── Find by PK ─────────────────────────────────────────── */

int table_find_by_pk(Database *db, const char *table_name,
                     int64_t pk, void *record_out) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    /* primary key index is index[0] */
    uint8_t key_buf[8];
    btree_encode_i64(pk, key_buf);

    RecordPtr rp;
    int rc = btree_find(&db->pool, &t->indexes[0].btree, key_buf, &rp);
    if (rc != LRM_OK) return rc;

    Page *p = pool_get(&db->pool, rp.page_id);
    if (!p) return LRM_ERR_IO;

    uint32_t slots = slots_per_page(t->record_size);
    uint32_t roff = record_offset(t->record_size, slots, rp.slot);
    memcpy(record_out, p->data + roff, t->record_size);
    return LRM_OK;
}

/* ── Find by secondary index ────────────────────────────── */

int table_find_by_index(Database *db, const char *table_name,
                        const char *index_name,
                        const void *key, void *results,
                        uint32_t *count, uint32_t max_results) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    IndexDef *idx = NULL;
    for (uint32_t i = 0; i < t->num_indexes; i++) {
        if (strcmp(t->indexes[i].name, index_name) == 0) {
            idx = &t->indexes[i];
            break;
        }
    }
    if (!idx) return LRM_ERR_NOTFOUND;

    RecordPtr ptrs[256];
    uint32_t found = 0;
    int rc;

    if (idx->type == IDX_NORMAL) {
        /* Non-unique index: key in B-tree is (column_values + PK).
         * The search key is just the column values — use prefix scan. */
        uint32_t prefix_len = idx->btree.key_size - 8;  /* subtract PK bytes */
        rc = btree_prefix_scan(&db->pool, &idx->btree, key,
                               prefix_len, ptrs, &found, 256);
    } else {
        /* Unique/primary index: exact match */
        rc = btree_find_all(&db->pool, &idx->btree, key,
                            ptrs, &found, 256);
    }
    if (rc != LRM_OK) return rc;

    *count = 0;
    uint32_t slots = slots_per_page(t->record_size);

    for (uint32_t i = 0; i < found && *count < max_results; i++) {
        Page *p = pool_get(&db->pool, ptrs[i].page_id);
        if (!p) continue;
        uint32_t roff = record_offset(t->record_size, slots, ptrs[i].slot);
        memcpy((uint8_t *)results + (*count) * t->record_size,
               p->data + roff, t->record_size);
        (*count)++;
    }

    return LRM_OK;
}

/* Build the B-tree key for an index entry from a record.
 * For NORMAL indexes, appends the PK to make the key unique. */
static uint32_t build_index_key(const TableDef *t, const IndexDef *idx,
                                const void *record, uint8_t *key_buf) {
    memset(key_buf, 0, 1024);
    uint32_t off = 0;
    for (uint32_t c = 0; c < idx->num_cols; c++) {
        uint32_t col_idx = idx->col_indices[c];
        uint32_t col_size = (t->cols[col_idx].type == COL_INT64 ||
                             t->cols[col_idx].type == COL_INT32) ? 8 :
                             t->cols[col_idx].size;
        get_col_bytes(t, record, col_idx, key_buf + off, col_size);
        off += col_size;
    }
    if (idx->type == IDX_NORMAL) {
        int64_t pk = get_pk(t, record);
        btree_encode_i64(pk, key_buf + off);
        off += 8;
    }
    return off;
}

/* ── Update ─────────────────────────────────────────────── */

int table_update(Database *db, const char *table_name,
                 int64_t pk, const void *record) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    /* find existing record */
    uint8_t key_buf[8];
    btree_encode_i64(pk, key_buf);

    RecordPtr rp;
    int rc = btree_find(&db->pool, &t->indexes[0].btree, key_buf, &rp);
    if (rc != LRM_OK) return rc;

    /* validate new record */
    rc = check_not_null(t, record);
    if (rc != LRM_OK) return rc;
    rc = check_constraints(t, record);
    if (rc != LRM_OK) return rc;
    rc = check_unique(db, t, record, pk);
    if (rc != LRM_OK) return rc;

    /* read old record for index update */
    uint32_t slots = slots_per_page(t->record_size);
    Page *p = pool_get(&db->pool, rp.page_id);
    if (!p) return LRM_ERR_IO;
    uint32_t roff = record_offset(t->record_size, slots, rp.slot);

    uint8_t old_rec[4096];
    memcpy(old_rec, p->data + roff, t->record_size);

    /* update secondary indexes: remove old keys, insert new */
    for (uint32_t i = 1; i < t->num_indexes; i++) {  /* skip PK index */
        IndexDef *idx = &t->indexes[i];
        uint8_t old_key[1024], new_key[1024];

        build_index_key(t, idx, old_rec, old_key);
        build_index_key(t, idx, record, new_key);

        uint32_t key_sz = idx->btree.key_size;
        if (memcmp(old_key, new_key, key_sz) != 0) {
            btree_delete(&db->pool, &idx->btree, old_key);
            RecordPtr rp_upd = { .page_id = rp.page_id, .slot = rp.slot };
            btree_insert(&db->pool, &idx->btree, new_key, &rp_upd);
        }
    }

    /* write new record data */
    p = pool_get(&db->pool, rp.page_id);  /* re-get */
    if (!p) return LRM_ERR_IO;
    memcpy(p->data + roff, record, t->record_size);
    pool_mark_dirty(&db->pool, rp.page_id);

    return LRM_OK;
}

/* ── Delete ─────────────────────────────────────────────── */

int table_delete(Database *db, const char *table_name, int64_t pk) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    uint8_t key_buf[8];
    btree_encode_i64(pk, key_buf);

    RecordPtr rp;
    int rc = btree_find(&db->pool, &t->indexes[0].btree, key_buf, &rp);
    if (rc != LRM_OK) return rc;

    /* read record for index cleanup */
    uint32_t slots = slots_per_page(t->record_size);
    Page *p = pool_get(&db->pool, rp.page_id);
    if (!p) return LRM_ERR_IO;
    uint32_t roff = record_offset(t->record_size, slots, rp.slot);

    uint8_t rec[4096];
    memcpy(rec, p->data + roff, t->record_size);

    /* remove from all indexes */
    for (uint32_t i = 0; i < t->num_indexes; i++) {
        IndexDef *idx = &t->indexes[i];
        uint8_t idx_key[1024];
        build_index_key(t, idx, rec, idx_key);
        btree_delete(&db->pool, &idx->btree, idx_key);
    }

    /* clear slot */
    p = pool_get(&db->pool, rp.page_id);
    if (!p) return LRM_ERR_IO;
    slot_set(p, rp.slot, false);
    memset(p->data + roff, 0, t->record_size);
    pool_mark_dirty(&db->pool, rp.page_id);

    t->row_count--;

    /* persist catalog (row_count changed) */
    catalog_save(db, t, find_table_index(db, t));

    return LRM_OK;
}

/* ── Full table scan with filter ────────────────────────── */

int table_scan(Database *db, const char *table_name,
               bool (*filter)(const void *record, void *ctx),
               void *ctx, void *results,
               uint32_t *count, uint32_t max_results) {
    TableDef *t = find_table(db, table_name);
    if (!t) return LRM_ERR_NOTFOUND;

    *count = 0;
    uint32_t slots = slots_per_page(t->record_size);
    uint32_t page_id = t->first_page;

    while (page_id != 0 && *count < max_results) {
        Page *p = pool_get(&db->pool, page_id);
        if (!p) return LRM_ERR_IO;

        for (uint32_t s = 0; s < slots && *count < max_results; s++) {
            if (!slot_is_used(p, s)) continue;

            uint32_t roff = record_offset(t->record_size, slots, s);
            const void *rec = p->data + roff;

            if (filter == NULL || filter(rec, ctx)) {
                memcpy((uint8_t *)results + (*count) * t->record_size,
                       rec, t->record_size);
                (*count)++;
            }
        }

        /* follow page chain */
        memcpy(&page_id, &p->data[8], 4);
    }

    return LRM_OK;
}

/* ── Database Open/Close/Create ─────────────────────────── */

/* Internal: create DB with lock already held (lock_fp/lock_path set in db) */
static int db_create_locked(Database *db, const char *path) {
    /* Caller guarantees db->lock_fp and db->lock_path are valid */
    FILE *held_lock = db->lock_fp;
    char held_lock_path[520];
    memcpy(held_lock_path, db->lock_path, sizeof(held_lock_path));

    memset(db, 0, sizeof(Database));
    strncpy(db->path, path, sizeof(db->path) - 1);
    db->lock_fp = held_lock;
    memcpy(db->lock_path, held_lock_path, sizeof(db->lock_path));

    db->magic = DB_MAGIC;
    db->version = DB_VERSION;

    FILE *fp = fopen(path, "w+b");
    if (!fp) { db_lock_release(db); return LRM_ERR_IO; }

    pool_init(&db->pool, fp);

    uint32_t hdr_id = pool_alloc_page(&db->pool);
    (void)hdr_id;
    Page *hdr = pool_get(&db->pool, 0);
    if (!hdr) { fclose(fp); db_lock_release(db); return LRM_ERR_IO; }
    memset(hdr->data, 0, PAGE_SIZE);
    memcpy(hdr->data, &db->magic, 4);
    memcpy(hdr->data + 4, &db->version, 4);
    pool_mark_dirty(&db->pool, 0);

    int rc = wal_open(&db->wal, path);
    if (rc != LRM_OK) { fclose(fp); db_lock_release(db); return rc; }

    db->open = true;
    return LRM_OK;
}

int db_create(Database *db, const char *path) {
    memset(db, 0, sizeof(Database));
    strncpy(db->path, path, sizeof(db->path) - 1);

    int lrc = db_lock_acquire(db, path);
    if (lrc != LRM_OK) return lrc;

    /* lock_fp now set — delegate to locked path */
    return db_create_locked(db, path);
}

int db_open(Database *db, const char *path) {
    memset(db, 0, sizeof(Database));
    strncpy(db->path, path, sizeof(db->path) - 1);

    /* Acquire exclusive lock — fails if another process holds it */
    int lrc = db_lock_acquire(db, path);
    if (lrc != LRM_OK) return lrc;

    FILE *fp = fopen(path, "r+b");
    if (!fp) {
        /* doesn't exist — create (lock already held) */
        return db_create_locked(db, path);
    }

    pool_init(&db->pool, fp);

    /* read header */
    Page *hdr = pool_get(&db->pool, 0);
    if (!hdr) { fclose(fp); db_lock_release(db); return LRM_ERR_IO; }
    memcpy(&db->magic, hdr->data, 4);
    memcpy(&db->version, hdr->data + 4, 4);

    if (db->magic != DB_MAGIC) {
        /* Corrupt or empty file — recreate (lock already held) */
        fclose(fp);
        remove(path);
        char wal_path[512];
        snprintf(wal_path, sizeof(wal_path), "%s.wal", path);
        remove(wal_path);
        return db_create_locked(db, path);
    }

    /* open WAL and recover */
    int rc = wal_open(&db->wal, path);
    if (rc != LRM_OK) { fclose(fp); db_lock_release(db); return rc; }
    wal_recover(&db->wal, &db->pool);

    db->open = true;
    return LRM_OK;
}

int db_close(Database *db) {
    if (!db->open) return LRM_OK;

    pool_flush(&db->pool);
    wal_checkpoint(&db->wal);
    wal_close(&db->wal);

    if (db->pool.fp) {
        fclose(db->pool.fp);
        db->pool.fp = NULL;
    }

    db_lock_release(db);

    db->open = false;
    return LRM_OK;
}
