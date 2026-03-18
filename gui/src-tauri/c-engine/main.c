/*
 * main.c — LRM Inventory Server
 *
 * Usage: ./lrm-server [port] [db_path]
 *   Default: port 8080, db ./lrm_inventory.db
 *
 * On first run: creates database, registers 21 tables, seeds admin user.
 * On subsequent runs: opens existing database, re-registers schema (indexes rebuilt).
 */

#include "lrm_db.h"
#include "lrm_schema.h"
#include "lrm_http.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char *argv[]) {
    int port = HTTP_PORT;
    const char *db_path = "lrm_inventory.db";

    if (argc > 1) port = atoi(argv[1]);
    if (argc > 2) db_path = argv[2];
    if (port <= 0 || port > 65535) port = HTTP_PORT;

    printf("╔═══════════════════════════════════════════════════════╗\n");
    printf("║  ISE Labs — Burn-In Inventory Server                 ║\n");
    printf("╠═══════════════════════════════════════════════════════╣\n");
    printf("║  Database: %-42s ║\n", db_path);
    printf("║  Port:     %-42d ║\n", port);
    printf("╚═══════════════════════════════════════════════════════╝\n\n");

    /* Open or create database */
    Database db;
    int rc = db_open(&db, db_path);
    if (rc != LRM_OK) {
        fprintf(stderr, "Failed to open database: %s (error %d)\n", db_path, rc);
        if (rc == LRM_ERR_IO)
            fprintf(stderr, "  Hint: another lrm-server may be running on the same database.\n");
        return 1;
    }

    /* Register schema (21 tables) */
    rc = schema_init(&db);
    if (rc == LRM_ERR_CORRUPT) {
        /* Schema changed since DB was created — auto-recreate */
        fprintf(stderr, "[init] Schema mismatch detected. Recreating database...\n");
        db_close(&db);
        remove(db_path);
        char wal_path[512];
        snprintf(wal_path, sizeof(wal_path), "%s.wal", db_path);
        remove(wal_path);
        rc = db_open(&db, db_path);
        if (rc != LRM_OK) {
            fprintf(stderr, "Failed to recreate database (error %d)\n", rc);
            return 1;
        }
        rc = schema_init(&db);
    }
    if (rc != LRM_OK) {
        fprintf(stderr, "Failed to initialize schema (error %d)\n", rc);
        db_close(&db);
        return 1;
    }

    /* Seed default admin if no users exist */
    User users[1]; uint32_t user_count = 0;
    lrm_list_users(&db, users, &user_count, 1);
    if (user_count == 0) {
        printf("[init] Creating default admin user (admin/admin)\n");
        User admin = {0};
        strncpy(admin.username, "admin", MAX_TEXT_LEN);
        strncpy(admin.display_name, "Administrator", MAX_TEXT_LEN);
        admin.role = ROLE_ADMIN;
        lrm_create_user(&db, &admin, "admin");
        printf("[init] Default admin created. Change password on first login.\n\n");
    }

    /* Start HTTP server */
    HttpServer srv = {0};
    rc = http_serve(&srv, &db, port);

    /* Cleanup */
    db_close(&db);
    printf("[shutdown] Database closed. Goodbye.\n");
    return rc;
}
