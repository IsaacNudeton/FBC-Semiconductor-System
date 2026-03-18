/*
 * lrm_http.h — Minimal HTTP/JSON API for the inventory engine
 *
 * Zero dependencies. Pure C sockets (POSIX).
 * Single-threaded event loop — fine for <20 concurrent operators.
 *
 * Routes:
 *   GET  /api/systems                    → list all systems
 *   GET  /api/systems/:id                → get system
 *   POST /api/systems                    → create system
 *   POST /api/systems/:id/status         → change status
 *   POST /api/systems/:id/generate-tree  → auto-generate locations
 *
 *   GET  /api/locations/:id/children     → list children
 *
 *   GET  /api/devices                    → list all devices
 *   POST /api/devices                    → create device
 *
 *   GET  /api/projects                   → list all projects
 *   GET  /api/projects/:id               → get project
 *   POST /api/projects                   → create project
 *   GET  /api/projects/number/:num       → find by S00## number
 *
 *   GET  /api/lots/:id                   → get lot
 *   POST /api/lots                       → create lot
 *   POST /api/lots/:id/advance           → advance workflow step
 *   POST /api/lots/:id/qty              → update quantities
 *   GET  /api/lots/project/:pid          → lots for project
 *
 *   GET  /api/hardware/serial/:serial    → find by barcode scan
 *   GET  /api/hardware/system/:sid       → list HW on system
 *   GET  /api/hardware/project/:pid      → list HW for project
 *   POST /api/hardware                   → create serialized item
 *   POST /api/hardware/:id/move          → move item
 *   POST /api/hardware/:id/assign        → assign to project
 *   POST /api/hardware/:id/unassign      → unassign
 *   POST /api/hardware/:id/status        → change status
 *
 *   GET  /api/quantity/system/:sid       → quantity HW on system
 *   POST /api/quantity                   → set quantity
 *   POST /api/quantity/:id/adjust        → adjust good/bad
 *
 *   POST /api/configured                 → create configured HW
 *   GET  /api/configured/project/:pid    → list for project
 *
 *   GET  /api/hw-types                   → list hardware types
 *   POST /api/hw-types                   → create hardware type
 *
 *   GET  /api/audit/recent               → recent audit entries
 *
 *   POST /api/auth/login                 → authenticate
 *   GET  /api/users                      → list users
 *   POST /api/users                      → create user
 */

#ifndef LRM_HTTP_H
#define LRM_HTTP_H

#include "lrm_db.h"
#include "lrm_schema.h"

#define HTTP_PORT       8080
#define HTTP_MAX_BODY   262144
#define HTTP_MAX_PATH   512
#define HTTP_MAX_HEADER 4096

typedef struct {
    Database *db;
    int       port;
    int       server_fd;
    int       running;
} HttpServer;

/* Start the HTTP server (blocks) */
int http_serve(HttpServer *srv, Database *db, int port);

/* Stop the server */
void http_stop(HttpServer *srv);

#endif /* LRM_HTTP_H */
