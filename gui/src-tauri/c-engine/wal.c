/*
 * wal.c — Write-Ahead Log for crash recovery
 *
 * Before modifying any page, the WAL records the before-image.
 * On crash recovery, before-images are replayed to undo
 * uncommitted changes.
 *
 * WAL file format:
 *   [WalEntry] [WalEntry] ... [COMMIT marker]
 *
 * COMMIT marker: magic=WAL_MAGIC, txn_id=id, page_id=0xFFFFFFFF
 *
 * Recovery: scan WAL, find uncommitted txns, restore their pages.
 */

#include "lrm_db.h"
#include <stdlib.h>
#include <string.h>

#define WAL_COMMIT_PAGE 0xFFFFFFFF

int wal_open(Wal *wal, const char *db_path) {
    memset(wal, 0, sizeof(Wal));
    snprintf(wal->path, sizeof(wal->path), "%s.wal", db_path);
    wal->fp = fopen(wal->path, "a+b");
    if (!wal->fp) {
        /* first time — create */
        wal->fp = fopen(wal->path, "w+b");
        if (!wal->fp) return LRM_ERR_IO;
    }
    wal->current_txn = 0;
    wal->entry_count = 0;
    return LRM_OK;
}

int wal_close(Wal *wal) {
    if (wal->fp) {
        fclose(wal->fp);
        wal->fp = NULL;
    }
    return LRM_OK;
}

int wal_log_page(Wal *wal, uint32_t txn_id, uint32_t page_id,
                 const uint8_t *before_image) {
    WalEntry entry;
    entry.magic = WAL_MAGIC;
    entry.txn_id = txn_id;
    entry.page_id = page_id;
    memcpy(entry.before_image, before_image, PAGE_SIZE);
    entry.checksum = lrm_checksum(entry.before_image, PAGE_SIZE);

    fseek(wal->fp, 0, SEEK_END);
    size_t n = fwrite(&entry, sizeof(WalEntry), 1, wal->fp);
    if (n != 1) return LRM_ERR_IO;
    fflush(wal->fp);
    wal->entry_count++;
    return LRM_OK;
}

/* Write a commit marker for a transaction */
static int wal_write_commit(Wal *wal, uint32_t txn_id) {
    WalEntry entry;
    memset(&entry, 0, sizeof(WalEntry));
    entry.magic = WAL_MAGIC;
    entry.txn_id = txn_id;
    entry.page_id = WAL_COMMIT_PAGE;  /* sentinel: this is a commit record */
    entry.checksum = 0;

    fseek(wal->fp, 0, SEEK_END);
    size_t n = fwrite(&entry, sizeof(WalEntry), 1, wal->fp);
    if (n != 1) return LRM_ERR_IO;
    fflush(wal->fp);
    return LRM_OK;
}

int wal_recover(Wal *wal, BufferPool *pool) {
    fseek(wal->fp, 0, SEEK_END);
    long size = ftell(wal->fp);
    if (size <= 0) return LRM_OK;  /* nothing to recover */

    size_t entry_size = sizeof(WalEntry);
    uint32_t num_entries = (uint32_t)(size / entry_size);
    if (num_entries == 0) return LRM_OK;

    /* Phase 1: find which txns are committed */
    fseek(wal->fp, 0, SEEK_SET);

    /* track committed txn_ids (simple linear scan — WAL is small) */
    uint32_t *committed = calloc(num_entries, sizeof(uint32_t));
    uint32_t num_committed = 0;
    if (!committed) return LRM_ERR_IO;

    WalEntry entry;
    for (uint32_t i = 0; i < num_entries; i++) {
        if (fread(&entry, entry_size, 1, wal->fp) != 1) break;
        if (entry.magic != WAL_MAGIC) continue;
        if (entry.page_id == WAL_COMMIT_PAGE) {
            committed[num_committed++] = entry.txn_id;
        }
    }

    /* Phase 2: undo uncommitted changes */
    fseek(wal->fp, 0, SEEK_SET);
    for (uint32_t i = 0; i < num_entries; i++) {
        if (fread(&entry, entry_size, 1, wal->fp) != 1) break;
        if (entry.magic != WAL_MAGIC) continue;
        if (entry.page_id == WAL_COMMIT_PAGE) continue;

        /* check if this txn was committed */
        bool is_committed = false;
        for (uint32_t c = 0; c < num_committed; c++) {
            if (committed[c] == entry.txn_id) {
                is_committed = true;
                break;
            }
        }

        if (!is_committed) {
            /* restore the before-image */
            uint32_t chk = lrm_checksum(entry.before_image, PAGE_SIZE);
            if (chk == entry.checksum) {
                Page *p = pool_get(pool, entry.page_id);
                if (p) {
                    memcpy(p->data, entry.before_image, PAGE_SIZE);
                    pool_mark_dirty(pool, entry.page_id);
                }
            }
        }
    }

    pool_flush(pool);
    free(committed);

    /* truncate WAL after recovery */
    return wal_checkpoint(wal);
}

int wal_checkpoint(Wal *wal) {
    if (wal->fp) {
        fclose(wal->fp);
    }
    /* truncate by reopening in write mode */
    wal->fp = fopen(wal->path, "w+b");
    if (!wal->fp) return LRM_ERR_IO;
    wal->entry_count = 0;
    return LRM_OK;
}

/* ── Transaction ────────────────────────────────────────── */

static uint32_t next_txn_id = 1;

int txn_begin(Txn *txn, Database *db) {
    txn->txn_id = next_txn_id++;
    txn->active = true;
    txn->wal = &db->wal;
    txn->pool = &db->pool;
    return LRM_OK;
}

int txn_commit(Txn *txn) {
    if (!txn->active) return LRM_ERR_TXN;

    /* write commit marker to WAL */
    int rc = wal_write_commit(txn->wal, txn->txn_id);
    if (rc != LRM_OK) return rc;

    /* flush dirty pages to disk */
    rc = pool_flush(txn->pool);
    if (rc != LRM_OK) return rc;

    txn->active = false;
    return LRM_OK;
}

int txn_rollback(Txn *txn) {
    if (!txn->active) return LRM_ERR_TXN;

    /* WAL has before-images — recover will undo our changes.
     * For immediate rollback, we scan our txn's entries and restore. */
    Wal *wal = txn->wal;
    fseek(wal->fp, 0, SEEK_SET);

    WalEntry entry;
    size_t entry_size = sizeof(WalEntry);

    while (fread(&entry, entry_size, 1, wal->fp) == 1) {
        if (entry.magic != WAL_MAGIC) continue;
        if (entry.txn_id != txn->txn_id) continue;
        if (entry.page_id == WAL_COMMIT_PAGE) continue;

        /* restore before-image */
        Page *p = pool_get(txn->pool, entry.page_id);
        if (p) {
            memcpy(p->data, entry.before_image, PAGE_SIZE);
            pool_mark_dirty(txn->pool, entry.page_id);
        }
    }

    pool_flush(txn->pool);
    txn->active = false;
    return LRM_OK;
}
