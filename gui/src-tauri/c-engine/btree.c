/*
 * btree.c — B-tree index for the LRM database engine
 *
 * On-disk B-tree stored across pages. Each node is one page.
 * Leaf nodes store key-value pairs.
 * Internal nodes store keys and child page pointers.
 *
 * Key comparison: memcmp on fixed-size keys.
 * For int64 keys, we store them big-endian so memcmp gives correct ordering.
 */

#include "lrm_db.h"
#include <stdlib.h>
#include <string.h>

/* ── Key encoding helpers ───────────────────────────────── */

/* Store int64 as big-endian for correct memcmp ordering */
void btree_encode_i64(int64_t val, uint8_t *buf) {
    /* flip sign bit so negative < positive in unsigned comparison */
    uint64_t u = (uint64_t)val ^ ((uint64_t)1 << 63);
    for (int i = 7; i >= 0; i--) {
        buf[i] = (uint8_t)(u & 0xFF);
        u >>= 8;
    }
}

int64_t btree_decode_i64(const uint8_t *buf) {
    uint64_t u = 0;
    for (int i = 0; i < 8; i++) {
        u = (u << 8) | buf[i];
    }
    return (int64_t)(u ^ ((uint64_t)1 << 63));
}

/* ── Node layout within a page ──────────────────────────── */

/*
 * Leaf node:
 *   PageHeader (16 bytes)
 *   cells[]: array of (key[key_size] + val[val_size])
 *   num_cells stored in header
 *
 * Internal node:
 *   PageHeader (16 bytes)
 *   cells[]: array of (key[key_size] + child_page_id[4])
 *   right_ptr in header = rightmost child
 *   num_cells stored in header
 *
 * Max cells per node depends on key/val size.
 */

typedef struct {
    uint16_t page_type;
    uint16_t num_cells;
    uint32_t right_ptr;
    uint32_t next_page;     /* unused for btree, or leaf chain */
    uint32_t free_space;    /* unused — we compute from num_cells */
} BNodeHeader;

static uint32_t leaf_cell_size(const BTreeMeta *m) {
    return m->key_size + m->val_size;
}

static uint32_t internal_cell_size(const BTreeMeta *m) {
    return m->key_size + 4; /* key + child page_id */
}

static uint32_t max_leaf_cells(const BTreeMeta *m) {
    return (PAGE_SIZE - PAGE_HEADER_SZ) / leaf_cell_size(m);
}

static uint32_t max_internal_cells(const BTreeMeta *m) {
    return (PAGE_SIZE - PAGE_HEADER_SZ) / internal_cell_size(m);
}

/* Get pointer to cell i in a page */
static uint8_t *cell_at(Page *p, uint32_t i, uint32_t cell_size) {
    return &p->data[PAGE_HEADER_SZ + i * cell_size];
}

static const uint8_t *cell_at_const(const Page *p, uint32_t i, uint32_t cell_size) {
    return &p->data[PAGE_HEADER_SZ + i * cell_size];
}

static void read_node_header(const Page *p, BNodeHeader *h) {
    memcpy(&h->page_type,  &p->data[0],  2);
    memcpy(&h->num_cells,  &p->data[2],  2);
    memcpy(&h->right_ptr,  &p->data[4],  4);
    memcpy(&h->next_page,  &p->data[8],  4);
    memcpy(&h->free_space, &p->data[12], 4);
}

static void write_node_cells(Page *p, uint16_t num_cells) {
    memcpy(&p->data[2], &num_cells, 2);
}

static void write_node_right_ptr(Page *p, uint32_t right_ptr) {
    memcpy(&p->data[4], &right_ptr, 4);
}

static void write_node_type(Page *p, uint16_t type) {
    memcpy(&p->data[0], &type, 2);
}

static void write_node_next_page(Page *p, uint32_t next_page) {
    memcpy(&p->data[8], &next_page, 4);
}

static uint32_t read_node_next_page(const Page *p) {
    uint32_t v;
    memcpy(&v, &p->data[8], 4);
    return v;
}

/* ── Binary search within a node ────────────────────────── */

/*
 * Returns index of first cell with key >= search_key.
 * If all keys < search_key, returns num_cells.
 */
static uint32_t node_search(const Page *p, uint32_t num_cells,
                            uint32_t cell_size, uint32_t key_size,
                            const void *search_key) {
    uint32_t lo = 0, hi = num_cells;
    while (lo < hi) {
        uint32_t mid = lo + (hi - lo) / 2;
        const uint8_t *cell_key = cell_at_const(p, mid, cell_size);
        int cmp = memcmp(cell_key, search_key, key_size);
        if (cmp < 0) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    return lo;
}

/*
 * Descend through an internal node to find the correct child page.
 *
 * Layout: cell[i] = (key[i], child_ptr[i]) where child_ptr[i] points
 * to keys LESS THAN key[i]. right_ptr points to keys >= last key.
 *
 * Promoted keys (from leaf splits) are the FIRST key of the right node,
 * so exact matches must descend RIGHT (the leaf containing that key).
 */
static uint32_t descend_internal(const Page *p, const BNodeHeader *h,
                                  const BTreeMeta *meta, const void *key) {
    uint32_t cs = internal_cell_size(meta);
    uint32_t idx = node_search(p, h->num_cells, cs, meta->key_size, key);

    if (idx < h->num_cells) {
        const uint8_t *cell = cell_at_const(p, idx, cs);
        if (memcmp(cell, key, meta->key_size) == 0) {
            /* exact match — key lives in the RIGHT subtree */
            if (idx + 1 < h->num_cells) {
                uint32_t child;
                memcpy(&child, cell_at_const(p, idx + 1, cs) + meta->key_size, 4);
                return child;
            } else {
                return h->right_ptr;
            }
        } else {
            /* key < cell[idx].key — go to left subtree of cell[idx] */
            uint32_t child;
            memcpy(&child, cell + meta->key_size, 4);
            return child;
        }
    } else {
        /* key > all keys — rightmost child */
        return h->right_ptr;
    }
}

/* ── Create ─────────────────────────────────────────────── */

int btree_create(BufferPool *pool, BTreeMeta *meta,
                 uint32_t key_size, uint32_t val_size) {
    meta->key_size = key_size;
    meta->val_size = val_size;

    /* allocate root as empty leaf */
    uint32_t root_id = pool_alloc_page(pool);
    Page *root = pool_get(pool, root_id);
    if (!root) return LRM_ERR_IO;

    memset(root->data, 0, PAGE_SIZE);
    write_node_type(root, PAGE_LEAF);
    write_node_cells(root, 0);
    pool_mark_dirty(pool, root_id);

    meta->root_page = root_id;
    return LRM_OK;
}

/* ── Find ───────────────────────────────────────────────── */

int btree_find(BufferPool *pool, BTreeMeta *meta,
               const void *key, void *val_out) {
    uint32_t page_id = meta->root_page;

    for (;;) {
        Page *p = pool_get(pool, page_id);
        if (!p) return LRM_ERR_IO;

        BNodeHeader h;
        read_node_header(p, &h);

        if (h.page_type == PAGE_LEAF) {
            uint32_t cs = leaf_cell_size(meta);
            uint32_t idx = node_search(p, h.num_cells, cs,
                                       meta->key_size, key);
            if (idx < h.num_cells) {
                const uint8_t *cell = cell_at_const(p, idx, cs);
                if (memcmp(cell, key, meta->key_size) == 0) {
                    memcpy(val_out, cell + meta->key_size, meta->val_size);
                    return LRM_OK;
                }
            }
            return LRM_ERR_NOTFOUND;
        }

        /* internal node — descend */
        page_id = descend_internal(p, &h, meta, key);
    }
}

/* ── Insert (with split) ────────────────────────────────── */

/* Result of inserting into a node that may split */
typedef struct {
    bool     did_split;
    uint8_t  promoted_key[1024]; /* key pushed up to parent */
    uint32_t new_page_id;       /* right sibling after split */
} InsertResult;

static int btree_insert_internal(BufferPool *pool, BTreeMeta *meta,
                                 uint32_t page_id, const void *key,
                                 const void *val, InsertResult *result);

/* Insert into a leaf node */
static int leaf_insert(BufferPool *pool, BTreeMeta *meta,
                       uint32_t page_id, Page *p, BNodeHeader *h,
                       const void *key, const void *val,
                       InsertResult *result) {
    uint32_t cs = leaf_cell_size(meta);
    uint32_t max_cells = max_leaf_cells(meta);
    uint32_t idx = node_search(p, h->num_cells, cs, meta->key_size, key);

    /* For unique enforcement, the TABLE layer handles this via check_unique().
     * The B-tree just stores key-value pairs and allows duplicates.
     * This is critical for non-unique secondary indexes (e.g., location_id). */

    result->did_split = false;

    if (h->num_cells < max_cells) {
        /* room — shift cells right and insert */
        for (uint32_t i = h->num_cells; i > idx; i--) {
            memcpy(cell_at(p, i, cs), cell_at_const(p, i - 1, cs), cs);
        }
        uint8_t *cell = cell_at(p, idx, cs);
        memcpy(cell, key, meta->key_size);
        memcpy(cell + meta->key_size, val, meta->val_size);
        h->num_cells++;
        write_node_cells(p, h->num_cells);
        pool_mark_dirty(pool, page_id);
        return LRM_OK;
    }

    /* need to split */
    uint32_t new_id = pool_alloc_page(pool);
    Page *new_p = pool_get(pool, new_id);
    if (!new_p) return LRM_ERR_IO;

    memset(new_p->data, 0, PAGE_SIZE);
    write_node_type(new_p, PAGE_LEAF);

    /* collect all cells + new cell, then split */
    uint32_t total = h->num_cells + 1;
    uint32_t mid = total / 2;

    /* temp buffer for all cells */
    uint8_t *tmp = malloc(total * cs);
    if (!tmp) return LRM_ERR_IO;

    /* copy existing cells, inserting new one at idx */
    uint32_t j = 0;
    for (uint32_t i = 0; i < total; i++) {
        if (i == idx) {
            memcpy(tmp + i * cs, key, meta->key_size);
            memcpy(tmp + i * cs + meta->key_size, val, meta->val_size);
        } else {
            memcpy(tmp + i * cs, cell_at_const(p, j, cs), cs);
            j++;
        }
    }

    /* left node gets [0..mid-1], right gets [mid..total-1] */
    memset(p->data + PAGE_HEADER_SZ, 0, PAGE_SIZE - PAGE_HEADER_SZ);
    for (uint32_t i = 0; i < mid; i++) {
        memcpy(cell_at(p, i, cs), tmp + i * cs, cs);
    }
    write_node_cells(p, (uint16_t)mid);
    pool_mark_dirty(pool, page_id);

    uint32_t right_count = total - mid;
    for (uint32_t i = 0; i < right_count; i++) {
        memcpy(cell_at(new_p, i, cs), tmp + (mid + i) * cs, cs);
    }
    write_node_cells(new_p, (uint16_t)right_count);
    pool_mark_dirty(pool, new_id);

    /* maintain leaf chain: left → new_right → old_next */
    uint32_t old_next = read_node_next_page(p);
    write_node_next_page(p, new_id);        /* left.next = right */
    write_node_next_page(new_p, old_next);  /* right.next = old_next */
    pool_mark_dirty(pool, page_id);
    pool_mark_dirty(pool, new_id);

    /* promote first key of right node */
    result->did_split = true;
    memcpy(result->promoted_key, cell_at_const(new_p, 0, cs), meta->key_size);
    result->new_page_id = new_id;

    free(tmp);
    return LRM_OK;
}

/* Insert into an internal node (after child split) */
static int internal_insert_key(BufferPool *pool, BTreeMeta *meta,
                               uint32_t page_id, Page *p, BNodeHeader *h,
                               const void *key, uint32_t left_child,
                               uint32_t right_child, InsertResult *result) {
    uint32_t cs = internal_cell_size(meta);
    uint32_t max_cells = max_internal_cells(meta);
    uint32_t idx = node_search(p, h->num_cells, cs, meta->key_size, key);

    result->did_split = false;

    if (h->num_cells < max_cells) {
        /* shift right */
        for (uint32_t i = h->num_cells; i > idx; i--) {
            memcpy(cell_at(p, i, cs), cell_at_const(p, i - 1, cs), cs);
        }
        uint8_t *cell = cell_at(p, idx, cs);
        memcpy(cell, key, meta->key_size);
        memcpy(cell + meta->key_size, &left_child, 4);

        /* update: the child to the right of this key */
        if (idx + 1 < h->num_cells + 1) {
            /* next cell's child ptr becomes right_child, or right_ptr */
        }
        /* Actually: in our layout, cell[i].child_ptr = left child of key[i] */
        /* right_ptr = rightmost child */
        /* After inserting key at idx: */
        /*   cell[idx].child = left_child (subtree < key) */
        /*   right of key = either cell[idx+1].child or right_ptr */
        /* We need to set: if idx was at the end, right_ptr = right_child */
        /* Otherwise, cell[idx+1] already has correct left child */

        /* Simpler model: cell[i] = (key, child_ptr) where child_ptr is the
         * page for keys LESS than key[i]. right_ptr is for keys >= last key.
         * When we insert a promoted key between positions:
         *   cell[idx].child = left_child
         *   The pointer that was at right_ptr or cell[idx+1] stays correct
         *   BUT we need right_child to be accessible.
         *   right_child should replace what was previously pointing to the
         *   combined node. */

        /* Let me use a cleaner model:
         * Internal node has N keys and N+1 children.
         * children[0] key[0] children[1] key[1] ... key[N-1] children[N]
         * We store: cell[i] = (key[i], children[i])  (left pointer)
         *           right_ptr = children[N]           (rightmost pointer)
         *
         * When child at position `pos` splits:
         *   - left_child is the original (now smaller)
         *   - right_child is the new page
         *   - promoted key separates them
         *   - Insert promoted key at position `pos`
         *   - cell[pos].child = left_child
         *   - what was cell[pos].child (or right_ptr) needs to become right_child
         *     ... actually the right_child needs to be the "right" of the new key
         *
         * After shift and insert:
         *   cell[idx].key = promoted_key
         *   cell[idx].child = left_child
         *   right of promoted_key = right_child
         *   But right of promoted_key is cell[idx+1].child (or right_ptr if idx is last)
         *
         * So: cell[idx+1].child should be right_child? No, that shifts the tree.
         *
         * I think the cleanest approach:
         *   cell[idx] = (key, left_child)
         *   then set the pointer that comes AFTER idx to right_child.
         *   If idx+1 < num_cells+1, then cell[idx+1].child = right_child? No...
         *
         * Let me just do: after inserting at idx, the right_child goes into
         * what was the next pointer. For the rightmost case, right_ptr = right_child.
         * For others, we shift cells so cell[idx+1]'s child becomes right_child.
         */

        /* Simplification: cell[idx].child = left_child always.
         * If idx == h->num_cells (was at end), set right_ptr = right_child.
         * Otherwise, the existing cell at idx+1 already had the correct pointer,
         * but we just shifted it. The new cell[idx+1] (which was cell[idx] before shift)
         * had the pointer to the node that was BEFORE the split. That needs to stay.
         * Actually... let me think again.
         *
         * Before split, we had: ... child_A key_B child_C ...
         * child_A split into left_child and right_child with promoted_key between them.
         * We need: ... left_child promoted_key right_child key_B child_C ...
         *
         * So: insert (promoted_key, left_child) at position idx.
         * The pointer AFTER promoted_key should be right_child.
         * That pointer is currently cell[idx+1].child (or right_ptr).
         * But cell[idx+1] was shifted from cell[idx], which had child_A.
         * So cell[idx+1].child = child_A (the old combined node page).
         * We need to replace that with right_child.
         */

        /* Fix: after insert and shift, update the pointer after idx */
        h->num_cells++;
        write_node_cells(p, h->num_cells);

        if (idx + 1 < h->num_cells) {
            /* update child ptr of next cell to right_child */
            uint8_t *next_cell = cell_at(p, idx + 1, cs);
            memcpy(next_cell + meta->key_size, &right_child, 4);
        } else {
            /* inserted at end — right_ptr = right_child */
            write_node_right_ptr(p, right_child);
        }

        pool_mark_dirty(pool, page_id);
        return LRM_OK;
    }

    /* Need to split internal node */
    uint32_t new_id = pool_alloc_page(pool);
    Page *new_p = pool_get(pool, new_id);
    if (!new_p) return LRM_ERR_IO;
    memset(new_p->data, 0, PAGE_SIZE);
    write_node_type(new_p, PAGE_INTERNAL);

    uint32_t total = h->num_cells + 1;
    uint32_t mid = total / 2;

    uint8_t *tmp = malloc(total * cs);
    if (!tmp) return LRM_ERR_IO;

    /* Collect all cells with new one inserted */
    uint32_t j = 0;
    for (uint32_t i = 0; i < total; i++) {
        if (i == idx) {
            memcpy(tmp + i * cs, key, meta->key_size);
            memcpy(tmp + i * cs + meta->key_size, &left_child, 4);
        } else {
            memcpy(tmp + i * cs, cell_at_const(p, j, cs), cs);
            j++;
        }
    }

    /* Also need to fix the right_child pointer — same logic as above */
    /* For the tmp array, cell[idx+1].child should be right_child */
    if (idx + 1 < total) {
        memcpy(tmp + (idx + 1) * cs + meta->key_size, &right_child, 4);
    }

    /* Left gets [0..mid-1], promote key[mid], right gets [mid+1..total-1] */
    memset(p->data + PAGE_HEADER_SZ, 0, PAGE_SIZE - PAGE_HEADER_SZ);
    for (uint32_t i = 0; i < mid; i++) {
        memcpy(cell_at(p, i, cs), tmp + i * cs, cs);
    }
    write_node_cells(p, (uint16_t)mid);

    /* The child pointer of the promoted key's position becomes left's right_ptr */
    uint32_t promoted_right_child;
    memcpy(&promoted_right_child,
           tmp + mid * cs + meta->key_size, 4);

    /* Actually, for internal split: the promoted key doesn't stay in either node.
     * Left gets keys [0..mid-1] with their children.
     * Right gets keys [mid+1..total-1] with their children.
     * The promoted key[mid] goes up.
     * Left's right_ptr = child to the LEFT of promoted key = left_child_of_key[mid]
     * Actually: child_of_cell[mid] is left of key[mid], and right of key[mid-1].
     * After split:
     *   Left node: cells [0..mid-1], right_ptr = cell[mid].child
     *   Promoted: key[mid]
     *   Right node: cells [mid+1..total-1], right_ptr = old right_ptr (or right_child if at end)
     */

    uint32_t left_right_ptr;
    memcpy(&left_right_ptr, tmp + mid * cs + meta->key_size, 4);
    write_node_right_ptr(p, left_right_ptr);
    pool_mark_dirty(pool, page_id);

    uint32_t right_count = total - mid - 1;
    for (uint32_t i = 0; i < right_count; i++) {
        memcpy(cell_at(new_p, i, cs), tmp + (mid + 1 + i) * cs, cs);
    }
    write_node_cells(new_p, (uint16_t)right_count);
    /* right node's right_ptr: if the rightmost insertion was here, use right_child */
    /* otherwise, use the original right_ptr */
    if (idx >= total - 1) {
        write_node_right_ptr(new_p, right_child);
    } else {
        write_node_right_ptr(new_p, h->right_ptr);
    }
    pool_mark_dirty(pool, new_id);

    /* Promote */
    result->did_split = true;
    memcpy(result->promoted_key, tmp + mid * cs, meta->key_size);
    result->new_page_id = new_id;

    free(tmp);
    return LRM_OK;
}

/* Recursive insert */
static int btree_insert_internal(BufferPool *pool, BTreeMeta *meta,
                                 uint32_t page_id, const void *key,
                                 const void *val, InsertResult *result) {
    Page *p = pool_get(pool, page_id);
    if (!p) return LRM_ERR_IO;

    BNodeHeader h;
    read_node_header(p, &h);

    if (h.page_type == PAGE_LEAF) {
        return leaf_insert(pool, meta, page_id, p, &h, key, val, result);
    }

    /* Internal node — find child to descend */
    uint32_t cs = internal_cell_size(meta);
    uint32_t idx = node_search(p, h.num_cells, cs, meta->key_size, key);

    uint32_t child_id;
    if (idx < h.num_cells) {
        /* check exact match — go right */
        const uint8_t *cell = cell_at_const(p, idx, cs);
        if (memcmp(cell, key, meta->key_size) == 0) {
            /* For unique index, this means duplicate at internal level */
            /* But the real check happens at the leaf. Descend right. */
            if (idx + 1 < h.num_cells) {
                memcpy(&child_id, cell_at_const(p, idx + 1, cs) + meta->key_size, 4);
            } else {
                child_id = h.right_ptr;
            }
        } else {
            memcpy(&child_id, cell + meta->key_size, 4);
        }
    } else {
        child_id = h.right_ptr;
    }

    InsertResult child_result;
    int rc = btree_insert_internal(pool, meta, child_id, key, val, &child_result);
    if (rc != LRM_OK) return rc;

    if (!child_result.did_split) {
        result->did_split = false;
        return LRM_OK;
    }

    /* Child split — insert promoted key into this node */
    /* Re-read page in case buffer pool evicted it */
    p = pool_get(pool, page_id);
    if (!p) return LRM_ERR_IO;
    read_node_header(p, &h);

    return internal_insert_key(pool, meta, page_id, p, &h,
                               child_result.promoted_key,
                               child_id, child_result.new_page_id,
                               result);
}

/* Public insert */
int btree_insert(BufferPool *pool, BTreeMeta *meta,
                 const void *key, const void *val) {
    InsertResult result;
    int rc = btree_insert_internal(pool, meta, meta->root_page,
                                   key, val, &result);
    if (rc != LRM_OK) return rc;

    if (result.did_split) {
        /* Root split — create new root */
        uint32_t new_root_id = pool_alloc_page(pool);
        Page *new_root = pool_get(pool, new_root_id);
        if (!new_root) return LRM_ERR_IO;

        memset(new_root->data, 0, PAGE_SIZE);
        write_node_type(new_root, PAGE_INTERNAL);
        write_node_cells(new_root, 1);

        uint32_t cs = internal_cell_size(meta);
        uint8_t *cell = cell_at(new_root, 0, cs);
        memcpy(cell, result.promoted_key, meta->key_size);
        memcpy(cell + meta->key_size, &meta->root_page, 4);

        write_node_right_ptr(new_root, result.new_page_id);
        pool_mark_dirty(pool, new_root_id);

        meta->root_page = new_root_id;
    }

    return LRM_OK;
}

/* ── Delete ─────────────────────────────────────────────── */

int btree_delete(BufferPool *pool, BTreeMeta *meta, const void *key) {
    /* Simple approach: find the leaf, remove the cell, don't rebalance.
     * For our workload (< 10K records per table), underflow is fine.
     * Pages that become empty can be freed. */

    uint32_t page_id = meta->root_page;
    uint32_t path[64];
    int depth = 0;

    /* traverse to leaf */
    for (;;) {
        Page *p = pool_get(pool, page_id);
        if (!p) return LRM_ERR_IO;

        BNodeHeader h;
        read_node_header(p, &h);

        if (h.page_type == PAGE_LEAF) {
            uint32_t cs = leaf_cell_size(meta);
            uint32_t idx = node_search(p, h.num_cells, cs,
                                       meta->key_size, key);
            if (idx >= h.num_cells ||
                memcmp(cell_at_const(p, idx, cs), key, meta->key_size) != 0) {
                return LRM_ERR_NOTFOUND;
            }

            /* shift cells left */
            for (uint32_t i = idx; i < (uint32_t)(h.num_cells - 1); i++) {
                memcpy(cell_at(p, i, cs), cell_at_const(p, i + 1, cs), cs);
            }
            h.num_cells--;
            write_node_cells(p, h.num_cells);
            pool_mark_dirty(pool, page_id);
            return LRM_OK;
        }

        /* descend */
        path[depth++] = page_id;
        page_id = descend_internal(p, &h, meta, key);
    }
}

/* ── Find all matching keys (prefix match for non-unique indexes) ─ */

int btree_find_all(BufferPool *pool, BTreeMeta *meta,
                   const void *key, RecordPtr *results,
                   uint32_t *count, uint32_t max_results) {
    /* For non-unique indexes, the stored key is (column_value + PK).
     * The search key is just column_value (prefix).
     * We pass the prefix_size through the search key buffer,
     * which is zero-padded after the prefix. The B-tree navigates
     * to where this prefix would be, then we scan forward. */
    *count = 0;

    /* Navigate to the leaf containing the key */
    uint32_t page_id = meta->root_page;

    for (;;) {
        Page *p = pool_get(pool, page_id);
        if (!p) return LRM_ERR_IO;

        BNodeHeader h;
        read_node_header(p, &h);

        if (h.page_type == PAGE_LEAF) {
            uint32_t cs = leaf_cell_size(meta);
            uint32_t idx = node_search(p, h.num_cells, cs,
                                       meta->key_size, key);
            /* collect all matching by full key comparison, following leaf chain */
            for (;;) {
                while (idx < h.num_cells && *count < max_results) {
                    const uint8_t *cell = cell_at_const(p, idx, cs);
                    if (memcmp(cell, key, meta->key_size) != 0)
                        return LRM_OK;
                    memcpy(&results[*count], cell + meta->key_size,
                           sizeof(RecordPtr));
                    (*count)++;
                    idx++;
                }
                if (*count >= max_results) return LRM_OK;
                /* follow leaf chain */
                uint32_t next = read_node_next_page(p);
                if (next == 0) return LRM_OK;
                p = pool_get(pool, next);
                if (!p) return LRM_ERR_IO;
                read_node_header(p, &h);
                idx = 0;
            }
        }

        /* descend */
        page_id = descend_internal(p, &h, meta, key);
    }
}

/* Find all entries where first prefix_len bytes match */
int btree_prefix_scan(BufferPool *pool, BTreeMeta *meta,
                      const void *prefix, uint32_t prefix_len,
                      RecordPtr *results, uint32_t *count,
                      uint32_t max_results) {
    *count = 0;

    /* Build a search key: prefix zero-padded to full key_size */
    uint8_t search_key[1024];
    memset(search_key, 0, meta->key_size);
    memcpy(search_key, prefix, prefix_len);

    /* Navigate to leaf */
    uint32_t page_id = meta->root_page;

    for (;;) {
        Page *p = pool_get(pool, page_id);
        if (!p) return LRM_ERR_IO;

        BNodeHeader h;
        read_node_header(p, &h);

        if (h.page_type == PAGE_LEAF) {
            uint32_t cs = leaf_cell_size(meta);
            uint32_t idx = node_search(p, h.num_cells, cs,
                                       meta->key_size, search_key);
            /* collect all where first prefix_len bytes match, following leaf chain */
            for (;;) {
                while (idx < h.num_cells && *count < max_results) {
                    const uint8_t *cell = cell_at_const(p, idx, cs);
                    if (memcmp(cell, prefix, prefix_len) != 0)
                        return LRM_OK;
                    memcpy(&results[*count], cell + meta->key_size,
                           sizeof(RecordPtr));
                    (*count)++;
                    idx++;
                }
                if (*count >= max_results) return LRM_OK;
                /* follow leaf chain */
                uint32_t next = read_node_next_page(p);
                if (next == 0) return LRM_OK;
                p = pool_get(pool, next);
                if (!p) return LRM_ERR_IO;
                read_node_header(p, &h);
                idx = 0;
            }
        }

        /* descend */
        page_id = descend_internal(p, &h, meta, search_key);
    }
}
