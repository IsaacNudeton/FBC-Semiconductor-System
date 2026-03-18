/*
 * tracker.c — Burn-in tracker business logic (v4)
 *
 * Team members, daily activities, upload/download/eng tasks,
 * engineering hours. Operational workflow tables.
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <string.h>
#include <stdio.h>

extern void btree_encode_i64(int64_t val, uint8_t *buf);

/* ── Audit helper (same pattern as inventory.c) ────────── */

static int t_audit(Database *db, int64_t uid, AuditAction act,
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
 *  Team Members
 * ══════════════════════════════════════════════════════════ */

int lrm_create_team_member(Database *db, TeamMember *tm, int64_t uid) {
    int rc = table_insert(db, "team_members", tm);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Created team member: %s", tm->name);
    t_audit(db, uid, AUDIT_CREATE, "team_members", tm->team_member_id, d);
    return LRM_OK;
}

int lrm_update_team_member(Database *db, TeamMember *tm, int64_t uid) {
    int rc = table_update(db, "team_members", tm->team_member_id, tm);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Updated team member: %s", tm->name);
    t_audit(db, uid, AUDIT_UPDATE, "team_members", tm->team_member_id, d);
    return LRM_OK;
}

int lrm_delete_team_member(Database *db, int64_t tmid, int64_t uid) {
    TeamMember tm;
    int rc = table_find_by_pk(db, "team_members", tmid, &tm);
    if (rc != LRM_OK) return rc;
    rc = table_delete(db, "team_members", tmid);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Deleted team member: %s", tm.name);
    t_audit(db, uid, AUDIT_DELETE, "team_members", tmid, d);
    return LRM_OK;
}

int lrm_get_team_member(Database *db, int64_t tmid, TeamMember *out) {
    return table_find_by_pk(db, "team_members", tmid, out);
}

int lrm_list_team_members(Database *db, TeamMember *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "team_members", NULL, NULL, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  Daily Activities
 * ══════════════════════════════════════════════════════════ */

int lrm_create_activity(Database *db, DailyActivity *act, int64_t uid) {
    act->created_at_ms = lrm_now_ms();
    int rc = table_insert(db, "daily_activities", act);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Created activity for %s", act->date);
    t_audit(db, uid, AUDIT_CREATE, "daily_activities", act->activity_id, d);
    return LRM_OK;
}

int lrm_get_activity(Database *db, int64_t aid, DailyActivity *out) {
    return table_find_by_pk(db, "daily_activities", aid, out);
}

int lrm_find_activity_by_date(Database *db, const char *date, DailyActivity *out) {
    uint8_t key[16]; memset(key, 0, 16);
    strncpy((char*)key, date, 15);
    DailyActivity results[1]; uint32_t count = 0;
    int rc = table_find_by_index(db, "daily_activities", "uq_act_date",
                                 key, results, &count, 1);
    if (rc != LRM_OK || count == 0) return LRM_ERR_NOTFOUND;
    memcpy(out, &results[0], sizeof(DailyActivity));
    return LRM_OK;
}

int lrm_list_activities(Database *db, DailyActivity *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "daily_activities", NULL, NULL, out, count, max);
}

int lrm_delete_activity(Database *db, int64_t aid, int64_t uid) {
    /* Manual CASCADE: delete all child tasks first */
    DailyActivity act;
    int rc = table_find_by_pk(db, "daily_activities", aid, &act);
    if (rc != LRM_OK) return rc;

    uint8_t key[8]; btree_encode_i64(aid, key);

    /* Delete upload_tasks */
    UploadTask ups[64]; uint32_t ucnt = 0;
    table_find_by_index(db, "upload_tasks", "idx_upload_act",
                        key, ups, &ucnt, 64);
    for (uint32_t i = 0; i < ucnt; i++)
        table_delete(db, "upload_tasks", ups[i].upload_id);

    /* Delete download_tasks */
    DownloadTask dls[64]; uint32_t dcnt = 0;
    table_find_by_index(db, "download_tasks", "idx_download_act",
                        key, dls, &dcnt, 64);
    for (uint32_t i = 0; i < dcnt; i++)
        table_delete(db, "download_tasks", dls[i].download_id);

    /* Delete eng_activity_tasks */
    EngActivityTask ets[64]; uint32_t ecnt = 0;
    table_find_by_index(db, "eng_activity_tasks", "idx_eng_task_act",
                        key, ets, &ecnt, 64);
    for (uint32_t i = 0; i < ecnt; i++)
        table_delete(db, "eng_activity_tasks", ets[i].eng_task_id);

    /* Delete the activity itself */
    rc = table_delete(db, "daily_activities", aid);
    if (rc != LRM_OK) return rc;

    char d[256]; snprintf(d, 256, "Deleted activity %s (cascade: %u ups, %u dls, %u eng)",
                          act.date, ucnt, dcnt, ecnt);
    t_audit(db, uid, AUDIT_DELETE, "daily_activities", aid, d);
    return LRM_OK;
}

/* ══════════════════════════════════════════════════════════
 *  Upload Tasks
 * ══════════════════════════════════════════════════════════ */

int lrm_create_upload(Database *db, UploadTask *task, int64_t uid) {
    /* Verify activity FK */
    DailyActivity act;
    if (table_find_by_pk(db, "daily_activities", task->activity_id, &act) != LRM_OK)
        return LRM_ERR_FK;
    /* Verify assigned_to FK if set */
    if (task->assigned_to > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", task->assigned_to, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task->status = UPSTAT_NOT_LOADED;
    task->created_at_ms = lrm_now_ms();
    int rc = table_insert(db, "upload_tasks", task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Upload: %s %s", task->customer, task->lot);
    t_audit(db, uid, AUDIT_CREATE, "upload_tasks", task->upload_id, d);
    return LRM_OK;
}

int lrm_set_upload_status(Database *db, int64_t task_id, UploadStatus st, int64_t uid) {
    UploadTask task;
    int rc = table_find_by_pk(db, "upload_tasks", task_id, &task);
    if (rc != LRM_OK) return rc;
    int32_t old = task.status;
    task.status = st;
    if (st == UPSTAT_LOADED)
        task.completed_at_ms = lrm_now_ms();
    else
        task.completed_at_ms = 0;
    rc = table_update(db, "upload_tasks", task_id, &task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s",
                          upload_status_str(old), upload_status_str(st));
    t_audit(db, uid, AUDIT_STATUS, "upload_tasks", task_id, d);
    return LRM_OK;
}

int lrm_assign_upload(Database *db, int64_t task_id, int64_t tmid, int64_t uid) {
    UploadTask task;
    int rc = table_find_by_pk(db, "upload_tasks", task_id, &task);
    if (rc != LRM_OK) return rc;
    if (tmid > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", tmid, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task.assigned_to = tmid;
    rc = table_update(db, "upload_tasks", task_id, &task);
    if (rc != LRM_OK) return rc;
    t_audit(db, uid, tmid > 0 ? AUDIT_ASSIGN : AUDIT_UNASSIGN,
            "upload_tasks", task_id, NULL);
    return LRM_OK;
}

int lrm_list_uploads_for_activity(Database *db, int64_t aid,
                                   UploadTask *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(aid, key);
    return table_find_by_index(db, "upload_tasks", "idx_upload_act",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  Download Tasks
 * ══════════════════════════════════════════════════════════ */

int lrm_create_download(Database *db, DownloadTask *task, int64_t uid) {
    DailyActivity act;
    if (table_find_by_pk(db, "daily_activities", task->activity_id, &act) != LRM_OK)
        return LRM_ERR_FK;
    if (task->assigned_to > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", task->assigned_to, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task->status = TSTAT_PENDING;
    task->created_at_ms = lrm_now_ms();
    int rc = table_insert(db, "download_tasks", task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Download: %s %s", task->customer, task->lot);
    t_audit(db, uid, AUDIT_CREATE, "download_tasks", task->download_id, d);
    return LRM_OK;
}

int lrm_set_download_status(Database *db, int64_t did, TaskStatus st, int64_t uid) {
    DownloadTask task;
    int rc = table_find_by_pk(db, "download_tasks", did, &task);
    if (rc != LRM_OK) return rc;
    int32_t old = task.status;
    task.status = st;
    if (st == TSTAT_COMPLETED)
        task.completed_at_ms = lrm_now_ms();
    else
        task.completed_at_ms = 0;
    rc = table_update(db, "download_tasks", did, &task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s",
                          task_status_str(old), task_status_str(st));
    t_audit(db, uid, AUDIT_STATUS, "download_tasks", did, d);
    return LRM_OK;
}

int lrm_assign_download(Database *db, int64_t did, int64_t tmid, int64_t uid) {
    DownloadTask task;
    int rc = table_find_by_pk(db, "download_tasks", did, &task);
    if (rc != LRM_OK) return rc;
    if (tmid > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", tmid, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task.assigned_to = tmid;
    rc = table_update(db, "download_tasks", did, &task);
    if (rc != LRM_OK) return rc;
    t_audit(db, uid, tmid > 0 ? AUDIT_ASSIGN : AUDIT_UNASSIGN,
            "download_tasks", did, NULL);
    return LRM_OK;
}

int lrm_list_downloads_for_activity(Database *db, int64_t aid,
                                     DownloadTask *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(aid, key);
    return table_find_by_index(db, "download_tasks", "idx_download_act",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  Eng Activity Tasks
 * ══════════════════════════════════════════════════════════ */

int lrm_create_eng_task(Database *db, EngActivityTask *task, int64_t uid) {
    DailyActivity act;
    if (table_find_by_pk(db, "daily_activities", task->activity_id, &act) != LRM_OK)
        return LRM_ERR_FK;
    if (task->assigned_to > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", task->assigned_to, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task->status = TSTAT_PENDING;
    task->created_at_ms = lrm_now_ms();
    int rc = table_insert(db, "eng_activity_tasks", task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Eng task: %s %s", task->customer, task->device);
    t_audit(db, uid, AUDIT_CREATE, "eng_activity_tasks", task->eng_task_id, d);
    return LRM_OK;
}

int lrm_set_eng_task_status(Database *db, int64_t eid, TaskStatus st, int64_t uid) {
    EngActivityTask task;
    int rc = table_find_by_pk(db, "eng_activity_tasks", eid, &task);
    if (rc != LRM_OK) return rc;
    int32_t old = task.status;
    task.status = st;
    if (st == TSTAT_COMPLETED)
        task.completed_at_ms = lrm_now_ms();
    else
        task.completed_at_ms = 0;
    rc = table_update(db, "eng_activity_tasks", eid, &task);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "%s → %s",
                          task_status_str(old), task_status_str(st));
    t_audit(db, uid, AUDIT_STATUS, "eng_activity_tasks", eid, d);
    return LRM_OK;
}

int lrm_assign_eng_task(Database *db, int64_t eid, int64_t tmid, int64_t uid) {
    EngActivityTask task;
    int rc = table_find_by_pk(db, "eng_activity_tasks", eid, &task);
    if (rc != LRM_OK) return rc;
    if (tmid > 0) {
        TeamMember tm;
        if (table_find_by_pk(db, "team_members", tmid, &tm) != LRM_OK)
            return LRM_ERR_FK;
    }
    task.assigned_to = tmid;
    rc = table_update(db, "eng_activity_tasks", eid, &task);
    if (rc != LRM_OK) return rc;
    t_audit(db, uid, tmid > 0 ? AUDIT_ASSIGN : AUDIT_UNASSIGN,
            "eng_activity_tasks", eid, NULL);
    return LRM_OK;
}

int lrm_list_eng_tasks_for_activity(Database *db, int64_t aid,
                                     EngActivityTask *out, uint32_t *count, uint32_t max) {
    uint8_t key[8]; btree_encode_i64(aid, key);
    return table_find_by_index(db, "eng_activity_tasks", "idx_eng_task_act",
                               key, out, count, max);
}

/* ══════════════════════════════════════════════════════════
 *  Engineering Hours
 * ══════════════════════════════════════════════════════════ */

int lrm_create_eng_hours(Database *db, EngHoursEntry *entry, int64_t uid) {
    /* Verify source_task FK if set */
    if (entry->source_task_id > 0) {
        EngActivityTask et;
        if (table_find_by_pk(db, "eng_activity_tasks", entry->source_task_id, &et) != LRM_OK)
            return LRM_ERR_FK;
    }
    entry->created_at_ms = lrm_now_ms();
    int rc = table_insert(db, "engineering_hours", entry);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Eng hours: %s %s %.2fh",
                          entry->engineer, entry->customer,
                          entry->hours_hundredths / 100.0);
    t_audit(db, uid, AUDIT_CREATE, "engineering_hours", entry->entry_id, d);
    return LRM_OK;
}

int lrm_update_eng_hours(Database *db, EngHoursEntry *entry, int64_t uid) {
    int rc = table_update(db, "engineering_hours", entry->entry_id, entry);
    if (rc != LRM_OK) return rc;
    char d[256]; snprintf(d, 256, "Updated eng hours: %s %.2fh",
                          entry->engineer, entry->hours_hundredths / 100.0);
    t_audit(db, uid, AUDIT_UPDATE, "engineering_hours", entry->entry_id, d);
    return LRM_OK;
}

int lrm_delete_eng_hours(Database *db, int64_t eid, int64_t uid) {
    int rc = table_delete(db, "engineering_hours", eid);
    if (rc != LRM_OK) return rc;
    t_audit(db, uid, AUDIT_DELETE, "engineering_hours", eid, "Deleted eng hours entry");
    return LRM_OK;
}

int lrm_get_eng_hours(Database *db, int64_t eid, EngHoursEntry *out) {
    return table_find_by_pk(db, "engineering_hours", eid, out);
}

int lrm_list_eng_hours(Database *db, EngHoursEntry *out, uint32_t *count, uint32_t max) {
    return table_scan(db, "engineering_hours", NULL, NULL, out, count, max);
}
