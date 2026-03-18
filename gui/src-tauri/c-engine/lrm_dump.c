/*
 * lrm_dump.c — Disaster Recovery Export Tool
 *
 * Reads lrm_inventory.db in read-only mode (no lock required)
 * and exports all 21 tables to CSV files.
 *
 * Usage: lrm_dump.exe <database.db> [output_dir]
 *
 * Can run while lrm-server is active — uses read-only file handle.
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <io.h>
#else
#include <unistd.h>
#endif

/* ── CSV Field Escaping ────────────────────────────────── */

static void fprint_csv_field(FILE *out, const char *field, size_t max_len) {
    if (!field) return;
    
    int need_quote = 0;
    size_t len = strnlen(field, max_len);
    
    for (size_t i = 0; i < len; i++) {
        char c = field[i];
        if (c == ',' || c == '"' || c == '\n' || c == '\r') {
            need_quote = 1;
            break;
        }
    }
    
    if (need_quote) {
        fputc('"', out);
        for (size_t i = 0; i < len; i++) {
            if (field[i] == '"') fputc('"', out);
            fputc(field[i], out);
        }
        fputc('"', out);
    } else {
        fwrite(field, 1, len, out);
    }
}

static void fprint_csv_i64(FILE *out, int64_t val) {
    fprintf(out, "%lld", (long long)val);
}

static void fprint_csv_i32(FILE *out, int32_t val) {
    fprintf(out, "%d", (int)val);
}

/* ── Read-Only Database Open ───────────────────────────── */

static int db_open_readonly(Database *db, const char *path) {
    memset(db, 0, sizeof(Database));
    strncpy(db->path, path, sizeof(db->path) - 1);
    
    db->magic = DB_MAGIC;
    db->version = DB_VERSION;
    
    FILE *fp = fopen(path, "rb");
    if (!fp) return LRM_ERR_IO;
    
    pool_init(&db->pool, fp);
    
    Page *hdr = pool_get(&db->pool, 0);
    if (!hdr) { fclose(fp); return LRM_ERR_IO; }
    
    uint32_t magic;
    memcpy(&magic, hdr->data, 4);
    if (magic != DB_MAGIC) { fclose(fp); return LRM_ERR_CORRUPT; }
    
    db->open = 1;
    return LRM_OK;
}

/* ── Table Dump ────────────────────────────────────────── */

static int dump_table(Database *db, const char *table_name, const char *csv_path) {
    TableDef *t = find_table(db, table_name);
    if (!t) {
        fprintf(stderr, "  Table '%s' not found\n", table_name);
        return LRM_ERR_NOTFOUND;
    }
    
    FILE *csv = fopen(csv_path, "w");
    if (!csv) {
        fprintf(stderr, "  Cannot create '%s'\n", csv_path);
        return LRM_ERR_IO;
    }
    
    /* Header row */
    for (uint32_t c = 0; c < t->num_cols; c++) {
        if (c > 0) fputc(',', csv);
        fputs(t->cols[c].name, csv);
    }
    fputc('\n', csv);
    
    /* Data rows */
    void *record_buf = malloc(t->record_size);
    if (!record_buf) { fclose(csv); return LRM_ERR_IO; }
    
    uint32_t count = 0;
    while (1) {
        count = 0;
        int rc = table_scan(db, table_name, NULL, NULL, record_buf, &count, 1);
        if (rc != LRM_OK || count == 0) break;
        
        uint8_t *rec = (uint8_t *)record_buf;
        for (uint32_t c = 0; c < t->num_cols; c++) {
            if (c > 0) fputc(',', csv);
            
            const void *field = rec + t->cols[c].offset;
            switch (t->cols[c].size) {
                case 8: fprint_csv_i64(csv, *(const int64_t *)field); break;
                case 4: fprint_csv_i32(csv, *(const int32_t *)field); break;
                case 2: fprint_csv_i32(csv, *(const int16_t *)field); break;
                case 1: fprint_csv_i32(csv, *(const int8_t *)field); break;
                default: fprint_csv_field(csv, (const char *)field, t->cols[c].size); break;
            }
        }
        fputc('\n', csv);
    }
    
    free(record_buf);
    fclose(csv);
    return LRM_OK;
}

/* ── Main ──────────────────────────────────────────────── */

static const char *ALL_TABLES[] = {
    "systems", "locations", "devices", "projects", "lots",
    "hardware_types", "serialized_hw", "quantity_hw", "configured_hw",
    "audit_log", "users", "team_members", "daily_activities",
    "upload_tasks", "download_tasks", "eng_activity_tasks",
    "engineering_hours", "v1_boards", "v1_board_types",
    "v1_board_logs", "v1_socket_notes", NULL
};

int main(int argc, char *argv[]) {
    printf("╔═══════════════════════════════════════════════════╗\n");
    printf("║  LRM Database Dump — Disaster Recovery Export    ║\n");
    printf("╚═══════════════════════════════════════════════════╝\n\n");
    
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <database.db> [output_dir]\n", argv[0]);
        return 1;
    }
    
    const char *db_path = argv[1];
    const char *output_dir = argc > 2 ? argv[2] : ".";
    
    Database db;
    int rc = db_open_readonly(&db, db_path);
    if (rc != LRM_OK) {
        fprintf(stderr, "Failed to open '%s' (error %d)\n", db_path, rc);
        return 1;
    }
    
    /* Initialize schema (registers all 21 tables) */
    rc = schema_init(&db);
    if (rc != LRM_OK) {
        fprintf(stderr, "Failed to initialize schema (error %d)\n", rc);
        db_close(&db);
        return 1;
    }
    
    printf("Opened: %s (read-only)\n", db_path);
    printf("Output: %s/\n\n", output_dir);
    
    int dumped = 0, failed = 0;
    
    for (int i = 0; ALL_TABLES[i] != NULL; i++) {
        char csv_path[512];
        snprintf(csv_path, sizeof(csv_path), "%s/%s.csv", output_dir, ALL_TABLES[i]);
        
        printf("Dumping %-20s → %s ... ", ALL_TABLES[i], csv_path);
        fflush(stdout);
        
        rc = dump_table(&db, ALL_TABLES[i], csv_path);
        if (rc == LRM_OK) { printf("OK\n"); dumped++; }
        else { printf("FAILED (%d)\n", rc); failed++; }
    }
    
    db_close(&db);
    
    printf("\n═══════════════════════════════════════════════════\n");
    printf("  Dumped: %d  |  Failed: %d\n", dumped, failed);
    printf("═══════════════════════════════════════════════════\n");
    
    return failed > 0 ? 1 : 0;
}
