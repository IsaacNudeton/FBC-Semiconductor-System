/*
 * lrm_import.h — File import pipeline
 *
 * Architecture: FILE → PARSER → AST → MAPPER → ENGINE
 *
 * Parsers are dumb: bytes → ParseNode tree
 * Mappers are smart: ParseNode tree → lrm_create_*() calls
 * Arena allocator: parse, map, free — no leak tracking
 */

#ifndef LRM_IMPORT_H
#define LRM_IMPORT_H

#include "lrm_db.h"
#include "lrm_schema.h"
#include <stddef.h>

/* ── Arena Allocator ────────────────────────────────────── */

#define ARENA_BLOCK_SIZE (64 * 1024)  /* 64KB blocks */

typedef struct ArenaBlock {
    uint8_t *data;
    size_t   used;
    size_t   capacity;
    struct ArenaBlock *next;
} ArenaBlock;

typedef struct {
    ArenaBlock *first;
    ArenaBlock *current;
} Arena;

Arena *arena_create(void);
void  *arena_alloc(Arena *a, size_t size);
char  *arena_strdup(Arena *a, const char *s);
void   arena_free(Arena *a);

/* ── ParseNode AST ──────────────────────────────────────── */

typedef enum {
    NODE_ROOT    = 0,
    NODE_ELEMENT = 1,   /* XML tag, INI section */
    NODE_ATTR    = 2,   /* XML attribute, INI key=value, CSV cell */
    NODE_TEXT    = 3    /* raw string content */
} NodeType;

typedef struct ParseNode {
    NodeType type;
    char     key[64];
    char     value[256];
    struct ParseNode *first_child;
    struct ParseNode *next_sibling;
} ParseNode;

/* Create nodes via arena */
ParseNode *node_new(Arena *a, NodeType type, const char *key, const char *value);
void       node_add_child(ParseNode *parent, ParseNode *child);

/* ── Dumb Parsers ───────────────────────────────────────── */

/* XML: handles tags, attributes, text content, self-closing tags.
 * Not a full XML parser — no CDATA, no DTD, no namespaces.
 * Enough for system.hw, system.cfg, and XLSX inner XML. */
ParseNode *parse_xml(Arena *a, const char *buf, size_t len);

/* INI: sections [Name], key=value pairs, comments (;#) */
ParseNode *parse_ini(Arena *a, const char *buf, size_t len);

/* CSV/TSV: delimiter-separated values with optional quoting */
ParseNode *parse_csv(Arena *a, const char *buf, size_t len, char delim);

/* Custom map: "SIGNAL_NAME VALUE" or "NUMBER SIGNAL_NAME" per line */
ParseNode *parse_map(Arena *a, const char *buf, size_t len);

/* ── Smart Mappers ──────────────────────────────────────── */

/* system.hw → systems + locations via lrm_generate_system_tree()
 * Reads <Chamber>, <Backplanes>, <Backplane duplicates="N" slots="M"> */
int map_system_hw(Database *db, ParseNode *root, int64_t user_id);

/* system.cfg → system settings (environments, temp profiles) */
int map_system_cfg(Database *db, ParseNode *root, int64_t system_id, int64_t user_id);

/* run.ini → system IP, temp zones, pattern zones */
int map_run_ini(Database *db, ParseNode *root, int64_t system_id, int64_t user_id);

/* .map files → pin mappings (stored as configured_hw or notes) */
int map_pin_file(Database *db, ParseNode *root, int64_t system_id, int64_t user_id);

/* CSV import → serialized_hw, quantity_hw, lots (auto-detect columns) */
int map_csv_inventory(Database *db, ParseNode *root, int64_t user_id);

/* ── File Dispatcher ────────────────────────────────────── */

typedef enum {
    FMT_UNKNOWN = 0,
    FMT_XML     = 1,
    FMT_INI     = 2,
    FMT_CSV     = 3,
    FMT_TSV     = 4,
    FMT_MAP     = 5,
} FileFormat;

FileFormat detect_format(const char *filepath, const char *buf, size_t len);

/* Main entry point: load file, detect format, parse, map, free */
int lrm_import_file(Database *db, const char *filepath,
                    int64_t system_id, int64_t user_id);

/* ── Utility ────────────────────────────────────────────── */

/* Read entire file into malloc'd buffer. Caller frees. */
char *read_file(const char *path, size_t *out_len);

/* Find child element by key (first match) */
ParseNode *node_find_child(ParseNode *parent, const char *key);

/* Get attribute value from element's children */
const char *node_get_attr(ParseNode *elem, const char *attr_name);

/* Count direct children of a node */
int node_child_count(ParseNode *parent);

#endif /* LRM_IMPORT_H */
