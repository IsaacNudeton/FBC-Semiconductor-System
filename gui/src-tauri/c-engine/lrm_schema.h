/*
 * lrm_schema.h — Burn-In Lab Inventory Schema v3
 *
 * Models the REAL ISE Labs domain:
 *   Systems  → physical machines (HX, Sonoma, XP-160, MCC, Shasta)
 *   Devices  → customer+chip pair (Marvell Iliad, Cisco C512)
 *   Projects → what's running where (S00## / B#### numbers)
 *   Lots     → wafer batches flowing through burn-in (the spine)
 *   Hardware → three tracking patterns:
 *     Serialized: BIMs, BIBs, Boards, Powertrains (individual serial)
 *     Quantity:   Temp cards, controllers, neg supply (count good/bad)
 *     Configured: LCPS, HCPS (address resistors), Cores (master/slave)
 *
 * Naming by system type:
 *   Sonoma/MCC → BIMs    HX/XP-160/Shasta → BIBs    Generic → Boards
 *
 * Key invariant: Device IS the customer-device pair. No separate customers table.
 * Everything flows through the LOT.
 */

#ifndef LRM_SCHEMA_H
#define LRM_SCHEMA_H

#include "lrm_db.h"

/* ── Enums ──────────────────────────────────────────────── */

typedef enum {
    SYS_HX=0, SYS_SONOMA=1, SYS_XP160=2, SYS_MCC=3, SYS_SHASTA=4,
    SYS__COUNT=5
} SystemType;

typedef enum {
    SSTAT_RUNNING=0, SSTAT_FREE=1, SSTAT_DOWN=2, SSTAT_MAINTENANCE=3,
    SSTAT__COUNT=4
} SystemStatus;

typedef enum {
    COOL_AIR=0, COOL_LIQUID=1, COOL_NONE=2, COOL__COUNT=3
} CoolingType;

typedef enum {
    LOC_BAY=0, LOC_CHAMBER=1, LOC_SHELF=2, LOC_TRAY=3,
    LOC_SLOT=4, LOC_BACKPLANE=5, LOC_STORAGE=6, LOC__COUNT=7
} LocType;

typedef enum {
    LSTAT_ACTIVE=0, LSTAT_DOWN=1, LSTAT_INACTIVE=2, LSTAT__COUNT=3
} LocStatus;

typedef enum {
    PROJ_ACTIVE=0, PROJ_COMPLETE=1, PROJ_HOLD=2, PROJ_CANCELLED=3,
    PROJ__COUNT=4
} ProjectStatus;

typedef enum {
    HW_BIM=0, HW_BIB=1, HW_BOARD=2, HW_POWERTRAIN=3,
    HW_LCPS=4, HW_HCPS=5, HW_CORE=6, HW_TEMP_DIODE=7,
    HW_TEMP_CIRCUIT=8, HW_NEG_SUPPLY=9, HW_CONTROLLER=10,
    HW__COUNT=11
} HwCategory;

typedef enum {
    TRACK_SERIALIZED=0, TRACK_QUANTITY=1, TRACK_CONFIGURED=2,
    TRACK__COUNT=3
} TrackingMode;

typedef enum {
    ITEM_AVAILABLE=0, ITEM_IN_USE=1, ITEM_DAMAGED=2,
    ITEM_MAINTENANCE=3, ITEM_RETIRED=4, ITEM__COUNT=5
} ItemStatus;

typedef enum { CORE_MASTER=0, CORE_SLAVE=1, CORE__COUNT=2 } CoreRole;

/* Socket status (2-bit per socket in bitmask) */
typedef enum {
    SOCK_WORKING=0, SOCK_BAD=1, SOCK_NOT_INSTALLED=2, SOCK_RESERVED=3,
    SOCK__COUNT=4
} SocketStatus;

typedef enum {
    AUDIT_CREATE=0, AUDIT_UPDATE=1, AUDIT_DELETE=2, AUDIT_MOVE=3,
    AUDIT_STATUS=4, AUDIT_ASSIGN=5, AUDIT_UNASSIGN=6, AUDIT_QTY=7,
    AUDIT__COUNT=8
} AuditAction;

typedef enum {
    ROLE_OPERATOR=0, ROLE_TECH=1, ROLE_ENGINEER=2, ROLE_ADMIN=3,
    ROLE__COUNT=4
} UserRole;

/* Lot workflow steps — the burn-in lifecycle */
typedef enum {
    LOT_RECEIVED=0, LOT_SETUP=1, LOT_LOADING=2, LOT_BURN_IN=3,
    LOT_READPOINT=4, LOT_UNLOADING=5, LOT_SHIPPING=6, LOT_COMPLETE=7,
    LOT_STEP__COUNT=8
} LotStep;

/* Lot status */
typedef enum {
    LSTAT_LOT_ACTIVE=0, LSTAT_LOT_HOLD=1, LSTAT_LOT_COMPLETE=2,
    LSTAT_LOT_CANCELLED=3, LSTAT_LOT__COUNT=4
} LotStatus;

/* ── Tracker Enums ─────────────────────────────────────── */

typedef enum {
    UPSTAT_NOT_LOADED=0, UPSTAT_LOADING=1, UPSTAT_LOADED=2, UPSTAT_ERROR=3,
    UPSTAT__COUNT=4
} UploadStatus;

typedef enum {
    TSTAT_PENDING=0, TSTAT_IN_PROGRESS=1, TSTAT_COMPLETED=2, TSTAT_BLOCKED=3,
    TSTAT__COUNT=4
} TaskStatus;

/* ── Enum-to-string ─────────────────────────────────────── */
const char *system_type_str(SystemType s);
const char *system_status_str(SystemStatus s);
const char *cooling_str(CoolingType c);
const char *loc_type_str(LocType t);
const char *loc_status_str(LocStatus s);
const char *project_status_str(ProjectStatus s);
const char *hw_category_str(HwCategory c);
const char *tracking_mode_str(TrackingMode m);
const char *item_status_str(ItemStatus s);
const char *core_role_str(CoreRole r);
const char *audit_action_str(AuditAction a);
const char *user_role_str(UserRole r);
const char *lot_step_str(LotStep s);
const char *lot_status_str(LotStatus s);
const char *socket_status_str(SocketStatus s);
const char *upload_status_str(UploadStatus s);
const char *task_status_str(TaskStatus s);

/* MSVC compat: __attribute__((packed)) → #pragma pack */
#ifdef _MSC_VER
  #define LRM_PACKED
  #pragma pack(push, 1)
#else
  #define LRM_PACKED __attribute__((packed))
#endif

/* ── Table 1: systems ───────────────────────────────────── */
typedef struct {
    int64_t  system_id;
    char     name[MAX_TEXT_LEN];            /* UNIQUE */
    int32_t  system_type;                   /* SystemType */
    int32_t  status;                        /* SystemStatus */
    int32_t  cooling;                       /* CoolingType */
    char     ip_base[64];                   /* "172.16" for Sonoma */
    int32_t  chamber_count;                 /* >= 1 */
    int32_t  shelves_per_chamber;           /* Sonoma:10-11, MCC:13 */
    int32_t  slots_per_shelf;               /* Sonoma:2(F/B), MCC:4 */
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED System;

/* ── Table 2: locations ─────────────────────────────────── */
typedef struct {
    int64_t  location_id;
    int64_t  system_id;                     /* FK→systems, 0=standalone */
    int64_t  parent_id;                     /* FK→locations, 0=root */
    char     name[MAX_TEXT_LEN];
    int32_t  loc_type;                      /* LocType */
    int32_t  position;                      /* numeric position */
    int32_t  status;                        /* LocStatus */
    char     path_cache[MAX_TEXT_LEN];
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED Location;

/* ── Table 3: devices ───────────────────────────────────── */
/* The customer+device pair IS the atomic identity.
 * No separate customers table — device IS the customer-device pair. */
typedef struct {
    int64_t  device_id;
    char     customer[MAX_TEXT_LEN];        /* "Marvell", "Cisco" */
    char     device_name[MAX_TEXT_LEN];     /* "Iliad", "C512" — Everest folder name */
    char     device_number[MAX_TEXT_LEN];   /* B#### board design number */
    char     device_family[MAX_TEXT_LEN];   /* optional grouping */
    char     package_type[64];              /* BGA, QFP, etc */
} LRM_PACKED Device;

/* ── Table 4: projects ──────────────────────────────────── */
typedef struct {
    int64_t  project_id;
    int64_t  device_id;                     /* FK→devices */
    char     project_number[64];            /* UNIQUE "S0026" or "B1822" */
    int64_t  system_id;                     /* FK→systems */
    int32_t  status;                        /* ProjectStatus */
    int32_t  cooling;                       /* CoolingType */
    int64_t  start_date_ms;
    int64_t  end_date_ms;                   /* 0=ongoing */
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED Project;

/* ── Table 5: lots ──────────────────────────────────────── */
/* The spine — every test run IS a lot flowing through the workflow */
typedef struct {
    int64_t  lot_id;
    int64_t  project_id;                    /* FK→projects */
    int64_t  system_id;                     /* FK→systems, 0=not assigned */
    char     lot_number[MAX_TEXT_LEN];      /* L######## */
    char     customer_lot[MAX_TEXT_LEN];    /* customer's internal lot ID */
    int32_t  step;                          /* LotStep — workflow position */
    int32_t  lot_status;                    /* LotStatus */
    int32_t  expected_qty;                  /* DUTs expected */
    int32_t  running_qty;                   /* DUTs actually loaded */
    int32_t  good;
    int32_t  reject;
    int32_t  missing;
    int64_t  received_ms;
    int64_t  started_ms;
    int64_t  completed_ms;
} LRM_PACKED Lot;

/* ── Table 6: hardware_types ────────────────────────────── */
typedef struct {
    int64_t  type_id;
    char     name[MAX_TEXT_LEN];
    int32_t  category;                      /* HwCategory */
    int32_t  tracking;                      /* TrackingMode */
    int32_t  for_system_type;               /* SystemType, -1=any */
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED HardwareType;

/* ── Table 7: serialized_hw ─────────────────────────────── */
typedef struct {
    int64_t  item_id;
    int64_t  type_id;                       /* FK→hardware_types */
    char     serial_no[MAX_TEXT_LEN];       /* UNIQUE */
    int64_t  system_id;                     /* FK→systems, 0=not installed */
    int64_t  location_id;                   /* FK→locations, 0=unassigned */
    int64_t  project_id;                    /* FK→projects, 0=unassigned */
    int32_t  status;                        /* ItemStatus */
    int64_t  date_created_ms;
    int64_t  last_moved_ms;
    char     notes[MAX_TEXT_LEN];
    uint8_t  socket_mask[16];           /* 2-bit per socket, 64 sockets max */
    int32_t  socket_count;              /* how many sockets on this board */
} LRM_PACKED SerializedHw;

/* ── Table 8: quantity_hw ───────────────────────────────── */
typedef struct {
    int64_t  qty_id;
    int64_t  type_id;                       /* FK→hardware_types */
    int64_t  system_id;                     /* FK→systems */
    int64_t  location_id;                   /* FK→locations, 0=system-level */
    int64_t  project_id;                    /* FK→projects, 0=unassigned */
    int32_t  total;
    int32_t  good;
    int32_t  bad;
    int64_t  last_updated_ms;
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED QuantityHw;

/* ── Table 9: configured_hw ─────────────────────────────── */
typedef struct {
    int64_t  config_id;
    int64_t  type_id;                       /* FK→hardware_types */
    int64_t  system_id;                     /* FK→systems */
    int64_t  location_id;                   /* FK→locations */
    int64_t  project_id;                    /* FK→projects */
    int32_t  r0_ohms;                       /* LCPS/HCPS address R0 */
    int32_t  r4_ohms;                       /* HCPS address R4 */
    int32_t  vout_mv;                       /* target mV */
    int32_t  role;                          /* CoreRole, -1 if N/A */
    int32_t  quantity;
    int64_t  last_updated_ms;
    char     notes[MAX_TEXT_LEN];
} LRM_PACKED ConfiguredHw;

/* ── Table 10: audit_log ────────────────────────────────── */
typedef struct {
    int64_t  log_id;
    int64_t  user_id;                       /* FK→users */
    int32_t  action;                        /* AuditAction */
    char     entity_table[64];
    int64_t  entity_id;
    int64_t  timestamp_ms;
    char     detail[MAX_TEXT_LEN];
} LRM_PACKED AuditEntry;

/* ── Table 11: users ────────────────────────────────────── */
#define HASH_LEN 128
typedef struct {
    int64_t  user_id;
    char     username[MAX_TEXT_LEN];        /* UNIQUE */
    char     display_name[MAX_TEXT_LEN];
    char     password_hash[HASH_LEN];
    int32_t  role;                          /* UserRole */
    int32_t  active;
    int64_t  created_ms;
    int64_t  last_login_ms;
} LRM_PACKED User;

/* ── Table 12: team_members ─────────────────────────────── */
typedef struct {
    int64_t  team_member_id;
    char     name[MAX_TEXT_LEN];
    char     role[MAX_TEXT_LEN];
    char     primary_systems[MAX_TEXT_LEN];
    char     board_patterns[MAX_TEXT_LEN];
} LRM_PACKED TeamMember;

/* ── Table 13: daily_activities ────────────────────────── */
typedef struct {
    int64_t  activity_id;
    char     date[16];                     /* "2026-03-06" UNIQUE */
    int64_t  created_at_ms;
} LRM_PACKED DailyActivity;

/* ── Table 14: upload_tasks ────────────────────────────── */
typedef struct {
    int64_t  upload_id;
    int64_t  activity_id;                  /* FK→daily_activities */
    char     load_date[16];
    char     customer[MAX_TEXT_LEN];
    char     lot[MAX_TEXT_LEN];
    char     ise_id[MAX_TEXT_LEN];
    int32_t  qty;
    char     device[MAX_TEXT_LEN];
    char     time_at_lab[MAX_TEXT_LEN];
    char     notes[MAX_TEXT_LEN];
    int32_t  status;                       /* UploadStatus */
    int64_t  assigned_to;                  /* FK→team_members, 0=unassigned */
    int64_t  completed_at_ms;
    int64_t  created_at_ms;
} LRM_PACKED UploadTask;

/* ── Table 15: download_tasks ──────────────────────────── */
typedef struct {
    int64_t  download_id;
    int64_t  activity_id;                  /* FK→daily_activities */
    char     customer[MAX_TEXT_LEN];
    char     lot[MAX_TEXT_LEN];
    char     ise_id[MAX_TEXT_LEN];
    int32_t  qty;
    char     device[MAX_TEXT_LEN];
    char     download_time[MAX_TEXT_LEN];
    char     notes[MAX_TEXT_LEN];
    int32_t  status;                       /* TaskStatus */
    int64_t  assigned_to;                  /* FK→team_members, 0=unassigned */
    int64_t  completed_at_ms;
    int64_t  created_at_ms;
} LRM_PACKED DownloadTask;

/* ── Table 16: eng_activity_tasks ──────────────────────── */
typedef struct {
    int64_t  eng_task_id;
    int64_t  activity_id;                  /* FK→daily_activities */
    char     customer[MAX_TEXT_LEN];
    char     device[MAX_TEXT_LEN];
    char     description[MAX_TEXT_LEN];
    char     ise_numbers[MAX_TEXT_LEN];
    int32_t  status;                       /* TaskStatus */
    int64_t  assigned_to;                  /* FK→team_members, 0=unassigned */
    int64_t  completed_at_ms;
    int64_t  created_at_ms;
} LRM_PACKED EngActivityTask;

/* ── Table 17: engineering_hours ───────────────────────── */
typedef struct {
    int64_t  entry_id;
    char     date[16];
    char     customer[MAX_TEXT_LEN];
    char     project[MAX_TEXT_LEN];
    char     pcb_number[MAX_TEXT_LEN];
    char     description[MAX_TEXT_LEN];
    char     engineer[MAX_TEXT_LEN];
    int32_t  hours_hundredths;             /* 1.5h = 150 */
    int32_t  billable;                     /* 1=billable, 0=not */
    int32_t  quoted_hours_hundredths;      /* 0=not set */
    char     po_number[MAX_TEXT_LEN];
    int64_t  source_task_id;               /* FK→eng_activity_tasks, 0=none */
    int64_t  created_at_ms;
} LRM_PACKED EngHoursEntry;

/* ── Table 18: v1_boards ──────────────────────────────────── */
typedef struct {
    int64_t  board_id;
    char     customer[MAX_TEXT_LEN];
    char     platform[MAX_TEXT_LEN];
    char     pcb_number_text[MAX_TEXT_LEN];
    char     revision[64];
    char     serial_no[MAX_TEXT_LEN];
    int32_t  power_qty;
    char     status[64];               /* string: Available, In Use, etc */
    int64_t  location_id;              /* FK→locations, 0=unassigned */
    int32_t  socket_rows;
    int32_t  socket_cols;
    char     notes[MAX_TEXT_LEN];
    char     individual_notes[MAX_TEXT_LEN];
    char     date_created[32];
    char     last_used_date[32];
    int32_t  sockets_working;
    int32_t  sockets_bad;
    int32_t  sockets_not_installed;
} LRM_PACKED V1Board;

/* ── Table 19: v1_board_types ─────────────────────────────── */
typedef struct {
    int64_t  board_type_id;
    char     customer[MAX_TEXT_LEN];
    char     pcb_number_text[MAX_TEXT_LEN];
    char     revision[64];
    char     platform[MAX_TEXT_LEN];
    int32_t  power_qty;
    int32_t  socket_rows;
    int32_t  socket_cols;
    char     notes[MAX_TEXT_LEN];
    int32_t  is_default;
    char     devices[MAX_TEXT_LEN];
} LRM_PACKED V1BoardType;

/* ── Table 20: v1_board_logs ──────────────────────────────── */
typedef struct {
    int64_t  log_id;
    int64_t  board_id;                 /* FK→v1_boards */
    char     timestamp[32];
    char     user[MAX_TEXT_LEN];
    char     action[MAX_TEXT_LEN];
    char     details[MAX_TEXT_LEN];
    int64_t  from_location_id;         /* 0=none */
    int64_t  to_location_id;           /* 0=none */
} LRM_PACKED V1BoardLog;

/* ── Table 21: v1_socket_notes ────────────────────────────── */
typedef struct {
    int64_t  note_id;
    int64_t  board_id;                 /* FK→v1_boards */
    int32_t  socket_number;
    char     status[64];               /* "working", "bad", "not_installed" */
    char     note[MAX_TEXT_LEN];
} LRM_PACKED V1SocketNote;

#ifdef _MSC_VER
  #pragma pack(pop)
#endif

/* ── API ────────────────────────────────────────────────── */
int  schema_init(Database *db);

/* Systems */
int  lrm_create_system(Database *db, System *sys, int64_t user_id);
int  lrm_update_system(Database *db, System *sys, int64_t user_id);
int  lrm_set_system_status(Database *db, int64_t sid, SystemStatus st, int64_t uid);
int  lrm_get_system(Database *db, int64_t sid, System *out);
int  lrm_list_systems(Database *db, System *out, uint32_t *count, uint32_t max);

/* Locations */
int  lrm_create_location(Database *db, Location *loc, int64_t user_id);
int  lrm_generate_system_tree(Database *db, int64_t system_id, int64_t user_id);
int  lrm_get_location(Database *db, int64_t lid, Location *out);
int  lrm_list_children(Database *db, int64_t parent_id,
                       Location *out, uint32_t *count, uint32_t max);
int  lrm_set_location_status(Database *db, int64_t lid, LocStatus st, int64_t uid);
int  lrm_rename_location(Database *db, int64_t lid, const char *new_name, int64_t uid);
int  lrm_move_location(Database *db, int64_t lid, int64_t new_parent_id, int64_t uid);

/* Devices */
int  lrm_create_device(Database *db, Device *dev, int64_t user_id);
int  lrm_get_device(Database *db, int64_t did, Device *out);
int  lrm_list_devices(Database *db, Device *out, uint32_t *count, uint32_t max);

/* Projects */
int  lrm_create_project(Database *db, Project *proj, int64_t user_id);
int  lrm_assign_project_to_system(Database *db, int64_t pid, int64_t sid, int64_t uid);
int  lrm_set_project_status(Database *db, int64_t pid, ProjectStatus st, int64_t uid);
int  lrm_get_project(Database *db, int64_t pid, Project *out);
int  lrm_list_projects(Database *db, Project *out, uint32_t *count, uint32_t max);
int  lrm_find_project_by_number(Database *db, const char *num, Project *out);

/* Lots */
int  lrm_create_lot(Database *db, Lot *lot, int64_t user_id);
int  lrm_get_lot(Database *db, int64_t lid, Lot *out);
int  lrm_advance_lot(Database *db, int64_t lid, LotStep step, int64_t uid);
int  lrm_set_lot_status(Database *db, int64_t lid, LotStatus st, int64_t uid);
int  lrm_update_lot_qty(Database *db, int64_t lid,
                        int32_t good, int32_t reject, int32_t missing, int64_t uid);
int  lrm_list_lots_for_project(Database *db, int64_t pid,
                                Lot *out, uint32_t *count, uint32_t max);

/* Hardware Types */
int  lrm_create_hw_type(Database *db, HardwareType *ht, int64_t user_id);
int  lrm_list_hw_types(Database *db, HardwareType *out, uint32_t *count, uint32_t max);

/* Serialized HW */
int  lrm_create_serialized(Database *db, SerializedHw *item, int64_t user_id);
int  lrm_move_serialized(Database *db, int64_t iid, int64_t sys, int64_t loc, int64_t uid);
int  lrm_assign_to_project(Database *db, int64_t iid, int64_t pid, int64_t uid);
int  lrm_unassign_from_project(Database *db, int64_t iid, int64_t uid);
int  lrm_set_item_status(Database *db, int64_t iid, ItemStatus st, int64_t uid);
int  lrm_find_by_serial(Database *db, const char *serial, SerializedHw *out);
int  lrm_get_serialized(Database *db, int64_t iid, SerializedHw *out);
int  lrm_list_serialized_at(Database *db, int64_t sys_id,
                            SerializedHw *out, uint32_t *count, uint32_t max);
int  lrm_list_serialized_for_project(Database *db, int64_t pid,
                                     SerializedHw *out, uint32_t *count, uint32_t max);
int  lrm_set_socket_status(Database *db, int64_t item_id, int32_t socket_idx,
                            SocketStatus status, int64_t uid);
int  lrm_get_socket_status(Database *db, int64_t item_id, int32_t socket_idx,
                            SocketStatus *out);

/* Quantity HW */
int  lrm_set_quantity(Database *db, QuantityHw *qty, int64_t user_id);
int  lrm_adjust_quantity(Database *db, int64_t qid, int32_t good_d, int32_t bad_d, int64_t uid);
int  lrm_list_quantity_at(Database *db, int64_t sys_id,
                          QuantityHw *out, uint32_t *count, uint32_t max);

/* Configured HW */
int  lrm_create_configured(Database *db, ConfiguredHw *cfg, int64_t user_id);
int  lrm_get_configured(Database *db, int64_t id, ConfiguredHw *out);
int  lrm_update_configured(Database *db, ConfiguredHw *cfg, int64_t user_id);
int  lrm_list_configured_for_project(Database *db, int64_t pid,
                                     ConfiguredHw *out, uint32_t *count, uint32_t max);

/* Audit */
int  lrm_get_entity_log(Database *db, const char *tbl, int64_t eid,
                        AuditEntry *out, uint32_t *count, uint32_t max);
int  lrm_get_recent_log(Database *db, AuditEntry *out, uint32_t *count, uint32_t max);

/* Users */
int  lrm_create_user(Database *db, User *user, const char *password);
int  lrm_authenticate(Database *db, const char *uname, const char *pw, User *out);
int  lrm_change_password(Database *db, int64_t uid, const char *old_pw, const char *new_pw);
int  lrm_reset_password(Database *db, int64_t uid, const char *new_pw);
int  lrm_list_users(Database *db, User *out, uint32_t *count, uint32_t max);

/* Batch */
int  lrm_batch_move(Database *db, int64_t *ids, uint32_t cnt,
                    int64_t sys, int64_t loc, int64_t uid);
int  lrm_batch_assign(Database *db, int64_t *ids, uint32_t cnt,
                      int64_t pid, int64_t uid);

/* ── Tracker: Team Members ─────────────────────────────── */
int  lrm_create_team_member(Database *db, TeamMember *tm, int64_t uid);
int  lrm_update_team_member(Database *db, TeamMember *tm, int64_t uid);
int  lrm_delete_team_member(Database *db, int64_t tmid, int64_t uid);
int  lrm_get_team_member(Database *db, int64_t tmid, TeamMember *out);
int  lrm_list_team_members(Database *db, TeamMember *out, uint32_t *count, uint32_t max);

/* ── Tracker: Daily Activities ─────────────────────────── */
int  lrm_create_activity(Database *db, DailyActivity *act, int64_t uid);
int  lrm_get_activity(Database *db, int64_t aid, DailyActivity *out);
int  lrm_find_activity_by_date(Database *db, const char *date, DailyActivity *out);
int  lrm_delete_activity(Database *db, int64_t aid, int64_t uid);
int  lrm_list_activities(Database *db, DailyActivity *out, uint32_t *count, uint32_t max);

/* ── Tracker: Upload Tasks ─────────────────────────────── */
int  lrm_create_upload(Database *db, UploadTask *task, int64_t uid);
int  lrm_set_upload_status(Database *db, int64_t uid_task, UploadStatus st, int64_t uid);
int  lrm_assign_upload(Database *db, int64_t uid_task, int64_t tmid, int64_t uid);
int  lrm_list_uploads_for_activity(Database *db, int64_t aid,
                                    UploadTask *out, uint32_t *count, uint32_t max);

/* ── Tracker: Download Tasks ───────────────────────────── */
int  lrm_create_download(Database *db, DownloadTask *task, int64_t uid);
int  lrm_set_download_status(Database *db, int64_t did, TaskStatus st, int64_t uid);
int  lrm_assign_download(Database *db, int64_t did, int64_t tmid, int64_t uid);
int  lrm_list_downloads_for_activity(Database *db, int64_t aid,
                                      DownloadTask *out, uint32_t *count, uint32_t max);

/* ── Tracker: Eng Activity Tasks ───────────────────────── */
int  lrm_create_eng_task(Database *db, EngActivityTask *task, int64_t uid);
int  lrm_set_eng_task_status(Database *db, int64_t eid, TaskStatus st, int64_t uid);
int  lrm_assign_eng_task(Database *db, int64_t eid, int64_t tmid, int64_t uid);
int  lrm_list_eng_tasks_for_activity(Database *db, int64_t aid,
                                      EngActivityTask *out, uint32_t *count, uint32_t max);

/* ── Tracker: Engineering Hours ────────────────────────── */
int  lrm_create_eng_hours(Database *db, EngHoursEntry *entry, int64_t uid);
int  lrm_update_eng_hours(Database *db, EngHoursEntry *entry, int64_t uid);
int  lrm_delete_eng_hours(Database *db, int64_t eid, int64_t uid);
int  lrm_get_eng_hours(Database *db, int64_t eid, EngHoursEntry *out);
int  lrm_list_eng_hours(Database *db, EngHoursEntry *out, uint32_t *count, uint32_t max);

/* ── V1 Compat: Boards ────────────────────────────────── */
int  lrm_create_v1_board(Database *db, V1Board *board, int64_t uid);
int  lrm_get_v1_board(Database *db, int64_t bid, V1Board *out);
int  lrm_update_v1_board(Database *db, V1Board *board, int64_t uid);
int  lrm_list_v1_boards(Database *db, V1Board *out, uint32_t *count, uint32_t max);
int  lrm_find_v1_board(Database *db, const char *customer, const char *pcb, const char *serial, V1Board *out);

/* ── V1 Compat: Board Types ──────────────────────────── */
int  lrm_create_v1_board_type(Database *db, V1BoardType *bt, int64_t uid);
int  lrm_get_v1_board_type(Database *db, int64_t btid, V1BoardType *out);
int  lrm_update_v1_board_type(Database *db, V1BoardType *bt, int64_t uid);
int  lrm_delete_v1_board_type(Database *db, int64_t btid, int64_t uid);
int  lrm_list_v1_board_types(Database *db, V1BoardType *out, uint32_t *count, uint32_t max);
int  lrm_find_v1_board_type(Database *db, const char *customer, const char *pcb, const char *rev, V1BoardType *out);

/* ── V1 Compat: Board Logs ───────────────────────────── */
int  lrm_create_v1_board_log(Database *db, V1BoardLog *log);
int  lrm_list_v1_board_logs(Database *db, int64_t board_id, V1BoardLog *out, uint32_t *count, uint32_t max);

/* ── V1 Compat: Socket Notes ─────────────────────────── */
int  lrm_upsert_v1_socket(Database *db, V1SocketNote *sn);
int  lrm_list_v1_sockets(Database *db, int64_t board_id, V1SocketNote *out, uint32_t *count, uint32_t max);

/* ── User additions ──────────────────────────────────── */
int  lrm_get_user(Database *db, int64_t uid, User *out);
int  lrm_find_user_by_name(Database *db, const char *username, User *out);
int  lrm_delete_user(Database *db, int64_t uid_user, int64_t uid_actor);
int  lrm_set_user_active(Database *db, int64_t uid_user, int32_t active, int64_t uid_actor);
int  lrm_set_user_password_hash(Database *db, int64_t uid_user, const char *hash);
int  lrm_set_user_role(Database *db, int64_t uid_user, int32_t role, int64_t uid_actor);

/* ── Location additions ──────────────────────────────── */
int  lrm_list_all_locations(Database *db, Location *out, uint32_t *count, uint32_t max);
int  lrm_delete_location(Database *db, int64_t lid, int64_t uid);

#endif /* LRM_SCHEMA_H */
