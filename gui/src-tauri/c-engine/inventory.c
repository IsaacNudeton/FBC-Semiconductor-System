/*
 * inventory.c — Burn-in inventory business logic v3
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

extern void btree_encode_i64(int64_t val, uint8_t *buf);

/* ── Audit helper ───────────────────────────────────────── */

static int audit(Database *db, int64_t uid, AuditAction act,
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

/* ── SHA-256 password hashing (v1 compatible) ──────────── */

static const uint32_t sha256_k[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define RR(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define CH(x,y,z) (((x)&(y))^((~(x))&(z)))
#define MAJ(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define EP0(x) (RR(x,2)^RR(x,13)^RR(x,22))
#define EP1(x) (RR(x,6)^RR(x,11)^RR(x,25))
#define SIG0(x) (RR(x,7)^RR(x,18)^((x)>>3))
#define SIG1(x) (RR(x,17)^RR(x,19)^((x)>>10))

static void sha256(const uint8_t *data, size_t len, uint8_t hash[32]) {
    uint32_t h[8] = {0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                     0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19};
    /* Padding: message + 0x80 + zeros + 64-bit length */
    size_t padded = ((len + 9 + 63) / 64) * 64;
    uint8_t *msg = (uint8_t*)calloc(padded, 1);
    memcpy(msg, data, len);
    msg[len] = 0x80;
    uint64_t bits = (uint64_t)len * 8;
    for (int i=0; i<8; i++) msg[padded-1-i] = (uint8_t)(bits>>(i*8));

    for (size_t off=0; off<padded; off+=64) {
        uint32_t w[64];
        for (int i=0;i<16;i++)
            w[i] = ((uint32_t)msg[off+i*4]<<24)|((uint32_t)msg[off+i*4+1]<<16)|
                   ((uint32_t)msg[off+i*4+2]<<8)|msg[off+i*4+3];
        for (int i=16;i<64;i++)
            w[i] = SIG1(w[i-2]) + w[i-7] + SIG0(w[i-15]) + w[i-16];

        uint32_t a=h[0],b=h[1],c=h[2],d=h[3],e=h[4],f=h[5],g=h[6],hh=h[7];
        for (int i=0;i<64;i++) {
            uint32_t t1 = hh + EP1(e) + CH(e,f,g) + sha256_k[i] + w[i];
            uint32_t t2 = EP0(a) + MAJ(a,b,c);
            hh=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
        }
        h[0]+=a; h[1]+=b; h[2]+=c; h[3]+=d;
        h[4]+=e; h[5]+=f; h[6]+=g; h[7]+=hh;
    }
    free(msg);
    for (int i=0;i<8;i++) {
        hash[i*4]=(uint8_t)(h[i]>>24); hash[i*4+1]=(uint8_t)(h[i]>>16);
        hash[i*4+2]=(uint8_t)(h[i]>>8); hash[i*4+3]=(uint8_t)h[i];
    }
}

static void hash_pw(const char *pw, char *out) {
    uint8_t digest[32];
    sha256((const uint8_t*)pw, strlen(pw), digest);
    for (int i=0; i<32; i++) snprintf(out+i*2, 3, "%02x", digest[i]);
    out[64] = 0;
}

/* ── Path cache rebuild ─────────────────────────────────── */

static int rebuild_path(Database *db, Location *loc) {
    if (loc->parent_id == 0) {
        snprintf(loc->path_cache, MAX_TEXT_LEN, "%s", loc->name);
        return LRM_OK;
    }
    Location parent;
    int rc = table_find_by_pk(db, "locations", loc->parent_id, &parent);
    if (rc != LRM_OK) {
        snprintf(loc->path_cache, MAX_TEXT_LEN, "%s", loc->name);
        return LRM_OK;
    }
    snprintf(loc->path_cache, MAX_TEXT_LEN, "%.120s/%.120s",
             parent.path_cache, loc->name);
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 * SYSTEMS
 * ══════════════════════════════════════════════════════════ */

int lrm_create_system(Database *db, System *sys, int64_t uid) {
    sys->status = SSTAT_FREE;
    int rc = table_insert(db, "systems", sys);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Created %s (%s)",
                          sys->name, system_type_str(sys->system_type));
    audit(db, uid, AUDIT_CREATE, "systems", sys->system_id, d);
    return LRM_OK;
}

int lrm_update_system(Database *db, System *sys, int64_t uid) {
    int rc = table_update(db, "systems", sys->system_id, sys);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_UPDATE, "systems", sys->system_id, "System updated");
    return rc;
}

int lrm_set_system_status(Database *db, int64_t sid, SystemStatus st, int64_t uid) {
    System sys;
    int rc = table_find_by_pk(db, "systems", sid, &sys);
    if (rc != LRM_OK) return rc;
    int32_t old = sys.status;
    sys.status = st;
    rc = table_update(db, "systems", sid, &sys);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s",
                          system_status_str(old), system_status_str(st));
    audit(db, uid, AUDIT_STATUS, "systems", sid, d);
    return LRM_OK;
}

int lrm_get_system(Database *db, int64_t sid, System *out) {
    return table_find_by_pk(db, "systems", sid, out);
}

int lrm_list_systems(Database *db, System *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "systems", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * LOCATIONS — with auto-tree generation
 * ══════════════════════════════════════════════════════════ */

int lrm_create_location(Database *db, Location *loc, int64_t uid) {
    loc->status = LSTAT_ACTIVE;
    rebuild_path(db, loc);
    int rc = table_insert(db, "locations", loc);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_CREATE, "locations", loc->location_id, loc->path_cache);
    return LRM_OK;
}

/*
 * Auto-generate the location tree for a system based on its config.
 * Sonoma: System → Ch1..ChN → Shelf1..ShelfM → Front/Back Tray
 * HX/XP/Shasta: System → Backplane1..BackplaneM → Slot1..SlotK
 * MCC: System → Chamber → Backplane1..BackplaneM → Slot1..SlotK
 */
int lrm_generate_system_tree(Database *db, int64_t system_id, int64_t uid) {
    System sys;
    int rc = lrm_get_system(db, system_id, &sys);
    if (rc != LRM_OK) return rc;

    for (int ch = 1; ch <= sys.chamber_count; ch++) {
        /* create chamber */
        Location chamber = {0};
        chamber.system_id = system_id;
        chamber.parent_id = 0; /* top-level within system */
        chamber.loc_type = LOC_CHAMBER;
        chamber.position = ch;
        snprintf(chamber.name, MAX_TEXT_LEN, "Chamber %d", ch);
        rc = lrm_create_location(db, &chamber, uid);
        if (rc != LRM_OK) return rc;

        for (int sh = 1; sh <= sys.shelves_per_chamber; sh++) {
            if (sys.system_type == SYS_SONOMA) {
                /* Sonoma: shelf → front/back tray */
                Location shelf = {0};
                shelf.system_id = system_id;
                shelf.parent_id = chamber.location_id;
                shelf.loc_type = LOC_SHELF;
                shelf.position = sh;
                snprintf(shelf.name, MAX_TEXT_LEN, "Shelf %d", sh);
                rc = lrm_create_location(db, &shelf, uid);
                if (rc != LRM_OK) return rc;

                /* front and back trays */
                const char *sides[] = {"Front", "Back"};
                for (int s = 0; s < sys.slots_per_shelf && s < 2; s++) {
                    Location tray = {0};
                    tray.system_id = system_id;
                    tray.parent_id = shelf.location_id;
                    tray.loc_type = LOC_TRAY;
                    tray.position = s + 1;
                    snprintf(tray.name, MAX_TEXT_LEN, "%s Tray", sides[s]);
                    rc = lrm_create_location(db, &tray, uid);
                    if (rc != LRM_OK) return rc;
                }
            } else {
                /* HX/XP/MCC/Shasta: backplane → slots */
                Location bp = {0};
                bp.system_id = system_id;
                bp.parent_id = chamber.location_id;
                bp.loc_type = LOC_BACKPLANE;
                bp.position = sh;
                snprintf(bp.name, MAX_TEXT_LEN, "Backplane %d", sh);
                rc = lrm_create_location(db, &bp, uid);
                if (rc != LRM_OK) return rc;

                for (int sl = 1; sl <= sys.slots_per_shelf; sl++) {
                    Location slot = {0};
                    slot.system_id = system_id;
                    slot.parent_id = bp.location_id;
                    slot.loc_type = LOC_SLOT;
                    slot.position = sl;
                    snprintf(slot.name, MAX_TEXT_LEN, "Slot %d", sl);
                    rc = lrm_create_location(db, &slot, uid);
                    if (rc != LRM_OK) return rc;
                }
            }
        }
    }

    char d[256]; snprintf(d, 256, "Generated tree: %dx%dx%d",
                          sys.chamber_count, sys.shelves_per_chamber,
                          sys.slots_per_shelf);
    audit(db, uid, AUDIT_CREATE, "systems", system_id, d);
    return LRM_OK;
}

/* Recursively rebuild path_cache for a location and all descendants */
static int rebuild_subtree_paths(Database *db, Location *loc) {
    rebuild_path(db, loc);
    int rc = table_update(db, "locations", loc->location_id, loc);
    if (rc != LRM_OK) return rc;

    Location children[64]; uint32_t cnt = 0;
    uint8_t key[8]; btree_encode_i64(loc->location_id, key);
    rc = table_find_by_index(db, "locations", "idx_loc_parent",
                             key, children, &cnt, 64);
    if (rc != LRM_OK) return rc;
    for (uint32_t i = 0; i < cnt; i++) {
        rc = rebuild_subtree_paths(db, &children[i]);
        if (rc != LRM_OK) return rc;
    }
    return LRM_OK;
}

int lrm_rename_location(Database *db, int64_t lid, const char *new_name, int64_t uid) {
    Location loc;
    int rc = table_find_by_pk(db, "locations", lid, &loc);
    if (rc != LRM_OK) return rc;
    strncpy(loc.name, new_name, MAX_TEXT_LEN - 1);
    loc.name[MAX_TEXT_LEN - 1] = '\0';
    rc = rebuild_subtree_paths(db, &loc);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Renamed → %s", new_name);
    audit(db, uid, AUDIT_UPDATE, "locations", lid, d);
    return LRM_OK;
}

int lrm_move_location(Database *db, int64_t lid, int64_t new_parent_id, int64_t uid) {
    Location loc;
    int rc = table_find_by_pk(db, "locations", lid, &loc);
    if (rc != LRM_OK) return rc;
    int64_t old_parent = loc.parent_id;
    loc.parent_id = new_parent_id;
    rc = rebuild_subtree_paths(db, &loc);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Moved parent %lld → %lld",
                          (long long)old_parent, (long long)new_parent_id);
    audit(db, uid, AUDIT_MOVE, "locations", lid, d);
    return LRM_OK;
}

int lrm_set_location_status(Database *db, int64_t lid, LocStatus st, int64_t uid) {
    Location loc;
    int rc = table_find_by_pk(db, "locations", lid, &loc);
    if (rc != LRM_OK) return rc;
    loc.status = st;
    rc = table_update(db, "locations", lid, &loc);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_STATUS, "locations", lid, loc_status_str(st));
    return LRM_OK;
}

int lrm_get_location(Database *db, int64_t lid, Location *out) {
    return table_find_by_pk(db, "locations", lid, out);
}

int lrm_list_children(Database *db, int64_t parent_id,
                      Location *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(parent_id, key);
    return table_find_by_index(db, "locations", "idx_loc_parent",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * DEVICES — customer+chip pair is the atomic identity
 * ══════════════════════════════════════════════════════════ */

int lrm_create_device(Database *db, Device *dev, int64_t uid) {
    int rc = table_insert(db, "devices", dev);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s %s", dev->customer, dev->device_name);
    audit(db, uid, AUDIT_CREATE, "devices", dev->device_id, d);
    return LRM_OK;
}

int lrm_get_device(Database *db, int64_t did, Device *out) {
    return table_find_by_pk(db, "devices", did, out);
}

int lrm_list_devices(Database *db, Device *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "devices", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * PROJECTS
 * ══════════════════════════════════════════════════════════ */

int lrm_create_project(Database *db, Project *proj, int64_t uid) {
    /* verify device exists */
    Device dev;
    int rc = table_find_by_pk(db, "devices", proj->device_id, &dev);
    if (rc != LRM_OK) return LRM_ERR_FK;

    proj->status = PROJ_ACTIVE;
    proj->start_date_ms = lrm_now_ms();
    rc = table_insert(db, "projects", proj);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s %s %s",
                          proj->project_number, dev.customer, dev.device_name);
    audit(db, uid, AUDIT_CREATE, "projects", proj->project_id, d);
    return LRM_OK;
}

int lrm_assign_project_to_system(Database *db, int64_t pid, int64_t sid, int64_t uid) {
    Project proj;
    int rc = table_find_by_pk(db, "projects", pid, &proj);
    if (rc != LRM_OK) return rc;
    /* verify system exists */
    System sys;
    rc = table_find_by_pk(db, "systems", sid, &sys);
    if (rc != LRM_OK) return LRM_ERR_FK;
    proj.system_id = sid;
    rc = table_update(db, "projects", pid, &proj);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s", proj.project_number, sys.name);
    audit(db, uid, AUDIT_ASSIGN, "projects", pid, d);
    return LRM_OK;
}

int lrm_set_project_status(Database *db, int64_t pid, ProjectStatus st, int64_t uid) {
    Project proj;
    int rc = table_find_by_pk(db, "projects", pid, &proj);
    if (rc != LRM_OK) return rc;
    proj.status = st;
    if (st == PROJ_COMPLETE) proj.end_date_ms = lrm_now_ms();
    rc = table_update(db, "projects", pid, &proj);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_STATUS, "projects", pid, project_status_str(st));
    return LRM_OK;
}

int lrm_get_project(Database *db, int64_t pid, Project *out) {
    return table_find_by_pk(db, "projects", pid, out);
}

int lrm_list_projects(Database *db, Project *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "projects", NULL, NULL, out, count, max);
}

int lrm_find_project_by_number(Database *db, const char *num, Project *out) {
    uint8_t key[64]; memset(key, 0, 64);
    strncpy((char*)key, num, 63);
    Project results[1]; uint32_t count = 0;
    int rc = table_find_by_index(db, "projects", "uq_proj_num",
                                 key, results, &count, 1);
    if (rc != LRM_OK || count == 0) return LRM_ERR_NOTFOUND;
    memcpy(out, &results[0], sizeof(Project));
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 * LOTS — the spine of every test run
 * ══════════════════════════════════════════════════════════ */

int lrm_create_lot(Database *db, Lot *lot, int64_t uid) {
    /* verify project exists */
    Project proj;
    int rc = table_find_by_pk(db, "projects", lot->project_id, &proj);
    if (rc != LRM_OK) return LRM_ERR_FK;

    lot->step = LOT_RECEIVED;
    lot->lot_status = LSTAT_LOT_ACTIVE;
    lot->received_ms = lrm_now_ms();
    rc = table_insert(db, "lots", lot);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Lot %s for %s", lot->lot_number, proj.project_number);
    audit(db, uid, AUDIT_CREATE, "lots", lot->lot_id, d);
    return LRM_OK;
}

int lrm_get_lot(Database *db, int64_t lid, Lot *out) {
    return table_find_by_pk(db, "lots", lid, out);
}

int lrm_advance_lot(Database *db, int64_t lid, LotStep step, int64_t uid) {
    Lot lot;
    int rc = table_find_by_pk(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    int32_t old = lot.step;
    lot.step = step;
    if (step == LOT_BURN_IN && lot.started_ms == 0)
        lot.started_ms = lrm_now_ms();
    if (step == LOT_COMPLETE) {
        lot.lot_status = LSTAT_LOT_COMPLETE;
        lot.completed_ms = lrm_now_ms();
    }
    rc = table_update(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s: %s → %s",
                          lot.lot_number, lot_step_str(old), lot_step_str(step));
    audit(db, uid, AUDIT_STATUS, "lots", lid, d);
    return LRM_OK;
}

int lrm_set_lot_status(Database *db, int64_t lid, LotStatus st, int64_t uid) {
    Lot lot;
    int rc = table_find_by_pk(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    LotStatus old = lot.lot_status;
    lot.lot_status = st;
    if (st == LSTAT_LOT_COMPLETE && lot.completed_ms == 0)
        lot.completed_ms = lrm_now_ms();
    rc = table_update(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s: %s → %s",
                          lot.lot_number, lot_status_str(old), lot_status_str(st));
    audit(db, uid, AUDIT_STATUS, "lots", lid, d);
    return LRM_OK;
}

int lrm_update_lot_qty(Database *db, int64_t lid,
                       int32_t good, int32_t reject, int32_t missing, int64_t uid) {
    Lot lot;
    int rc = table_find_by_pk(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    lot.good = good;
    lot.reject = reject;
    lot.missing = missing;
    lot.running_qty = good + reject + missing;
    rc = table_update(db, "lots", lid, &lot);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s: good=%d reject=%d missing=%d",
                          lot.lot_number, good, reject, missing);
    audit(db, uid, AUDIT_QTY, "lots", lid, d);
    return LRM_OK;
}

int lrm_list_lots_for_project(Database *db, int64_t pid,
                               Lot *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(pid, key);
    return table_find_by_index(db, "lots", "idx_lot_proj",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * HARDWARE TYPES
 * ══════════════════════════════════════════════════════════ */

int lrm_create_hw_type(Database *db, HardwareType *ht, int64_t uid) {
    int rc = table_insert(db, "hardware_types", ht);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_CREATE, "hardware_types", ht->type_id, ht->name);
    return LRM_OK;
}

int lrm_list_hw_types(Database *db, HardwareType *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "hardware_types", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * SERIALIZED HARDWARE (BIMs, BIBs, Boards, Powertrains)
 * ══════════════════════════════════════════════════════════ */

int lrm_create_serialized(Database *db, SerializedHw *item, int64_t uid) {
    /* verify type exists and is serialized */
    HardwareType ht;
    int rc = table_find_by_pk(db, "hardware_types", item->type_id, &ht);
    if (rc != LRM_OK) return LRM_ERR_FK;
    if (ht.tracking != TRACK_SERIALIZED) return LRM_ERR_CHECK;

    item->status = ITEM_AVAILABLE;
    item->date_created_ms = lrm_now_ms();
    rc = table_insert(db, "serialized_hw", item);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s: %s", hw_category_str(ht.category),
                          item->serial_no);
    audit(db, uid, AUDIT_CREATE, "serialized_hw", item->item_id, d);
    return LRM_OK;
}

int lrm_move_serialized(Database *db, int64_t iid,
                        int64_t sys, int64_t loc, int64_t uid) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;

    int64_t old_sys = item.system_id, old_loc = item.location_id;
    item.system_id = sys;
    item.location_id = loc;
    item.last_moved_ms = lrm_now_ms();
    rc = table_update(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;

    char d[256]; snprintf(d, 256, "Moved %s: sys %lld→%lld loc %lld→%lld",
                          item.serial_no, (long long)old_sys, (long long)sys,
                          (long long)old_loc, (long long)loc);
    audit(db, uid, AUDIT_MOVE, "serialized_hw", iid, d);
    return LRM_OK;
}

int lrm_assign_to_project(Database *db, int64_t iid, int64_t pid, int64_t uid) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    /* verify project exists */
    Project proj;
    rc = table_find_by_pk(db, "projects", pid, &proj);
    if (rc != LRM_OK) return LRM_ERR_FK;

    item.project_id = pid;
    item.status = ITEM_IN_USE;
    rc = table_update(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s", item.serial_no, proj.project_number);
    audit(db, uid, AUDIT_ASSIGN, "serialized_hw", iid, d);
    return LRM_OK;
}

int lrm_unassign_from_project(Database *db, int64_t iid, int64_t uid) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    item.project_id = 0;
    item.status = ITEM_AVAILABLE;
    rc = table_update(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_UNASSIGN, "serialized_hw", iid, item.serial_no);
    return LRM_OK;
}

int lrm_set_item_status(Database *db, int64_t iid, ItemStatus st, int64_t uid) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    int32_t old = item.status;
    item.status = st;
    rc = table_update(db, "serialized_hw", iid, &item);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s: %s → %s", item.serial_no,
                          item_status_str(old), item_status_str(st));
    audit(db, uid, AUDIT_STATUS, "serialized_hw", iid, d);
    return LRM_OK;
}

int lrm_find_by_serial(Database *db, const char *serial, SerializedHw *out) {
    uint8_t key[MAX_TEXT_LEN]; memset(key, 0, MAX_TEXT_LEN);
    strncpy((char*)key, serial, MAX_TEXT_LEN-1);
    SerializedHw results[1]; uint32_t count = 0;
    int rc = table_find_by_index(db, "serialized_hw", "uq_serial",
                                 key, results, &count, 1);
    if (rc != LRM_OK || count == 0) return LRM_ERR_NOTFOUND;
    memcpy(out, &results[0], sizeof(SerializedHw));
    return LRM_OK;
}

int lrm_get_serialized(Database *db, int64_t iid, SerializedHw *out) {
    return table_find_by_pk(db, "serialized_hw", iid, out);
}

int lrm_list_serialized_at(Database *db, int64_t sys_id,
                           SerializedHw *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(sys_id, key);
    return table_find_by_index(db, "serialized_hw", "idx_ser_sys",
                               key, out, count, max);
}

int lrm_list_serialized_for_project(Database *db, int64_t pid,
                                    SerializedHw *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(pid, key);
    return table_find_by_index(db, "serialized_hw", "idx_ser_proj",
                               key, out, count, max);
}

/* ── Socket health (2-bit bitmask) ─────────────────────── */

int lrm_set_socket_status(Database *db, int64_t item_id, int32_t socket_idx,
                           SocketStatus status, int64_t uid) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", item_id, &item);
    if (rc != LRM_OK) return rc;
    if (socket_idx < 0 || socket_idx >= 64) return LRM_ERR_CHECK;
    if (status < 0 || status >= SOCK__COUNT) return LRM_ERR_CHECK;

    int byte_idx = socket_idx / 4;
    int bit_off  = (socket_idx % 4) * 2;
    item.socket_mask[byte_idx] &= ~(0x03 << bit_off);
    item.socket_mask[byte_idx] |= ((uint8_t)status & 0x03) << bit_off;

    rc = table_update(db, "serialized_hw", item_id, &item);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s socket[%d] → %s",
                          item.serial_no, socket_idx, socket_status_str(status));
    audit(db, uid, AUDIT_UPDATE, "serialized_hw", item_id, d);
    return LRM_OK;
}

int lrm_get_socket_status(Database *db, int64_t item_id, int32_t socket_idx,
                           SocketStatus *out) {
    SerializedHw item;
    int rc = table_find_by_pk(db, "serialized_hw", item_id, &item);
    if (rc != LRM_OK) return rc;
    if (socket_idx < 0 || socket_idx >= 64) return LRM_ERR_CHECK;

    int byte_idx = socket_idx / 4;
    int bit_off  = (socket_idx % 4) * 2;
    *out = (SocketStatus)((item.socket_mask[byte_idx] >> bit_off) & 0x03);
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 * QUANTITY HARDWARE (temp cards, controllers, neg supply)
 * ══════════════════════════════════════════════════════════ */

int lrm_set_quantity(Database *db, QuantityHw *qty, int64_t uid) {
    qty->last_updated_ms = lrm_now_ms();
    int rc = table_insert(db, "quantity_hw", qty);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "total=%d good=%d bad=%d",
                          qty->total, qty->good, qty->bad);
    audit(db, uid, AUDIT_CREATE, "quantity_hw", qty->qty_id, d);
    return LRM_OK;
}

int lrm_adjust_quantity(Database *db, int64_t qid,
                        int32_t good_d, int32_t bad_d, int64_t uid) {
    QuantityHw qty;
    int rc = table_find_by_pk(db, "quantity_hw", qid, &qty);
    if (rc != LRM_OK) return rc;
    qty.good += good_d;
    qty.bad += bad_d;
    qty.total = qty.good + qty.bad;
    if (qty.good < 0) qty.good = 0;
    if (qty.bad < 0) qty.bad = 0;
    qty.last_updated_ms = lrm_now_ms();
    rc = table_update(db, "quantity_hw", qid, &qty);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Adjusted: good%+d bad%+d → total=%d",
                          good_d, bad_d, qty.total);
    audit(db, uid, AUDIT_QTY, "quantity_hw", qid, d);
    return LRM_OK;
}

int lrm_list_quantity_at(Database *db, int64_t sys_id,
                         QuantityHw *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(sys_id, key);
    return table_find_by_index(db, "quantity_hw", "idx_qty_sys",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * CONFIGURED HARDWARE (LCPS, HCPS, Cores)
 * ══════════════════════════════════════════════════════════ */

int lrm_create_configured(Database *db, ConfiguredHw *cfg, int64_t uid) {
    HardwareType ht;
    int rc = table_find_by_pk(db, "hardware_types", cfg->type_id, &ht);
    if (rc != LRM_OK) return LRM_ERR_FK;
    if (ht.tracking != TRACK_CONFIGURED) return LRM_ERR_CHECK;

    cfg->last_updated_ms = lrm_now_ms();
    rc = table_insert(db, "configured_hw", cfg);
    if (rc != LRM_OK) return rc;

    char d[256];
    if (ht.category == HW_CORE) {
        snprintf(d, 256, "%s: %s x%d @%dmV",
                 ht.name, core_role_str(cfg->role), cfg->quantity, cfg->vout_mv);
    } else {
        snprintf(d, 256, "%s: R0=%dΩ R4=%dΩ VOUT=%dmV x%d",
                 ht.name, cfg->r0_ohms, cfg->r4_ohms, cfg->vout_mv, cfg->quantity);
    }
    audit(db, uid, AUDIT_CREATE, "configured_hw", cfg->config_id, d);
    return LRM_OK;
}

int lrm_get_configured(Database *db, int64_t id, ConfiguredHw *out) {
    return table_find_by_pk(db, "configured_hw", id, out);
}

int lrm_update_configured(Database *db, ConfiguredHw *cfg, int64_t uid) {
    cfg->last_updated_ms = lrm_now_ms();
    int rc = table_update(db, "configured_hw", cfg->config_id, cfg);
    if (rc != LRM_OK) return rc;
    audit(db, uid, AUDIT_UPDATE, "configured_hw", cfg->config_id, "Config updated");
    return LRM_OK;
}

int lrm_list_configured_for_project(Database *db, int64_t pid,
                                    ConfiguredHw *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(pid, key);
    return table_find_by_index(db, "configured_hw", "idx_cfg_proj",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * AUDIT LOG
 * ══════════════════════════════════════════════════════════ */

int lrm_get_entity_log(Database *db, const char *tbl, int64_t eid,
                       AuditEntry *out, uint32_t *count, uint32_t max) {
    uint8_t key[72]; /* entity_table (64) + entity_id (8) */
    memset(key, 0, 72);
    strncpy((char*)key, tbl, 63);
    btree_encode_i64(eid, key + 64);
    return table_find_by_index(db, "audit_log", "idx_audit_entity",
                               key, out, count, max);
}

int lrm_get_recent_log(Database *db, AuditEntry *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "audit_log", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * USERS
 * ══════════════════════════════════════════════════════════ */

int lrm_create_user(Database *db, User *user, const char *password) {
    hash_pw(password, user->password_hash);
    user->active = 1;
    user->created_ms = lrm_now_ms();
    return table_insert(db, "users", user);
}

int lrm_authenticate(Database *db, const char *uname, const char *pw, User *out) {
    uint8_t key[MAX_TEXT_LEN]; memset(key, 0, MAX_TEXT_LEN);
    strncpy((char*)key, uname, MAX_TEXT_LEN-1);
    User results[1]; uint32_t count = 0;
    int rc = table_find_by_index(db, "users", "uq_username",
                                 key, results, &count, 1);
    if (rc != LRM_OK || count == 0) return LRM_ERR_NOTFOUND;
    if (!results[0].active) return LRM_ERR_CHECK;
    char hash[HASH_LEN]; hash_pw(pw, hash);
    if (strncmp(hash, results[0].password_hash, HASH_LEN) != 0)
        return LRM_ERR_CHECK;
    results[0].last_login_ms = lrm_now_ms();
    table_update(db, "users", results[0].user_id, &results[0]);
    memcpy(out, &results[0], sizeof(User));
    return LRM_OK;
}

int lrm_change_password(Database *db, int64_t uid, const char *old_pw, const char *new_pw) {
    User user;
    int rc = table_find_by_pk(db, "users", uid, &user);
    if (rc != LRM_OK) return rc;
    char hash[HASH_LEN]; hash_pw(old_pw, hash);
    if (strncmp(hash, user.password_hash, HASH_LEN) != 0) return LRM_ERR_CHECK;
    hash_pw(new_pw, user.password_hash);
    return table_update(db, "users", uid, &user);
}

int lrm_reset_password(Database *db, int64_t uid, const char *new_pw) {
    User user;
    int rc = table_find_by_pk(db, "users", uid, &user);
    if (rc != LRM_OK) return rc;
    hash_pw(new_pw, user.password_hash);
    return table_update(db, "users", uid, &user);
}

int lrm_list_users(Database *db, User *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "users", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 * BATCH OPERATIONS (transactional)
 * ══════════════════════════════════════════════════════════ */

int lrm_batch_move(Database *db, int64_t *ids, uint32_t cnt,
                   int64_t sys, int64_t loc, int64_t uid) {
    Txn txn; int rc = txn_begin(&txn, db);
    if (rc != LRM_OK) return rc;
    for (uint32_t i = 0; i < cnt; i++) {
        rc = lrm_move_serialized(db, ids[i], sys, loc, uid);
        if (rc != LRM_OK) { txn_rollback(&txn); return rc; }
    }
    return txn_commit(&txn);
}

int lrm_batch_assign(Database *db, int64_t *ids, uint32_t cnt,
                     int64_t pid, int64_t uid) {
    Txn txn; int rc = txn_begin(&txn, db);
    if (rc != LRM_OK) return rc;
    for (uint32_t i = 0; i < cnt; i++) {
        rc = lrm_assign_to_project(db, ids[i], pid, uid);
        if (rc != LRM_OK) { txn_rollback(&txn); return rc; }
    }
    return txn_commit(&txn);
}
