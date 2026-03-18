/*
 * v1_compat.c — V1 compatibility tables + user/location additions (v5)
 *
 * CRUD for v1_boards, v1_board_types, v1_board_logs, v1_socket_notes.
 * Additional user functions (get, find_by_name, delete, set_active, set_password_hash).
 * Additional location functions (list_all, delete).
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <string.h>
#include <stdio.h>

extern void btree_encode_i64(int64_t val, uint8_t *buf);

/* ── Audit helper ──────────────────────────────────────── */

static int v_audit(Database *db, int64_t uid, AuditAction act,
                   const char *tbl, int64_t eid, const char *detail) {
    AuditEntry e = {0};
    e.user_id = uid;
    e.action = act;
    strncpy(e.entity_table, tbl, 63);
    e.entity_id = eid;
    e.timestamp_ms = lrm_now_ms();
    if (detail) strncpy(e.detail, detail, MAX_TEXT_LEN-1);
    return table_insert(db, "audit_log", &e);
}

/* ══════════════════════════════════════════════════════════
 *  V1 Boards
 * ══════════════════════════════════════════════════════════ */

int lrm_create_v1_board(Database *db, V1Board *board, int64_t uid) {
    int rc = table_insert(db, "v1_boards", board);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Created board: %s %s", board->customer, board->serial_no);
    v_audit(db, uid, AUDIT_CREATE, "v1_boards", board->board_id, d);
    return LRM_OK;
}

int lrm_get_v1_board(Database *db, int64_t bid, V1Board *out) {
    return table_find_by_pk(db, "v1_boards", bid, out);
}

int lrm_update_v1_board(Database *db, V1Board *board, int64_t uid) {
    int rc = table_update(db, "v1_boards", board->board_id, board);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Updated board: %s %s", board->customer, board->serial_no);
    v_audit(db, uid, AUDIT_UPDATE, "v1_boards", board->board_id, d);
    return LRM_OK;
}

int lrm_list_v1_boards(Database *db, V1Board *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "v1_boards", NULL, NULL, out, count, max);
}

/* Filter context for find_v1_board */
typedef struct {
    const char *customer;
    const char *pcb;
    const char *serial;
} FindBoardCtx;

static bool filter_v1_board(const void *record, void *ctx) {
    const V1Board *b = record;
    const FindBoardCtx *f = ctx;
    if (strcmp(b->customer, f->customer) != 0) return false;
    if (strcmp(b->pcb_number_text, f->pcb) != 0) return false;
    if (strcmp(b->serial_no, f->serial) != 0) return false;
    return true;
}

int lrm_find_v1_board(Database *db, const char *customer, const char *pcb,
                       const char *serial, V1Board *out) {
    FindBoardCtx ctx = { customer, pcb, serial };
    V1Board results[1]; uint32_t cnt = 0;
    int rc = table_scan(db, "v1_boards", filter_v1_board, &ctx, results, &cnt, 1);
    if (rc != LRM_OK || cnt == 0) return LRM_ERR_NOTFOUND;
    *out = results[0];
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 *  V1 Board Types
 * ══════════════════════════════════════════════════════════ */

int lrm_create_v1_board_type(Database *db, V1BoardType *bt, int64_t uid) {
    int rc = table_insert(db, "v1_board_types", bt);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Created board type: %s %s", bt->customer, bt->pcb_number_text);
    v_audit(db, uid, AUDIT_CREATE, "v1_board_types", bt->board_type_id, d);
    return LRM_OK;
}

int lrm_get_v1_board_type(Database *db, int64_t btid, V1BoardType *out) {
    return table_find_by_pk(db, "v1_board_types", btid, out);
}

int lrm_update_v1_board_type(Database *db, V1BoardType *bt, int64_t uid) {
    int rc = table_update(db, "v1_board_types", bt->board_type_id, bt);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Updated board type: %s %s", bt->customer, bt->pcb_number_text);
    v_audit(db, uid, AUDIT_UPDATE, "v1_board_types", bt->board_type_id, d);
    return LRM_OK;
}

int lrm_delete_v1_board_type(Database *db, int64_t btid, int64_t uid) {
    V1BoardType bt;
    int rc = table_find_by_pk(db, "v1_board_types", btid, &bt);
    if (rc != LRM_OK) return rc;
    rc = table_delete(db, "v1_board_types", btid);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Deleted board type: %s %s", bt.customer, bt.pcb_number_text);
    v_audit(db, uid, AUDIT_DELETE, "v1_board_types", btid, d);
    return LRM_OK;
}

int lrm_list_v1_board_types(Database *db, V1BoardType *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "v1_board_types", NULL, NULL, out, count, max);
}

/* Filter context for find_v1_board_type */
typedef struct {
    const char *customer;
    const char *pcb;
    const char *rev;
} FindBoardTypeCtx;

static bool filter_v1_board_type(const void *record, void *ctx) {
    const V1BoardType *bt = record;
    const FindBoardTypeCtx *f = ctx;
    if (strcmp(bt->customer, f->customer) != 0) return false;
    if (strcmp(bt->pcb_number_text, f->pcb) != 0) return false;
    if (strcmp(bt->revision, f->rev) != 0) return false;
    return true;
}

int lrm_find_v1_board_type(Database *db, const char *customer, const char *pcb,
                            const char *rev, V1BoardType *out) {
    FindBoardTypeCtx ctx = { customer, pcb, rev };
    V1BoardType results[1]; uint32_t cnt = 0;
    int rc = table_scan(db, "v1_board_types", filter_v1_board_type, &ctx, results, &cnt, 1);
    if (rc != LRM_OK || cnt == 0) return LRM_ERR_NOTFOUND;
    *out = results[0];
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 *  V1 Board Logs
 * ══════════════════════════════════════════════════════════ */

int lrm_create_v1_board_log(Database *db, V1BoardLog *log) {
    return table_insert(db, "v1_board_logs", log);
}

/* Filter for board logs by board_id */
static bool filter_blog_by_board(const void *record, void *ctx) {
    const V1BoardLog *l = record;
    int64_t bid = *(int64_t *)ctx;
    return l->board_id == bid;
}

int lrm_list_v1_board_logs(Database *db, int64_t board_id,
                            V1BoardLog *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "v1_board_logs", filter_blog_by_board, &board_id, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  V1 Socket Notes
 * ══════════════════════════════════════════════════════════ */

/* Filter context for upsert: find by board_id + socket_number */
typedef struct {
    int64_t board_id;
    int32_t socket_number;
} FindSocketCtx;

static bool filter_socket_match(const void *record, void *ctx) {
    const V1SocketNote *sn = record;
    const FindSocketCtx *f = ctx;
    return sn->board_id == f->board_id && sn->socket_number == f->socket_number;
}

int lrm_upsert_v1_socket(Database *db, V1SocketNote *sn) {
    /* Try to find existing note for this board+socket */
    FindSocketCtx ctx = { sn->board_id, sn->socket_number };
    V1SocketNote existing[1]; uint32_t cnt = 0;
    table_scan(db, "v1_socket_notes", filter_socket_match, &ctx, existing, &cnt, 1);
    if (cnt > 0) {
        /* Update existing */
        sn->note_id = existing[0].note_id;
        return table_update(db, "v1_socket_notes", sn->note_id, sn);
    }
    /* Insert new */
    return table_insert(db, "v1_socket_notes", sn);
}

/* Filter for socket notes by board_id */
static bool filter_sock_by_board(const void *record, void *ctx) {
    const V1SocketNote *sn = record;
    int64_t bid = *(int64_t *)ctx;
    return sn->board_id == bid;
}

int lrm_list_v1_sockets(Database *db, int64_t board_id,
                         V1SocketNote *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "v1_socket_notes", filter_sock_by_board, &board_id, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  User Additions
 * ══════════════════════════════════════════════════════════ */

int lrm_get_user(Database *db, int64_t uid, User *out) {
    return table_find_by_pk(db, "users", uid, out);
}

int lrm_find_user_by_name(Database *db, const char *username, User *out) {
    uint8_t key[MAX_TEXT_LEN]; memset(key, 0, MAX_TEXT_LEN);
    strncpy((char*)key, username, MAX_TEXT_LEN-1);
    User users[1]; uint32_t cnt = 0;
    int rc = table_find_by_index(db, "users", "uq_username", key, users, &cnt, 1);
    if (rc != LRM_OK || cnt == 0) return LRM_ERR_NOTFOUND;
    *out = users[0];
    return LRM_OK;
}

int lrm_delete_user(Database *db, int64_t uid_user, int64_t uid_actor) {
    User u;
    int rc = table_find_by_pk(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    rc = table_delete(db, "users", uid_user);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Deleted user: %s", u.username);
    v_audit(db, uid_actor, AUDIT_DELETE, "users", uid_user, d);
    return LRM_OK;
}

int lrm_set_user_active(Database *db, int64_t uid_user, int32_t active, int64_t uid_actor) {
    User u;
    int rc = table_find_by_pk(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    u.active = active;
    rc = table_update(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Set user %s active=%d", u.username, active);
    v_audit(db, uid_actor, AUDIT_STATUS, "users", uid_user, d);
    return LRM_OK;
}

int lrm_set_user_password_hash(Database *db, int64_t uid_user, const char *hash) {
    User u;
    int rc = table_find_by_pk(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    memset(u.password_hash, 0, HASH_LEN);
    strncpy(u.password_hash, hash, HASH_LEN-1);
    return table_update(db, "users", uid_user, &u);
}

int lrm_set_user_role(Database *db, int64_t uid_user, int32_t role, int64_t uid_actor) {
    if (role < 0 || role >= ROLE__COUNT) return LRM_ERR_CHECK;
    User u;
    int rc = table_find_by_pk(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    /* Prevent removing the last active admin */
    if (u.role == ROLE_ADMIN && role != ROLE_ADMIN && u.active) {
        User list[64];
        uint32_t cnt = 0;
        if (lrm_list_users(db, list, &cnt, 64) == LRM_OK) {
            int admin_active = 0;
            for (uint32_t i = 0; i < cnt; i++) {
                if (list[i].active && list[i].role == ROLE_ADMIN) admin_active++;
            }
            if (admin_active <= 1) return LRM_ERR_CHECK;
        }
    }
    int32_t old_role = u.role;
    u.role = role;
    rc = table_update(db, "users", uid_user, &u);
    if (rc != LRM_OK) return rc;
    char d[256];
    snprintf(d, sizeof(d), "Role %s -> %s", user_role_str((UserRole)old_role), user_role_str((UserRole)role));
    v_audit(db, uid_actor, AUDIT_UPDATE, "users", uid_user, d);
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 *  Location Additions
 * ══════════════════════════════════════════════════════════ */

int lrm_list_all_locations(Database *db, Location *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "locations", NULL, NULL, out, count, max);
}

int lrm_delete_location(Database *db, int64_t lid, int64_t uid) {
    Location loc;
    int rc = table_find_by_pk(db, "locations", lid, &loc);
    if (rc != LRM_OK) return rc;
    rc = table_delete(db, "locations", lid);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Deleted location: %s", loc.name);
    v_audit(db, uid, AUDIT_DELETE, "locations", lid, d);
    return LRM_OK;
}
