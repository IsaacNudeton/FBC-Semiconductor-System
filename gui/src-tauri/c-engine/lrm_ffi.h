/*
 * lrm_ffi.h — C Database Engine FFI Header for Rust
 */

#ifndef LRM_FFI_H
#define LRM_FFI_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ═══════════════════════════════════════════════════════════════
 * OPAQUE HANDLE
 * ═══════════════════════════════════════════════════════════════ */

typedef struct LrmDatabase LrmDatabase;
typedef struct LrmResult LrmResult;

/* ═══════════════════════════════════════════════════════════════
 * LIFECYCLE
 * ═══════════════════════════════════════════════════════════════ */

/* Create or open database at path */
LrmDatabase* lrm_create(const char *path);

/* Close and free database */
void lrm_destroy(LrmDatabase *db);

/* ═══════════════════════════════════════════════════════════════
 * QUERY API
 * ═══════════════════════════════════════════════════════════════ */

/* Result codes */
#define LRM_FFI_OK          0
#define LRM_FFI_ERR        -1
#define LRM_FFI_NOT_FOUND  -2
#define LRM_FFI_EXISTS     -3

/* Execute query, returns JSON result */
LrmResult* lrm_query(LrmDatabase *db, const char *query);

/* Free query result */
void lrm_free_result(LrmResult *result);

/* Get result as JSON string */
const char* lrm_result_json(LrmResult *result);

/* Get row count */
int lrm_result_count(LrmResult *result);

/* Get error code */
int lrm_result_error(LrmResult *result);

/* ═══════════════════════════════════════════════════════════════
 * DOMAIN QUERIES — Controllers
 * ═══════════════════════════════════════════════════════════════ */

LrmResult* lrm_get_controllers(LrmDatabase *db);
LrmResult* lrm_get_controller(LrmDatabase *db, const char *id);
int lrm_insert_controller(
    LrmDatabase *db,
    const char *id,
    const char *ip_address,
    const char *mac_address,
    int status,
    const char *firmware_version
);

/* ═══════════════════════════════════════════════════════════════
 * DOMAIN QUERIES — Boards
 * ═══════════════════════════════════════════════════════════════ */

LrmResult* lrm_get_boards(LrmDatabase *db);
LrmResult* lrm_get_boards_by_lot(LrmDatabase *db, const char *lot_id);
int lrm_insert_board(
    LrmDatabase *db,
    const char *id,
    const char *lot_id,
    const char *system_id,
    int shelf,
    int tray,
    int slot,
    const char *position_label,
    const char *slot_label,
    int status,
    const char *serial,
    const char *device_id
);

/* ═══════════════════════════════════════════════════════════════
 * DOMAIN QUERIES — LOTs
 * ═══════════════════════════════════════════════════════════════ */

LrmResult* lrm_get_lots(LrmDatabase *db);
LrmResult* lrm_get_lot(LrmDatabase *db, const char *id);
int lrm_insert_lot(
    LrmDatabase *db,
    const char *id,
    const char *project_id,
    const char *system_id,
    const char *lot_number,
    const char *customer_lot,
    int expected_qty
);
int lrm_advance_lot(LrmDatabase *db, const char *lot_id);

/* ═══════════════════════════════════════════════════════════════
 * SCHEMA
 * ═══════════════════════════════════════════════════════════════ */

int lrm_init_schema(LrmDatabase *db);

/* ═══════════════════════════════════════════════════════════════
 * ERROR HANDLING
 * ═══════════════════════════════════════════════════════════════ */

const char* lrm_get_error(int error_code);

#ifdef __cplusplus
}
#endif

#endif /* LRM_FFI_H */
