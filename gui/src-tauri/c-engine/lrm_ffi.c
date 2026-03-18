/*
 * lrm_ffi.c — C Database Engine FFI for Rust
 *
 * Exposes LRM custom database engine to Rust via C ABI.
 * This wraps the page-based storage, B-tree indexes, and WAL.
 */

#include "lrm_db.h"
/* Note: lrm_schema.h excluded — its lrm_get_lot(Database*,int64_t,Lot*)
   conflicts with the FFI signature lrm_get_lot(LrmDatabase*,const char*) */
#include <stdlib.h>
#include <string.h>

/* ═══════════════════════════════════════════════════════════════
 * DATABASE HANDLE
 * ═══════════════════════════════════════════════════════════════ */

typedef struct {
    BufferPool pool;
    bool initialized;
} LrmDatabase;

/* ═══════════════════════════════════════════════════════════════
 * LIFECYCLE
 * ═══════════════════════════════════════════════════════════════ */

LrmDatabase* lrm_create(const char *path) {
    FILE *fp = fopen(path, "r+b");
    if (!fp) {
        fp = fopen(path, "w+b");
        if (!fp) return NULL;
    }

    LrmDatabase *db = (LrmDatabase*)malloc(sizeof(LrmDatabase));
    if (!db) {
        fclose(fp);
        return NULL;
    }

    pool_init(&db->pool, fp);
    db->initialized = true;
    return db;
}

void lrm_destroy(LrmDatabase *db) {
    if (!db) return;

    if (db->pool.fp) {
        fclose(db->pool.fp);
    }

    free(db);
}

/* ═══════════════════════════════════════════════════════════════
 * QUERY API — Simplified for Rust FFI
 * ═══════════════════════════════════════════════════════════════ */

/* Result codes */
#define LRM_FFI_OK          0
#define LRM_FFI_ERR        -1
#define LRM_FFI_NOT_FOUND  -2
#define LRM_FFI_EXISTS     -3

/* Query result — caller must free */
typedef struct {
    char *json;      /* JSON-encoded result */
    int   row_count;
    int   error_code;
    char  error_msg[256];
} LrmResult;

/* Execute SQL-like query (simplified for burn-in domain) */
LrmResult* lrm_query(LrmDatabase *db, const char *query) {
    LrmResult *result = (LrmResult*)malloc(sizeof(LrmResult));
    if (!result) return NULL;

    memset(result, 0, sizeof(LrmResult));

    /* TODO: Implement actual query parser */
    /* For now, return placeholder */
    result->json = strdup("{\"rows\": [], \"count\": 0}");
    result->row_count = 0;
    result->error_code = LRM_FFI_OK;

    return result;
}

/* Get JSON string from result */
const char* lrm_result_json(LrmResult *result) {
    if (!result) return NULL;
    return result->json;
}

/* Get row count from result */
int lrm_result_count(LrmResult *result) {
    if (!result) return 0;
    return result->row_count;
}

/* Get error code from result */
int lrm_result_error(LrmResult *result) {
    if (!result) return LRM_FFI_ERR;
    return result->error_code;
}

/* Free query result */
void lrm_free_result(LrmResult *result) {
    if (!result) return;

    if (result->json) {
        free(result->json);
    }

    free(result);
}

/* ═══════════════════════════════════════════════════════════════
 * DOMAIN QUERIES — Burn-in specific
 * ═══════════════════════════════════════════════════════════════ */

/* Get all controllers */
LrmResult* lrm_get_controllers(LrmDatabase *db) {
    return lrm_query(db, "SELECT * FROM controllers");
}

/* Get controller by ID */
LrmResult* lrm_get_controller(LrmDatabase *db, const char *id) {
    /* TODO: Implement */
    return lrm_query(db, "SELECT * FROM controllers WHERE id = ?");
}

/* Insert controller */
int lrm_insert_controller(
    LrmDatabase *db,
    const char *id,
    const char *ip_address,
    const char *mac_address,
    int status,
    const char *firmware_version
) {
    /* TODO: Implement */
    return LRM_FFI_OK;
}

/* Get all boards */
LrmResult* lrm_get_boards(LrmDatabase *db) {
    return lrm_query(db, "SELECT * FROM boards");
}

/* Get boards by LOT */
LrmResult* lrm_get_boards_by_lot(LrmDatabase *db, const char *lot_id) {
    return lrm_query(db, "SELECT * FROM boards WHERE lot_id = ?");
}

/* Get LOT by ID */
LrmResult* lrm_get_lot(LrmDatabase *db, const char *id) {
    return lrm_query(db, "SELECT * FROM lots WHERE id = ?");
}

/* Insert LOT */
int lrm_insert_lot(
    LrmDatabase *db,
    const char *id,
    const char *project_id,
    const char *system_id,
    const char *lot_number,
    const char *customer_lot,
    int expected_qty
) {
    /* TODO: Implement */
    return LRM_FFI_OK;
}

/* Get all LOTs */
LrmResult* lrm_get_lots(LrmDatabase *db) {
    return lrm_query(db, "SELECT * FROM lots");
}

/* Advance LOT step */
int lrm_advance_lot(LrmDatabase *db, const char *lot_id) {
    /* TODO: Implement */
    return LRM_FFI_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * SCHEMA INITIALIZATION
 * ═══════════════════════════════════════════════════════════════ */

int lrm_init_schema(LrmDatabase *db) {
    /* TODO: Call schema_init from lrm_schema.h */
    return LRM_FFI_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * ERROR HANDLING
 * ═══════════════════════════════════════════════════════════════ */

const char* lrm_get_error(int error_code) {
    switch (error_code) {
        case LRM_FFI_OK:          return "Success";
        case LRM_FFI_ERR:         return "General error";
        case LRM_FFI_NOT_FOUND:   return "Not found";
        case LRM_FFI_EXISTS:      return "Already exists";
        default:                  return "Unknown error";
    }
}
