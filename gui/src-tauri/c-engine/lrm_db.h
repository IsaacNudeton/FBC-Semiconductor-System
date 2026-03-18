/*
 * lrm_db.h — Lab Resource Manager Database Engine
 *
 * Custom page-based storage engine for burn-in inventory.
 * Single-file database, B-tree indexed, WAL-backed.
 *
 * Architecture (bottom up):
 *   Page layer   — 4KB pages, buffer pool, disk I/O
 *   B-tree layer — ordered index on any column
 *   WAL layer    — write-ahead log for crash recovery
 *   Table layer  — record storage, schema enforcement
 */

#ifndef LRM_DB_H
#define LRM_DB_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdio.h>

/* ── Page Format ────────────────────────────────────────── */

#define PAGE_SIZE       4096
#define MAX_PAGES       65536
#define PAGE_HEADER_SZ  16

/*
 * Page layout:
 *   [0..1]   page_type   (LEAF, INTERNAL, OVERFLOW, FREE)
 *   [2..3]   num_cells   (number of records/keys in this page)
 *   [4..7]   right_ptr   (rightmost child for internal B-tree nodes)
 *   [8..11]  next_page   (linked list: overflow chain or free list)
 *   [12..15] free_space   (bytes of free space remaining)
 *   [16..]   cell data
 *
 * Cells grow from the front, cell content grows from the back.
 * Cell pointer array: [offset, size] pairs starting at byte 16.
 */

typedef enum {
    PAGE_FREE     = 0,
    PAGE_LEAF     = 1,
    PAGE_INTERNAL = 2,
    PAGE_OVERFLOW  = 3,
    PAGE_DATA     = 4   /* raw table data page (non-indexed) */
} PageType;

typedef struct {
    uint16_t page_type;
    uint16_t num_cells;
    uint32_t right_ptr;     /* page_id of rightmost child (internal nodes) */
    uint32_t next_page;     /* overflow chain or free list next */
    uint32_t free_space;
} PageHeader;

typedef struct {
    uint8_t data[PAGE_SIZE];
} Page;

/* ── Buffer Pool ────────────────────────────────────────── */

#define POOL_SIZE 256   /* pages cached in memory */

typedef struct {
    Page     pages[POOL_SIZE];
    uint32_t page_ids[POOL_SIZE];
    bool     dirty[POOL_SIZE];
    bool     occupied[POOL_SIZE];
    uint32_t access_tick[POOL_SIZE]; /* LRU eviction */
    uint32_t tick;
    FILE    *fp;
    uint32_t total_pages;  /* pages on disk */
} BufferPool;

/* ── B-Tree ─────────────────────────────────────────────── */

#define BTREE_ORDER 64  /* max keys per node */

typedef struct {
    uint32_t root_page;
    uint32_t key_size;      /* bytes per key (fixed) */
    uint32_t val_size;      /* bytes per value (page_id + slot for record pointer) */
} BTreeMeta;

typedef struct {
    uint32_t page_id;
    uint16_t slot;
} RecordPtr;

/* ── WAL (Write-Ahead Log) ──────────────────────────────── */

#define WAL_MAGIC 0x4C524D57  /* "LRMW" */

typedef struct {
    uint32_t magic;
    uint32_t txn_id;
    uint32_t page_id;
    uint32_t checksum;
    uint8_t  before_image[PAGE_SIZE];
} WalEntry;

typedef struct {
    FILE    *fp;
    uint32_t current_txn;
    uint32_t entry_count;
    char     path[512];
} Wal;

/* ── Transaction ────────────────────────────────────────── */

typedef struct {
    uint32_t txn_id;
    bool     active;
    Wal     *wal;
    BufferPool *pool;
} Txn;

/* ── Column / Schema Definition ─────────────────────────── */

typedef enum {
    COL_INT32   = 1,
    COL_INT64   = 2,
    COL_TEXT     = 3,  /* fixed-size char array */
    COL_BOOL    = 4,
} ColType;

#define MAX_COLS       32
#define MAX_INDEXES    8
#define MAX_TABLE_NAME 64
#define MAX_COL_NAME   64
#define MAX_TEXT_LEN   256

typedef struct {
    char     name[MAX_COL_NAME];
    ColType  type;
    uint32_t size;          /* byte width (for TEXT: max chars including null) */
    uint32_t offset;        /* byte offset within record */
    bool     not_null;
    bool     is_primary;
    bool     auto_inc;
} ColDef;

typedef enum {
    IDX_PRIMARY = 0,
    IDX_UNIQUE  = 1,
    IDX_NORMAL  = 2,
} IndexType;

typedef struct {
    char       name[MAX_COL_NAME];
    uint32_t   col_indices[4];  /* up to 4-column composite index */
    uint32_t   num_cols;
    IndexType  type;
    BTreeMeta  btree;
} IndexDef;

/* Foreign key action */
typedef enum {
    FK_NO_ACTION  = 0,
    FK_CASCADE    = 1,
    FK_SET_NULL   = 2,
    FK_RESTRICT   = 3,
} FkAction;

typedef struct {
    uint32_t col_index;         /* column in this table */
    char     ref_table[MAX_TABLE_NAME];
    uint32_t ref_col_index;     /* column in referenced table */
    FkAction on_delete;
    FkAction on_update;
} ForeignKey;

#define MAX_FK 8

/* Check constraint: callback that validates a record */
typedef bool (*CheckFn)(const void *record, uint32_t record_size);

#define MAX_CHECKS 8

typedef struct {
    char       name[MAX_TABLE_NAME];
    ColDef     cols[MAX_COLS];
    uint32_t   num_cols;
    uint32_t   record_size;    /* computed: sum of col sizes */

    IndexDef   indexes[MAX_INDEXES];
    uint32_t   num_indexes;

    ForeignKey fks[MAX_FK];
    uint32_t   num_fks;

    CheckFn    checks[MAX_CHECKS];
    uint32_t   num_checks;

    /* storage */
    uint32_t   first_page;     /* first data page for this table */
    uint32_t   row_count;
    int64_t    auto_inc_next;  /* next auto-increment value */
} TableDef;

/* ── Database Handle ────────────────────────────────────── */

#define MAX_TABLES 24
#define DB_MAGIC   0x4C524D44  /* "LRMD" */
#define DB_VERSION 1

typedef struct {
    /* header page (page 0) */
    uint32_t   magic;
    uint32_t   version;
    uint32_t   num_tables;
    uint32_t   free_list_head;  /* first free page */

    /* runtime */
    BufferPool pool;
    Wal        wal;
    TableDef   tables[MAX_TABLES];
    char       path[512];
    char       lock_path[520]; /* path + ".lock" */
    FILE      *lock_fp;        /* held open to enforce single-writer */
    bool       open;
} Database;

/* ── Public API: Engine ─────────────────────────────────── */

/* Database lifecycle */
int  db_open(Database *db, const char *path);
int  db_close(Database *db);
int  db_create(Database *db, const char *path);

/* Buffer pool */
int  pool_init(BufferPool *pool, FILE *fp);
Page *pool_get(BufferPool *pool, uint32_t page_id);
int  pool_mark_dirty(BufferPool *pool, uint32_t page_id);
int  pool_flush(BufferPool *pool);
uint32_t pool_alloc_page(BufferPool *pool);
int  pool_free_page(BufferPool *pool, uint32_t page_id);

/* WAL */
int  wal_open(Wal *wal, const char *db_path);
int  wal_close(Wal *wal);
int  wal_log_page(Wal *wal, uint32_t txn_id, uint32_t page_id,
                  const uint8_t *before_image);
int  wal_recover(Wal *wal, BufferPool *pool);
int  wal_checkpoint(Wal *wal);

/* Transactions */
int  txn_begin(Txn *txn, Database *db);
int  txn_commit(Txn *txn);
int  txn_rollback(Txn *txn);

/* B-Tree */
int  btree_create(BufferPool *pool, BTreeMeta *meta,
                  uint32_t key_size, uint32_t val_size);
int  btree_insert(BufferPool *pool, BTreeMeta *meta,
                  const void *key, const void *val);
int  btree_find(BufferPool *pool, BTreeMeta *meta,
                const void *key, void *val_out);
int  btree_delete(BufferPool *pool, BTreeMeta *meta, const void *key);
int  btree_find_all(BufferPool *pool, BTreeMeta *meta,
                    const void *key, RecordPtr *results, uint32_t *count,
                    uint32_t max_results);
int  btree_prefix_scan(BufferPool *pool, BTreeMeta *meta,
                       const void *prefix, uint32_t prefix_len,
                       RecordPtr *results, uint32_t *count,
                       uint32_t max_results);

/* ── Public API: Table Operations ───────────────────────── */

TableDef *find_table(Database *db, const char *name);
int  table_register(Database *db, TableDef *def);
int  table_insert(Database *db, const char *table, const void *record);
int  table_update(Database *db, const char *table,
                  int64_t pk, const void *record);
int  table_delete(Database *db, const char *table, int64_t pk);
int  table_find_by_pk(Database *db, const char *table,
                      int64_t pk, void *record_out);
int  table_find_by_index(Database *db, const char *table,
                         const char *index_name,
                         const void *key, void *results,
                         uint32_t *count, uint32_t max_results);
int  table_scan(Database *db, const char *table,
                bool (*filter)(const void *record, void *ctx),
                void *ctx, void *results,
                uint32_t *count, uint32_t max_results);

/* ── Error Codes ────────────────────────────────────────── */

#define LRM_OK            0
#define LRM_ERR_IO       -1
#define LRM_ERR_CORRUPT  -2
#define LRM_ERR_FULL     -3
#define LRM_ERR_NOTFOUND -4
#define LRM_ERR_EXISTS   -5   /* unique constraint violation */
#define LRM_ERR_FK       -6   /* foreign key violation */
#define LRM_ERR_CHECK    -7   /* check constraint violation */
#define LRM_ERR_NULL     -8   /* NOT NULL violation */
#define LRM_ERR_TXN      -9   /* transaction error */
#define LRM_ERR_SCHEMA   -10  /* schema definition error */
#define LRM_ERR_OVERFLOW -11  /* record too large */

/* ── Utility ────────────────────────────────────────────── */

uint32_t lrm_checksum(const void *data, size_t len);
void     lrm_timestamp(char *buf, size_t len);  /* ISO 8601 into buf */
int64_t  lrm_now_ms(void);  /* milliseconds since epoch */

#endif /* LRM_DB_H */
