/*
 * schema.c — Register all 21 burn-in inventory tables (v5)
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <string.h>
#include <stdio.h>

/* ── Enum strings ───────────────────────────────────────── */

static const char *_sys_type[] = {"HX","Sonoma","XP-160","MCC","Shasta"};
static const char *_sys_stat[] = {"Running","Free","Down","Maintenance"};
static const char *_cool[]     = {"Air","Liquid","None"};
static const char *_loc_type[] = {"Bay","Chamber","Shelf","Tray","Slot","Backplane","Storage"};
static const char *_loc_stat[] = {"Active","Down","Inactive"};
static const char *_proj_stat[]= {"Active","Complete","Hold","Cancelled"};
static const char *_hw_cat[]   = {"BIM","BIB","Board","Powertrain","LCPS","HCPS",
                                  "Core","TempDiode","TempCircuit","NegSupply","Controller"};
static const char *_track[]    = {"Serialized","Quantity","Configured"};
static const char *_item_stat[]= {"Available","In Use","Damaged","Maintenance","Retired"};
static const char *_core_role[]= {"Master","Slave"};
static const char *_audit[]    = {"Create","Update","Delete","Move","Status","Assign","Unassign","QtyAdjust"};
static const char *_role[]     = {"Operator","Tech","Engineer","Admin"};
static const char *_lot_step[] = {"Received","Setup","Loading","Burn-In",
                                  "Readpoint","Unloading","Shipping","Complete"};
static const char *_lot_stat[] = {"Active","Hold","Complete","Cancelled"};
static const char *_sock_stat[]= {"Working","Bad","NotInstalled","Reserved"};
static const char *_up_stat[]  = {"NotLoaded","Loading","Loaded","Error"};
static const char *_task_stat[]= {"Pending","InProgress","Completed","Blocked"};

const char *system_type_str(SystemType s)    { return (s>=0&&s<SYS__COUNT)?_sys_type[s]:"?"; }
const char *system_status_str(SystemStatus s){ return (s>=0&&s<SSTAT__COUNT)?_sys_stat[s]:"?"; }
const char *cooling_str(CoolingType c)       { return (c>=0&&c<COOL__COUNT)?_cool[c]:"?"; }
const char *loc_type_str(LocType t)          { return (t>=0&&t<LOC__COUNT)?_loc_type[t]:"?"; }
const char *loc_status_str(LocStatus s)      { return (s>=0&&s<LSTAT__COUNT)?_loc_stat[s]:"?"; }
const char *project_status_str(ProjectStatus s){ return (s>=0&&s<PROJ__COUNT)?_proj_stat[s]:"?"; }
const char *hw_category_str(HwCategory c)    { return (c>=0&&c<HW__COUNT)?_hw_cat[c]:"?"; }
const char *tracking_mode_str(TrackingMode m){ return (m>=0&&m<TRACK__COUNT)?_track[m]:"?"; }
const char *item_status_str(ItemStatus s)    { return (s>=0&&s<ITEM__COUNT)?_item_stat[s]:"?"; }
const char *core_role_str(CoreRole r)        { return (r>=0&&r<CORE__COUNT)?_core_role[r]:"?"; }
const char *audit_action_str(AuditAction a)  { return (a>=0&&a<AUDIT__COUNT)?_audit[a]:"?"; }
const char *user_role_str(UserRole r)        { return (r>=0&&r<ROLE__COUNT)?_role[r]:"?"; }
const char *lot_step_str(LotStep s)          { return (s>=0&&s<LOT_STEP__COUNT)?_lot_step[s]:"?"; }
const char *lot_status_str(LotStatus s)      { return (s>=0&&s<LSTAT_LOT__COUNT)?_lot_stat[s]:"?"; }
const char *socket_status_str(SocketStatus s){ return (s>=0&&s<SOCK__COUNT)?_sock_stat[s]:"?"; }
const char *upload_status_str(UploadStatus s){ return (s>=0&&s<UPSTAT__COUNT)?_up_stat[s]:"?"; }
const char *task_status_str(TaskStatus s)   { return (s>=0&&s<TSTAT__COUNT)?_task_stat[s]:"?"; }

/* ── Column/index helpers ───────────────────────────────── */

static void col(ColDef *c, const char *name, ColType type, uint32_t size,
                bool not_null, bool is_pk, bool auto_inc) {
    memset(c, 0, sizeof(ColDef));
    strncpy(c->name, name, MAX_COL_NAME-1);
    c->type = type; c->size = size;
    c->not_null = not_null; c->is_primary = is_pk; c->auto_inc = auto_inc;
}

static void idx(IndexDef *ix, const char *name, IndexType type,
                uint32_t c0, uint32_t c1, uint32_t c2, uint32_t c3) {
    memset(ix, 0, sizeof(IndexDef));
    strncpy(ix->name, name, MAX_COL_NAME-1);
    ix->type = type; ix->num_cols = 0;
    uint32_t cols[] = {c0,c1,c2,c3};
    for (int i=0; i<4; i++)
        if (cols[i] != (uint32_t)-1)
            ix->col_indices[ix->num_cols++] = cols[i];
}

#define NC ((uint32_t)-1)

/* ── CHECK constraints ──────────────────────────────────── */

static bool chk_system(const void *r, uint32_t sz) {
    (void)sz; const System *s = r;
    if (s->name[0]=='\0') return false;
    if (s->system_type<0 || s->system_type>=SYS__COUNT) return false;
    if (s->status<0 || s->status>=SSTAT__COUNT) return false;
    if (s->cooling<0 || s->cooling>=COOL__COUNT) return false;
    if (s->chamber_count<1) return false;
    if (s->shelves_per_chamber<1) return false;
    if (s->slots_per_shelf<1) return false;
    return true;
}

static bool chk_location(const void *r, uint32_t sz) {
    (void)sz; const Location *l = r;
    if (l->name[0]=='\0') return false;
    if (l->loc_type<0 || l->loc_type>=LOC__COUNT) return false;
    if (l->status<0 || l->status>=LSTAT__COUNT) return false;
    return true;
}

static bool chk_device(const void *r, uint32_t sz) {
    (void)sz; const Device *d = r;
    if (d->customer[0]=='\0') return false;
    if (d->device_name[0]=='\0') return false;
    return true;
}

static bool chk_project(const void *r, uint32_t sz) {
    (void)sz; const Project *p = r;
    if (p->project_number[0]=='\0') return false;
    if (p->device_id<=0) return false;
    if (p->status<0 || p->status>=PROJ__COUNT) return false;
    if (p->cooling<0 || p->cooling>=COOL__COUNT) return false;
    return true;
}

static bool chk_lot(const void *r, uint32_t sz) {
    (void)sz; const Lot *l = r;
    if (l->project_id<=0) return false;
    if (l->lot_number[0]=='\0') return false;
    if (l->step<0 || l->step>=LOT_STEP__COUNT) return false;
    if (l->lot_status<0 || l->lot_status>=LSTAT_LOT__COUNT) return false;
    return true;
}

static bool chk_hw_type(const void *r, uint32_t sz) {
    (void)sz; const HardwareType *h = r;
    if (h->name[0]=='\0') return false;
    if (h->category<0 || h->category>=HW__COUNT) return false;
    if (h->tracking<0 || h->tracking>=TRACK__COUNT) return false;
    return true;
}

static bool chk_serialized(const void *r, uint32_t sz) {
    (void)sz; const SerializedHw *s = r;
    if (s->type_id<=0) return false;
    if (s->serial_no[0]=='\0') return false;
    if (s->status<0 || s->status>=ITEM__COUNT) return false;
    return true;
}

static bool chk_quantity(const void *r, uint32_t sz) {
    (void)sz; const QuantityHw *q = r;
    if (q->type_id<=0) return false;
    if (q->system_id<=0) return false;
    if (q->total<0 || q->good<0 || q->bad<0) return false;
    if (q->good + q->bad > q->total) return false;
    return true;
}

static bool chk_configured(const void *r, uint32_t sz) {
    (void)sz; const ConfiguredHw *c = r;
    if (c->type_id<=0) return false;
    if (c->system_id<=0) return false;
    if (c->project_id<=0) return false;
    if (c->quantity<1) return false;
    return true;
}

static bool chk_audit(const void *r, uint32_t sz) {
    (void)sz; const AuditEntry *a = r;
    if (a->user_id<=0) return false;
    if (a->action<0 || a->action>=AUDIT__COUNT) return false;
    return true;
}

static bool chk_user(const void *r, uint32_t sz) {
    (void)sz; const User *u = r;
    if (u->username[0]=='\0') return false;
    if (u->display_name[0]=='\0') return false;
    if (u->password_hash[0]=='\0') return false;
    if (u->role<0 || u->role>=ROLE__COUNT) return false;
    return true;
}

/* ── Tracker CHECK constraints ─────────────────────────── */

static bool chk_team_member(const void *r, uint32_t sz) {
    (void)sz; const TeamMember *t = r;
    if (t->name[0]=='\0') return false;
    return true;
}

static bool chk_daily_activity(const void *r, uint32_t sz) {
    (void)sz; const DailyActivity *a = r;
    if (a->date[0]=='\0') return false;
    return true;
}

static bool chk_upload_task(const void *r, uint32_t sz) {
    (void)sz; const UploadTask *t = r;
    if (t->activity_id<=0) return false;
    if (t->status<0 || t->status>=UPSTAT__COUNT) return false;
    return true;
}

static bool chk_download_task(const void *r, uint32_t sz) {
    (void)sz; const DownloadTask *t = r;
    if (t->activity_id<=0) return false;
    if (t->status<0 || t->status>=TSTAT__COUNT) return false;
    return true;
}

static bool chk_eng_activity_task(const void *r, uint32_t sz) {
    (void)sz; const EngActivityTask *t = r;
    if (t->activity_id<=0) return false;
    if (t->status<0 || t->status>=TSTAT__COUNT) return false;
    return true;
}

static bool chk_eng_hours(const void *r, uint32_t sz) {
    (void)sz; const EngHoursEntry *e = r;
    if (e->date[0]=='\0') return false;
    if (e->hours_hundredths<0) return false;
    return true;
}

/* ── V1 Compat CHECK constraints ──────────────────────── */

static bool chk_v1_board(const void *r, uint32_t sz) {
    (void)sz; const V1Board *b = r;
    if (b->customer[0]=='\0') return false;
    if (b->serial_no[0]=='\0') return false;
    return true;
}

static bool chk_v1_board_type(const void *r, uint32_t sz) {
    (void)sz; const V1BoardType *b = r;
    if (b->customer[0]=='\0') return false;
    if (b->pcb_number_text[0]=='\0') return false;
    return true;
}

static bool chk_v1_board_log(const void *r, uint32_t sz) {
    (void)sz; const V1BoardLog *l = r;
    if (l->board_id<=0) return false;
    return true;
}

static bool chk_v1_socket_note(const void *r, uint32_t sz) {
    (void)sz; const V1SocketNote *s = r;
    if (s->board_id<=0) return false;
    return true;
}

/* ── Schema Init ────────────────────────────────────────── */

int schema_init(Database *db) {
    int rc;
    TableDef t;

    /* ── 1. systems ─────────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "systems", MAX_TABLE_NAME);
    t.num_cols = 10;
    col(&t.cols[0], "system_id",           COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "name",                COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2], "system_type",         COL_INT32, 4, 1,0,0);
    col(&t.cols[3], "status",              COL_INT32, 4, 1,0,0);
    col(&t.cols[4], "cooling",             COL_INT32, 4, 1,0,0);
    col(&t.cols[5], "ip_base",             COL_TEXT, 64, 0,0,0);
    col(&t.cols[6], "chamber_count",       COL_INT32, 4, 1,0,0);
    col(&t.cols[7], "shelves_per_chamber", COL_INT32, 4, 1,0,0);
    col(&t.cols[8], "slots_per_shelf",     COL_INT32, 4, 1,0,0);
    col(&t.cols[9], "notes",               COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_sys",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_sys_name",  IDX_UNIQUE,  1, NC,NC,NC);
    idx(&t.indexes[2], "idx_sys_type", IDX_NORMAL,  2, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_system;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 2. locations ───────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "locations", MAX_TABLE_NAME);
    t.num_cols = 9;
    col(&t.cols[0], "location_id", COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "system_id",   COL_INT64, 8, 0,0,0);
    col(&t.cols[2], "parent_id",   COL_INT64, 8, 0,0,0);
    col(&t.cols[3], "name",        COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[4], "loc_type",    COL_INT32, 4, 1,0,0);
    col(&t.cols[5], "position",    COL_INT32, 4, 0,0,0);
    col(&t.cols[6], "status",      COL_INT32, 4, 1,0,0);
    col(&t.cols[7], "path_cache",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[8], "notes",       COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_loc",        IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_loc_sys",   IDX_NORMAL,  1, NC,NC,NC);
    idx(&t.indexes[2], "idx_loc_parent",IDX_NORMAL,  2, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_location;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 3. devices ─────────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "devices", MAX_TABLE_NAME);
    t.num_cols = 6;
    col(&t.cols[0], "device_id",      COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "customer",       COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2], "device_name",    COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[3], "device_number",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4], "device_family",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5], "package_type",   COL_TEXT, 64, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_dev",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_dev_name",  IDX_UNIQUE,  1, 2, NC,NC); /* customer+device_name */
    idx(&t.indexes[2], "idx_dev_cust", IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_device;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 4. projects ────────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "projects", MAX_TABLE_NAME);
    t.num_cols = 9;
    col(&t.cols[0], "project_id",     COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "device_id",      COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "project_number", COL_TEXT, 64, 1,0,0);
    col(&t.cols[3], "system_id",      COL_INT64, 8, 0,0,0);
    col(&t.cols[4], "status",         COL_INT32, 4, 1,0,0);
    col(&t.cols[5], "cooling",        COL_INT32, 4, 1,0,0);
    col(&t.cols[6], "start_date_ms",  COL_INT64, 8, 0,0,0);
    col(&t.cols[7], "end_date_ms",    COL_INT64, 8, 0,0,0);
    col(&t.cols[8], "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 4;
    idx(&t.indexes[0], "pk_proj",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_proj_num",  IDX_UNIQUE,  2, NC,NC,NC);
    idx(&t.indexes[2], "idx_proj_dev", IDX_NORMAL,  1, NC,NC,NC);
    idx(&t.indexes[3], "idx_proj_sys", IDX_NORMAL,  3, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_project;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 5. lots ─────────────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "lots", MAX_TABLE_NAME);
    t.num_cols = 15;
    col(&t.cols[0],  "lot_id",        COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "project_id",    COL_INT64, 8, 1,0,0);
    col(&t.cols[2],  "system_id",     COL_INT64, 8, 0,0,0);
    col(&t.cols[3],  "lot_number",    COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[4],  "customer_lot",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5],  "step",          COL_INT32, 4, 1,0,0);
    col(&t.cols[6],  "lot_status",    COL_INT32, 4, 1,0,0);
    col(&t.cols[7],  "expected_qty",  COL_INT32, 4, 0,0,0);
    col(&t.cols[8],  "running_qty",   COL_INT32, 4, 0,0,0);
    col(&t.cols[9],  "good",          COL_INT32, 4, 0,0,0);
    col(&t.cols[10], "reject",        COL_INT32, 4, 0,0,0);
    col(&t.cols[11], "missing",       COL_INT32, 4, 0,0,0);
    col(&t.cols[12], "received_ms",   COL_INT64, 8, 0,0,0);
    col(&t.cols[13], "started_ms",    COL_INT64, 8, 0,0,0);
    col(&t.cols[14], "completed_ms",  COL_INT64, 8, 0,0,0);
    t.num_indexes = 4;
    idx(&t.indexes[0], "pk_lot",        IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_lot_num",   IDX_UNIQUE,  3, NC,NC,NC);
    idx(&t.indexes[2], "idx_lot_proj",  IDX_NORMAL,  1, NC,NC,NC);
    idx(&t.indexes[3], "idx_lot_sys",   IDX_NORMAL,  2, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_lot;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 6. hardware_types ──────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "hardware_types", MAX_TABLE_NAME);
    t.num_cols = 6;
    col(&t.cols[0], "type_id",         COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "name",            COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2], "category",        COL_INT32, 4, 1,0,0);
    col(&t.cols[3], "tracking",        COL_INT32, 4, 1,0,0);
    col(&t.cols[4], "for_system_type", COL_INT32, 4, 0,0,0);
    col(&t.cols[5], "notes",           COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_hwtype",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_hwtype_cat", IDX_NORMAL,  2, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_hw_type;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 7. serialized_hw ───────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "serialized_hw", MAX_TABLE_NAME);
    t.num_cols = 12;
    col(&t.cols[0],  "item_id",        COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "type_id",        COL_INT64, 8, 1,0,0);
    col(&t.cols[2],  "serial_no",      COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[3],  "system_id",      COL_INT64, 8, 0,0,0);
    col(&t.cols[4],  "location_id",    COL_INT64, 8, 0,0,0);
    col(&t.cols[5],  "project_id",     COL_INT64, 8, 0,0,0);
    col(&t.cols[6],  "status",         COL_INT32, 4, 1,0,0);
    col(&t.cols[7],  "date_created_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[8],  "last_moved_ms",  COL_INT64, 8, 0,0,0);
    col(&t.cols[9],  "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[10], "socket_mask",    COL_TEXT, 16, 0,0,0);
    col(&t.cols[11], "socket_count",   COL_INT32, 4, 0,0,0);
    t.num_indexes = 6;
    idx(&t.indexes[0], "pk_ser",        IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_serial",     IDX_UNIQUE,  2, NC,NC,NC);
    idx(&t.indexes[2], "idx_ser_sys",   IDX_NORMAL,  3, NC,NC,NC);
    idx(&t.indexes[3], "idx_ser_loc",   IDX_NORMAL,  4, NC,NC,NC);
    idx(&t.indexes[4], "idx_ser_proj",  IDX_NORMAL,  5, NC,NC,NC);
    idx(&t.indexes[5], "idx_ser_stat",  IDX_NORMAL,  6, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_serialized;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 8. quantity_hw ─────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "quantity_hw", MAX_TABLE_NAME);
    t.num_cols = 10;
    col(&t.cols[0], "qty_id",         COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "type_id",        COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "system_id",      COL_INT64, 8, 1,0,0);
    col(&t.cols[3], "location_id",    COL_INT64, 8, 0,0,0);
    col(&t.cols[4], "project_id",     COL_INT64, 8, 0,0,0);
    col(&t.cols[5], "total",          COL_INT32, 4, 1,0,0);
    col(&t.cols[6], "good",           COL_INT32, 4, 1,0,0);
    col(&t.cols[7], "bad",            COL_INT32, 4, 1,0,0);
    col(&t.cols[8], "last_updated_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[9], "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_qty",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_qty_sys",  IDX_NORMAL,  2, NC,NC,NC);
    idx(&t.indexes[2], "uq_qty_loc",   IDX_UNIQUE,  1, 2, 3, NC);
    t.num_checks = 1; t.checks[0] = chk_quantity;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 9. configured_hw ───────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "configured_hw", MAX_TABLE_NAME);
    t.num_cols = 12;
    col(&t.cols[0],  "config_id",      COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "type_id",        COL_INT64, 8, 1,0,0);
    col(&t.cols[2],  "system_id",      COL_INT64, 8, 1,0,0);
    col(&t.cols[3],  "location_id",    COL_INT64, 8, 0,0,0);
    col(&t.cols[4],  "project_id",     COL_INT64, 8, 1,0,0);
    col(&t.cols[5],  "r0_ohms",        COL_INT32, 4, 0,0,0);
    col(&t.cols[6],  "r4_ohms",        COL_INT32, 4, 0,0,0);
    col(&t.cols[7],  "vout_mv",        COL_INT32, 4, 0,0,0);
    col(&t.cols[8],  "role",           COL_INT32, 4, 0,0,0);
    col(&t.cols[9],  "quantity",       COL_INT32, 4, 1,0,0);
    col(&t.cols[10], "last_updated_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[11], "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_cfg",        IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_cfg_proj",  IDX_NORMAL,  4, NC,NC,NC);
    idx(&t.indexes[2], "idx_cfg_sys",   IDX_NORMAL,  2, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_configured;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 10. audit_log ───────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "audit_log", MAX_TABLE_NAME);
    t.num_cols = 7;
    col(&t.cols[0], "log_id",       COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "user_id",      COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "action",       COL_INT32, 4, 1,0,0);
    col(&t.cols[3], "entity_table", COL_TEXT, 64, 1,0,0);
    col(&t.cols[4], "entity_id",    COL_INT64, 8, 1,0,0);
    col(&t.cols[5], "timestamp_ms", COL_INT64, 8, 1,0,0);
    col(&t.cols[6], "detail",       COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_audit",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_audit_entity",IDX_NORMAL, 3, 4, NC,NC);
    idx(&t.indexes[2], "idx_audit_time", IDX_NORMAL,  5, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_audit;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 11. users ───────────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "users", MAX_TABLE_NAME);
    t.num_cols = 8;
    col(&t.cols[0], "user_id",       COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "username",      COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2], "display_name",  COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[3], "password_hash", COL_TEXT, HASH_LEN, 1,0,0);
    col(&t.cols[4], "role",          COL_INT32, 4, 1,0,0);
    col(&t.cols[5], "active",        COL_INT32, 4, 0,0,0);
    col(&t.cols[6], "created_ms",    COL_INT64, 8, 0,0,0);
    col(&t.cols[7], "last_login_ms", COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_user",     IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_username", IDX_UNIQUE,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_user;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 12. team_members ────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "team_members", MAX_TABLE_NAME);
    t.num_cols = 5;
    col(&t.cols[0], "team_member_id",  COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "name",            COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2], "role",            COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[3], "primary_systems", COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4], "board_patterns",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_tm",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_tm_name", IDX_UNIQUE,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_team_member;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 13. daily_activities ──────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "daily_activities", MAX_TABLE_NAME);
    t.num_cols = 3;
    col(&t.cols[0], "activity_id",  COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "date",         COL_TEXT, 16, 1,0,0);
    col(&t.cols[2], "created_at_ms",COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_act",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "uq_act_date", IDX_UNIQUE,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_daily_activity;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 14. upload_tasks ──────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "upload_tasks", MAX_TABLE_NAME);
    t.num_cols = 14;
    col(&t.cols[0],  "upload_id",      COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "activity_id",    COL_INT64, 8, 1,0,0);
    col(&t.cols[2],  "load_date",      COL_TEXT, 16, 0,0,0);
    col(&t.cols[3],  "customer",       COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4],  "lot",            COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5],  "ise_id",         COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[6],  "qty",            COL_INT32, 4, 0,0,0);
    col(&t.cols[7],  "device",         COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[8],  "time_at_lab",    COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[9],  "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[10], "status",         COL_INT32, 4, 1,0,0);
    col(&t.cols[11], "assigned_to",    COL_INT64, 8, 0,0,0);
    col(&t.cols[12], "completed_at_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[13], "created_at_ms",  COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_upload",     IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_upload_act",IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_upload_task;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 15. download_tasks ────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "download_tasks", MAX_TABLE_NAME);
    t.num_cols = 13;
    col(&t.cols[0],  "download_id",    COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "activity_id",    COL_INT64, 8, 1,0,0);
    col(&t.cols[2],  "customer",       COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[3],  "lot",            COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4],  "ise_id",         COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5],  "qty",            COL_INT32, 4, 0,0,0);
    col(&t.cols[6],  "device",         COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[7],  "download_time",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[8],  "notes",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[9],  "status",         COL_INT32, 4, 1,0,0);
    col(&t.cols[10], "assigned_to",    COL_INT64, 8, 0,0,0);
    col(&t.cols[11], "completed_at_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[12], "created_at_ms",  COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_download",     IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_download_act",IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_download_task;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 16. eng_activity_tasks ────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "eng_activity_tasks", MAX_TABLE_NAME);
    t.num_cols = 10;
    col(&t.cols[0], "eng_task_id",   COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "activity_id",   COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "customer",      COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[3], "device",        COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4], "description",   COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5], "ise_numbers",   COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[6], "status",        COL_INT32, 4, 1,0,0);
    col(&t.cols[7], "assigned_to",   COL_INT64, 8, 0,0,0);
    col(&t.cols[8], "completed_at_ms",COL_INT64, 8, 0,0,0);
    col(&t.cols[9], "created_at_ms", COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_eng_task",     IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_eng_task_act",IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_eng_activity_task;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 17. engineering_hours ─────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "engineering_hours", MAX_TABLE_NAME);
    t.num_cols = 13;
    col(&t.cols[0],  "entry_id",               COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "date",                   COL_TEXT, 16, 1,0,0);
    col(&t.cols[2],  "customer",               COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[3],  "project",                COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4],  "pcb_number",             COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5],  "description",            COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[6],  "engineer",               COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[7],  "hours_hundredths",       COL_INT32, 4, 0,0,0);
    col(&t.cols[8],  "billable",               COL_INT32, 4, 0,0,0);
    col(&t.cols[9],  "quoted_hours_hundredths",COL_INT32, 4, 0,0,0);
    col(&t.cols[10], "po_number",              COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[11], "source_task_id",         COL_INT64, 8, 0,0,0);
    col(&t.cols[12], "created_at_ms",          COL_INT64, 8, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_eng_hours",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_eng_hours_date",IDX_NORMAL,  1, NC,NC,NC);
    idx(&t.indexes[2], "idx_eng_hours_eng", IDX_NORMAL,  6, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_eng_hours;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 18. v1_boards ──────────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "v1_boards", MAX_TABLE_NAME);
    t.num_cols = 18;
    col(&t.cols[0],  "board_id",          COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "customer",          COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2],  "platform",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[3],  "pcb_number_text",   COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4],  "revision",          COL_TEXT, 64, 0,0,0);
    col(&t.cols[5],  "serial_no",         COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[6],  "power_qty",         COL_INT32, 4, 0,0,0);
    col(&t.cols[7],  "status",            COL_TEXT, 64, 0,0,0);
    col(&t.cols[8],  "location_id",       COL_INT64, 8, 0,0,0);
    col(&t.cols[9],  "socket_rows",       COL_INT32, 4, 0,0,0);
    col(&t.cols[10], "socket_cols",       COL_INT32, 4, 0,0,0);
    col(&t.cols[11], "notes",             COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[12], "individual_notes",  COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[13], "date_created",      COL_TEXT, 32, 0,0,0);
    col(&t.cols[14], "last_used_date",    COL_TEXT, 32, 0,0,0);
    col(&t.cols[15], "sockets_working",   COL_INT32, 4, 0,0,0);
    col(&t.cols[16], "sockets_bad",       COL_INT32, 4, 0,0,0);
    col(&t.cols[17], "sockets_not_installed", COL_INT32, 4, 0,0,0);
    t.num_indexes = 3;
    idx(&t.indexes[0], "pk_v1board",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_v1board_cust", IDX_NORMAL,  1, NC,NC,NC);
    idx(&t.indexes[2], "idx_v1board_loc",  IDX_NORMAL,  8, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_v1_board;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 19. v1_board_types ──────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "v1_board_types", MAX_TABLE_NAME);
    t.num_cols = 11;
    col(&t.cols[0],  "board_type_id",    COL_INT64, 8, 1,1,1);
    col(&t.cols[1],  "customer",         COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[2],  "pcb_number_text",  COL_TEXT, MAX_TEXT_LEN, 1,0,0);
    col(&t.cols[3],  "revision",         COL_TEXT, 64, 0,0,0);
    col(&t.cols[4],  "platform",         COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5],  "power_qty",        COL_INT32, 4, 0,0,0);
    col(&t.cols[6],  "socket_rows",      COL_INT32, 4, 0,0,0);
    col(&t.cols[7],  "socket_cols",      COL_INT32, 4, 0,0,0);
    col(&t.cols[8],  "notes",            COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[9],  "is_default",       COL_INT32, 4, 0,0,0);
    col(&t.cols[10], "devices",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_v1bt",       IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_v1bt_cust", IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_v1_board_type;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 20. v1_board_logs ──────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "v1_board_logs", MAX_TABLE_NAME);
    t.num_cols = 8;
    col(&t.cols[0], "log_id",           COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "board_id",         COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "timestamp",        COL_TEXT, 32, 0,0,0);
    col(&t.cols[3], "user",             COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[4], "action",           COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[5], "details",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    col(&t.cols[6], "from_location_id", COL_INT64, 8, 0,0,0);
    col(&t.cols[7], "to_location_id",   COL_INT64, 8, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_v1blog",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_v1blog_bid", IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_v1_board_log;
    rc = table_register(db, &t); if (rc) return rc;

    /* ── 21. v1_socket_notes ──────────────────────────────── */
    memset(&t, 0, sizeof(t));
    strncpy(t.name, "v1_socket_notes", MAX_TABLE_NAME);
    t.num_cols = 5;
    col(&t.cols[0], "note_id",       COL_INT64, 8, 1,1,1);
    col(&t.cols[1], "board_id",      COL_INT64, 8, 1,0,0);
    col(&t.cols[2], "socket_number", COL_INT32, 4, 0,0,0);
    col(&t.cols[3], "status",        COL_TEXT, 64, 0,0,0);
    col(&t.cols[4], "note",          COL_TEXT, MAX_TEXT_LEN, 0,0,0);
    t.num_indexes = 2;
    idx(&t.indexes[0], "pk_v1sock",      IDX_PRIMARY, 0, NC,NC,NC);
    idx(&t.indexes[1], "idx_v1sock_bid", IDX_NORMAL,  1, NC,NC,NC);
    t.num_checks = 1; t.checks[0] = chk_v1_socket_note;
    rc = table_register(db, &t); if (rc) return rc;

    printf("[schema] Registered %u tables (v5)\n", db->num_tables);
    return LRM_OK;
}
