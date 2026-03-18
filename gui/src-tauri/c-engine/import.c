/*
 * import.c — File import pipeline implementation
 *
 * Arena allocator + ParseNode AST + format parsers
 */

#include "lrm_import.h"
#include <stdlib.h>
#include <string.h>
#include <strings.h>  /* strcasecmp */
#include <stdio.h>
#include <ctype.h>

/* ═══ ARENA ALLOCATOR ═══════════════════════════════════ */

static ArenaBlock *block_new(size_t cap) {
    ArenaBlock *b = malloc(sizeof(ArenaBlock));
    if (!b) return NULL;
    b->data = malloc(cap);
    if (!b->data) { free(b); return NULL; }
    b->used = 0;
    b->capacity = cap;
    b->next = NULL;
    return b;
}

Arena *arena_create(void) {
    Arena *a = malloc(sizeof(Arena));
    if (!a) return NULL;
    a->first = block_new(ARENA_BLOCK_SIZE);
    a->current = a->first;
    return a;
}

void *arena_alloc(Arena *a, size_t size) {
    /* align to 8 bytes */
    size = (size + 7) & ~7;
    if (a->current->used + size > a->current->capacity) {
        size_t cap = size > ARENA_BLOCK_SIZE ? size : ARENA_BLOCK_SIZE;
        ArenaBlock *b = block_new(cap);
        if (!b) return NULL;
        a->current->next = b;
        a->current = b;
    }
    void *ptr = a->current->data + a->current->used;
    a->current->used += size;
    return ptr;
}

char *arena_strdup(Arena *a, const char *s) {
    size_t len = strlen(s) + 1;
    char *d = arena_alloc(a, len);
    if (d) memcpy(d, s, len);
    return d;
}

void arena_free(Arena *a) {
    ArenaBlock *b = a->first;
    while (b) {
        ArenaBlock *next = b->next;
        free(b->data);
        free(b);
        b = next;
    }
    free(a);
}

/* ═══ PARSENODE HELPERS ═════════════════════════════════ */

ParseNode *node_new(Arena *a, NodeType type, const char *key, const char *value) {
    ParseNode *n = arena_alloc(a, sizeof(ParseNode));
    if (!n) return NULL;
    memset(n, 0, sizeof(ParseNode));
    n->type = type;
    if (key) strncpy(n->key, key, 63);
    if (value) strncpy(n->value, value, 255);
    return n;
}

void node_add_child(ParseNode *parent, ParseNode *child) {
    if (!parent->first_child) {
        parent->first_child = child;
    } else {
        ParseNode *last = parent->first_child;
        while (last->next_sibling) last = last->next_sibling;
        last->next_sibling = child;
    }
}

ParseNode *node_find_child(ParseNode *parent, const char *key) {
    if (!parent) return NULL;
    for (ParseNode *c = parent->first_child; c; c = c->next_sibling)
        if (strcmp(c->key, key) == 0) return c;
    return NULL;
}

const char *node_get_attr(ParseNode *elem, const char *attr_name) {
    if (!elem) return NULL;
    for (ParseNode *c = elem->first_child; c; c = c->next_sibling)
        if (c->type == NODE_ATTR && strcmp(c->key, attr_name) == 0)
            return c->value;
    return NULL;
}

int node_child_count(ParseNode *parent) {
    int n = 0;
    if (parent)
        for (ParseNode *c = parent->first_child; c; c = c->next_sibling) n++;
    return n;
}

/* ═══ XML PARSER (state machine) ════════════════════════ */

/*
 * Minimal XML parser. Handles:
 *   <Tag attr="val" attr2="val2">content</Tag>
 *   <SelfClosing attr="val"/>
 *   <!-- comments -->
 *   <?processing instructions?>
 *
 * Does NOT handle: CDATA, DTD, namespaces, entities (except &amp; &lt; &gt; &quot;)
 * Good enough for system.hw, system.cfg, XLSX internals.
 */

typedef enum {
    XS_TEXT, XS_TAG_OPEN, XS_TAG_NAME, XS_ATTR_NAME,
    XS_ATTR_EQ, XS_ATTR_VAL, XS_TAG_CLOSE, XS_COMMENT
} XmlState;

static void xml_unescape(char *s) {
    char *r = s, *w = s;
    while (*r) {
        if (*r == '&') {
            if (strncmp(r, "&amp;", 5) == 0) { *w++ = '&'; r += 5; }
            else if (strncmp(r, "&lt;", 4) == 0) { *w++ = '<'; r += 4; }
            else if (strncmp(r, "&gt;", 4) == 0) { *w++ = '>'; r += 4; }
            else if (strncmp(r, "&quot;", 6) == 0) { *w++ = '"'; r += 6; }
            else if (strncmp(r, "&apos;", 6) == 0) { *w++ = '\''; r += 6; }
            else { *w++ = *r++; }
        } else {
            *w++ = *r++;
        }
    }
    *w = '\0';
}

ParseNode *parse_xml(Arena *a, const char *buf, size_t len) {
    ParseNode *root = node_new(a, NODE_ROOT, "root", NULL);
    ParseNode *stack[64];      /* parent stack */
    int depth = 0;
    stack[0] = root;

    size_t i = 0;
    char tag_name[64], attr_name[64], attr_val[256], text_buf[256];
    int tn = 0, an = 0, av = 0, tb = 0;
    bool self_closing = false;
    bool is_close_tag = false;
    char quote_char = '"';
    XmlState state = XS_TEXT;

    while (i < len) {
        char c = buf[i];

        switch (state) {
        case XS_TEXT:
            if (c == '<') {
                /* emit accumulated text */
                if (tb > 0) {
                    text_buf[tb] = '\0';
                    /* trim whitespace */
                    char *t = text_buf;
                    while (*t && isspace(*t)) t++;
                    if (*t) {
                        xml_unescape(t);
                        ParseNode *tn_node = node_new(a, NODE_TEXT, "", t);
                        node_add_child(stack[depth], tn_node);
                    }
                    tb = 0;
                }
                state = XS_TAG_OPEN;
                tn = 0;
                is_close_tag = false;
                self_closing = false;
            } else {
                if (tb < 255) text_buf[tb++] = c;
            }
            i++;
            break;

        case XS_TAG_OPEN:
            if (c == '/') {
                is_close_tag = true;
                i++;
                state = XS_TAG_NAME;
            } else if (c == '!' && i + 2 < len && buf[i+1] == '-' && buf[i+2] == '-') {
                state = XS_COMMENT;
                i += 3;
            } else if (c == '?') {
                /* processing instruction — skip to ?> */
                while (i < len - 1 && !(buf[i] == '?' && buf[i+1] == '>')) i++;
                i += 2;
                state = XS_TEXT;
            } else {
                state = XS_TAG_NAME;
            }
            break;

        case XS_COMMENT:
            if (c == '-' && i + 2 < len && buf[i+1] == '-' && buf[i+2] == '>') {
                i += 3;
                state = XS_TEXT;
            } else {
                i++;
            }
            break;

        case XS_TAG_NAME:
            if (isspace(c) || c == '>' || c == '/') {
                tag_name[tn] = '\0';
                if (is_close_tag) {
                    /* close tag — pop stack */
                    if (depth > 0) depth--;
                    /* skip to > */
                    while (i < len && buf[i] != '>') i++;
                    i++;
                    state = XS_TEXT;
                } else {
                    /* open tag — create element, push */
                    ParseNode *elem = node_new(a, NODE_ELEMENT, tag_name, NULL);
                    node_add_child(stack[depth], elem);
                    if (depth < 62) { depth++; stack[depth] = elem; }
                    if (c == '>') {
                        i++;
                        state = XS_TEXT;
                    } else if (c == '/') {
                        self_closing = true;
                        i++;
                        /* expect > */
                        if (i < len && buf[i] == '>') i++;
                        if (depth > 0) depth--;
                        state = XS_TEXT;
                    } else {
                        i++;
                        state = XS_ATTR_NAME;
                        an = 0;
                    }
                }
            } else {
                if (tn < 63) tag_name[tn++] = c;
                i++;
            }
            break;

        case XS_ATTR_NAME:
            if (c == '=') {
                attr_name[an] = '\0';
                i++;
                state = XS_ATTR_EQ;
            } else if (c == '>' || c == '/') {
                if (c == '/') {
                    self_closing = true;
                    i++;
                    if (i < len && buf[i] == '>') i++;
                    if (depth > 0) depth--;
                } else {
                    i++;
                }
                state = XS_TEXT;
            } else if (isspace(c)) {
                i++;
            } else {
                if (an < 63) attr_name[an++] = c;
                i++;
            }
            break;

        case XS_ATTR_EQ:
            if (c == '"' || c == '\'') {
                quote_char = c;
                av = 0;
                i++;
                state = XS_ATTR_VAL;
            } else {
                i++;
            }
            break;

        case XS_ATTR_VAL:
            if (c == quote_char) {
                attr_val[av] = '\0';
                xml_unescape(attr_val);
                ParseNode *attr = node_new(a, NODE_ATTR, attr_name, attr_val);
                node_add_child(stack[depth], attr);
                i++;
                an = 0;
                state = XS_ATTR_NAME;
            } else {
                if (av < 255) attr_val[av++] = c;
                i++;
            }
            break;

        default:
            i++;
            break;
        }
    }

    return root;
}

/* ═══ INI PARSER ════════════════════════════════════════ */

ParseNode *parse_ini(Arena *a, const char *buf, size_t len) {
    ParseNode *root = node_new(a, NODE_ROOT, "root", NULL);
    ParseNode *section = root;

    const char *p = buf, *end = buf + len;
    char line[512];

    while (p < end) {
        /* read line */
        int li = 0;
        while (p < end && *p != '\n' && *p != '\r' && li < 511)
            line[li++] = *p++;
        line[li] = '\0';
        while (p < end && (*p == '\n' || *p == '\r')) p++;

        /* trim */
        char *l = line;
        while (*l && isspace(*l)) l++;
        int le = (int)strlen(l);
        while (le > 0 && isspace(l[le-1])) le--;
        l[le] = '\0';

        if (le == 0 || l[0] == ';' || l[0] == '#') continue;

        if (l[0] == '[') {
            /* section */
            char *e = strchr(l, ']');
            if (e) {
                *e = '\0';
                section = node_new(a, NODE_ELEMENT, l + 1, NULL);
                node_add_child(root, section);
            }
        } else {
            /* key=value or key = value */
            char *eq = strchr(l, '=');
            if (eq) {
                *eq = '\0';
                char *key = l;
                char *val = eq + 1;
                /* trim key and value */
                while (*key && isspace(*key)) key++;
                int kl = (int)strlen(key);
                while (kl > 0 && isspace(key[kl-1])) kl--;
                key[kl] = '\0';
                while (*val && isspace(*val)) val++;
                int vl = (int)strlen(val);
                while (vl > 0 && isspace(val[vl-1])) vl--;
                val[vl] = '\0';

                ParseNode *kv = node_new(a, NODE_ATTR, key, val);
                node_add_child(section, kv);
            }
        }
    }
    return root;
}

/* ═══ CSV PARSER ════════════════════════════════════════ */

ParseNode *parse_csv(Arena *a, const char *buf, size_t len, char delim) {
    ParseNode *root = node_new(a, NODE_ROOT, "root", NULL);
    const char *p = buf, *end = buf + len;
    int row_num = 0;

    while (p < end) {
        char row_key[32];
        snprintf(row_key, 32, "row_%d", row_num);
        ParseNode *row = node_new(a, NODE_ELEMENT, row_key, NULL);
        node_add_child(root, row);

        int col = 0;
        while (p < end && *p != '\n' && *p != '\r') {
            char val[256];
            int vi = 0;
            bool quoted = false;

            if (*p == '"') {
                quoted = true;
                p++;
                while (p < end && vi < 255) {
                    if (*p == '"') {
                        if (p + 1 < end && *(p+1) == '"') {
                            val[vi++] = '"';
                            p += 2;
                        } else {
                            p++;
                            break;
                        }
                    } else {
                        val[vi++] = *p++;
                    }
                }
                if (p < end && *p == delim) p++;
            } else {
                while (p < end && *p != delim && *p != '\n' && *p != '\r' && vi < 255)
                    val[vi++] = *p++;
                if (p < end && *p == delim) p++;
            }
            val[vi] = '\0';

            char col_key[32];
            snprintf(col_key, 32, "col_%d", col);
            ParseNode *cell = node_new(a, NODE_ATTR, col_key, val);
            node_add_child(row, cell);
            col++;
        }
        while (p < end && (*p == '\n' || *p == '\r')) p++;
        row_num++;
    }
    return root;
}

/* ═══ CUSTOM MAP PARSER ═════════════════════════════════ */
/* Handles:
 *   "B13_GPIO0 GPIO_1_C3;"    (iliad.map style)
 *   "0 GPIO_1_C3"             (PIN_MAP style)
 * One entry per line, whitespace separated, optional semicolon */

ParseNode *parse_map(Arena *a, const char *buf, size_t len) {
    ParseNode *root = node_new(a, NODE_ROOT, "root", NULL);
    const char *p = buf, *end = buf + len;

    while (p < end) {
        /* skip whitespace/newlines */
        while (p < end && (*p == '\n' || *p == '\r' || *p == ' ' || *p == '\t')) p++;
        if (p >= end) break;
        if (*p == '#') { /* comment */
            while (p < end && *p != '\n') p++;
            continue;
        }

        /* read first token (key) */
        char key[64]; int ki = 0;
        while (p < end && !isspace(*p) && *p != ';' && ki < 63)
            key[ki++] = *p++;
        key[ki] = '\0';

        /* skip whitespace */
        while (p < end && (*p == ' ' || *p == '\t')) p++;

        /* read second token (value) — may be empty */
        char val[256]; int vi = 0;
        while (p < end && *p != '\n' && *p != '\r' && *p != ';' && vi < 255)
            val[vi++] = *p++;
        val[vi] = '\0';
        /* trim trailing whitespace from value */
        while (vi > 0 && isspace(val[vi-1])) val[--vi] = '\0';

        /* skip rest of line */
        while (p < end && *p != '\n') p++;

        if (ki > 0) {
            ParseNode *entry = node_new(a, NODE_ATTR, key, val);
            node_add_child(root, entry);
        }
    }
    return root;
}

/* ═══ FILE UTILITY ══════════════════════════════════════ */

char *read_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (sz <= 0) { fclose(f); return NULL; }
    char *buf = malloc(sz + 1);
    if (!buf) { fclose(f); return NULL; }
    size_t n = fread(buf, 1, sz, f);
    buf[n] = '\0';
    fclose(f);
    *out_len = n;
    return buf;
}

/* ═══ FORMAT DETECTION ══════════════════════════════════ */

FileFormat detect_format(const char *filepath, const char *buf, size_t len) {
    /* check extension first */
    const char *dot = strrchr(filepath, '.');
    if (dot) {
        if (strcasecmp(dot, ".xml") == 0 || strcasecmp(dot, ".hw") == 0 ||
            strcasecmp(dot, ".cfg") == 0)
            return FMT_XML;
        if (strcasecmp(dot, ".ini") == 0)
            return FMT_INI;
        if (strcasecmp(dot, ".csv") == 0)
            return FMT_CSV;
        if (strcasecmp(dot, ".tsv") == 0)
            return FMT_TSV;
        if (strcasecmp(dot, ".map") == 0)
            return FMT_MAP;
    }
    /* check content */
    if (len > 0) {
        const char *p = buf;
        while (*p && isspace(*p)) p++;
        if (*p == '<') return FMT_XML;
        if (*p == '[') return FMT_INI;
    }
    /* check for PIN_MAP style (starts with digits) */
    if (len > 0 && isdigit(buf[0])) return FMT_MAP;
    return FMT_UNKNOWN;
}

/* ═══ SMART MAPPERS ═════════════════════════════════════ */

/*
 * map_system_hw: parse system.hw XML → create System + locations
 *
 * Expected structure:
 * <SystemHardware>
 *   <Chambers>
 *     <Chamber style="..." type="..." serial="...">
 *       <Backplanes>
 *         <Backplane type="Slot" [duplicates="N"]>
 *           <Driver slots="M"/>
 *         </Backplane>
 *       </Backplanes>
 *     </Chamber>
 *   </Chambers>
 * </SystemHardware>
 */
int map_system_hw(Database *db, ParseNode *root, int64_t user_id) {
    ParseNode *hw = node_find_child(root, "SystemHardware");
    if (!hw) return LRM_ERR_SCHEMA;

    ParseNode *chambers = node_find_child(hw, "Chambers");
    if (!chambers) return LRM_ERR_SCHEMA;

    /* count chambers and analyze backplane structure */
    int chamber_count = 0;
    int max_backplanes = 0;
    int max_slots = 0;

    for (ParseNode *ch = chambers->first_child; ch; ch = ch->next_sibling) {
        if (ch->type != NODE_ELEMENT || strcmp(ch->key, "Chamber") != 0) continue;
        chamber_count++;

        ParseNode *bps = node_find_child(ch, "Backplanes");
        if (!bps) continue;

        int bp_count = 0;
        int bp_slots = 0;
        for (ParseNode *bp = bps->first_child; bp; bp = bp->next_sibling) {
            if (bp->type != NODE_ELEMENT || strcmp(bp->key, "Backplane") != 0) continue;
            const char *dups = node_get_attr(bp, "duplicates");
            int n = dups ? atoi(dups) : 1;
            bp_count += n;

            ParseNode *driver = node_find_child(bp, "Driver");
            if (driver) {
                const char *slots = node_get_attr(driver, "slots");
                if (slots) bp_slots = atoi(slots);
            }
        }
        if (bp_count > max_backplanes) max_backplanes = bp_count;
        if (bp_slots > max_slots) max_slots = bp_slots;
    }

    if (chamber_count == 0) return LRM_ERR_SCHEMA;

    /* Determine system type from structure */
    const char *hw_file = node_get_attr(hw, "file");
    const char *settings_name = NULL;
    ParseNode *settings = node_find_child(hw, "SystemSettings");
    if (!settings) {
        /* look in system.cfg style */
        settings = node_find_child(root, "SystemConfiguration");
    }

    /* Create system */
    System sys = {0};
    /* use settings name or generic */
    if (settings) {
        settings_name = node_get_attr(settings, "name");
    }
    if (settings_name) {
        strncpy(sys.name, settings_name, MAX_TEXT_LEN - 1);
    } else {
        snprintf(sys.name, MAX_TEXT_LEN, "System-Imported");
    }

    /* detect type: MCC if has backplanes w/ slots, Sonoma if has shelves/trays */
    sys.system_type = SYS_MCC;  /* default for backplane systems */
    sys.cooling = COOL_AIR;
    sys.chamber_count = chamber_count;
    sys.shelves_per_chamber = max_backplanes > 0 ? max_backplanes : 1;
    sys.slots_per_shelf = max_slots > 0 ? max_slots : 1;

    int rc = lrm_create_system(db, &sys, user_id);
    if (rc != LRM_OK) return rc;

    /* Generate location tree */
    rc = lrm_generate_system_tree(db, sys.system_id, user_id);
    if (rc != LRM_OK) return rc;

    /* Parse temperature control info */
    for (ParseNode *ch = chambers->first_child; ch; ch = ch->next_sibling) {
        if (ch->type != NODE_ELEMENT || strcmp(ch->key, "Chamber") != 0) continue;
        ParseNode *temp = node_find_child(ch, "Temperature");
        if (temp) {
            ParseNode *ctrl = node_find_child(temp, "Control");
            if (ctrl) {
                const char *type = node_get_attr(ctrl, "type");
                if (type) {
                    /* store as system note */
                    System s;
                    lrm_get_system(db, sys.system_id, &s);
                    snprintf(s.notes, MAX_TEXT_LEN, "TempCtrl: %s", type);
                    lrm_update_system(db, &s, user_id);
                }
            }
        }
    }

    return LRM_OK;
}

/* map_run_ini: parse run.ini → system IP config, zone counts */
int map_run_ini(Database *db, ParseNode *root, int64_t system_id, int64_t user_id) {
    System sys;
    int rc = lrm_get_system(db, system_id, &sys);
    if (rc != LRM_OK) return rc;

    /* Look for TCP config */
    ParseNode *tcp = node_find_child(root, "TcpipCfg");
    if (tcp) {
        const char *watlow_ip = node_get_attr(tcp, "WATLOW_IP");
        const char *plc_ip = node_get_attr(tcp, "PLC_IP");
        if (watlow_ip || plc_ip) {
            char notes[MAX_TEXT_LEN];
            snprintf(notes, MAX_TEXT_LEN, "Watlow:%s PLC:%s",
                     watlow_ip ? watlow_ip : "N/A",
                     plc_ip ? plc_ip : "N/A");
            /* append to existing notes */
            size_t existing = strlen(sys.notes);
            if (existing > 0 && existing < MAX_TEXT_LEN - 2) {
                sys.notes[existing] = ' ';
                strncpy(sys.notes + existing + 1, notes,
                        MAX_TEXT_LEN - existing - 2);
            } else {
                strncpy(sys.notes, notes, MAX_TEXT_LEN - 1);
            }
        }
    }

    /* Look for pattern zones */
    ParseNode *pz = node_find_child(root, "PATTERN_ZONE");
    if (pz) {
        const char *num = node_get_attr(pz, "NUM_OF_PZONE");
        if (num) {
            char buf[64];
            snprintf(buf, 64, " PZones:%s", num);
            size_t e = strlen(sys.notes);
            if (e < MAX_TEXT_LEN - 64)
                strncat(sys.notes, buf, MAX_TEXT_LEN - e - 1);
        }
    }

    return lrm_update_system(db, &sys, user_id);
}

/* map_pin_file: parse .map file → append summary to system.notes
 * Full pin_maps table would avoid MAX_TEXT_LEN cap; until then we persist
 * entry count + truncated k=v list so import is auditable and grep-friendly. */
int map_pin_file(Database *db, ParseNode *root, int64_t system_id, int64_t user_id) {
    if (!db || !root || system_id <= 0) return LRM_ERR_SCHEMA;

    System sys;
    int rc = lrm_get_system(db, system_id, &sys);
    if (rc != LRM_OK) return rc;

    int n = 0;
    for (ParseNode *c = root->first_child; c; c = c->next_sibling)
        if (c->type == NODE_ATTR && c->key[0]) n++;

    char block[MAX_TEXT_LEN];
    int pos = snprintf(block, sizeof(block), " [pin_map:%d]", n);
    if (pos < 0 || (size_t)pos >= sizeof(block)) pos = (int)sizeof(block) - 1;

    /* Append up to ~12 pairs so notes stay useful without blowing MAX_TEXT_LEN */
    int shown = 0;
    for (ParseNode *c = root->first_child; c && shown < 12; c = c->next_sibling) {
        if (c->type != NODE_ATTR || !c->key[0]) continue;
        int left = (int)sizeof(block) - pos - 4;
        if (left < 8) break;
        int w = snprintf(block + pos, (size_t)left, " %s=%s;", c->key, c->value);
        if (w > 0 && pos + w < (int)sizeof(block) - 1) {
            pos += w;
            shown++;
        } else
            break;
    }
    if (n > shown && pos < (int)sizeof(block) - 20)
        snprintf(block + pos, sizeof(block) - (size_t)pos, " ...+%d", n - shown);

    /* Always null-terminate: strncpy may omit \0 when truncating */
    size_t existing = strlen(sys.notes);
    if (existing > 0 && existing < MAX_TEXT_LEN - 2) {
        sys.notes[existing] = ' ';
        strncpy(sys.notes + existing + 1, block, MAX_TEXT_LEN - existing - 2);
    } else {
        /* Replace notes when empty or already full — avoid unbounded growth */
        strncpy(sys.notes, block, MAX_TEXT_LEN - 1);
    }
    sys.notes[MAX_TEXT_LEN - 1] = '\0';
    return lrm_update_system(db, &sys, user_id);
}

/* map_system_cfg: parse system.cfg XML → environments */
int map_system_cfg(Database *db, ParseNode *root, int64_t system_id, int64_t user_id) {
    (void)db; (void)root; (void)system_id; (void)user_id;
    /* TODO: parse environments, temp profiles */
    return LRM_OK;
}

/* map_csv_inventory: auto-detect columns from header row */
int map_csv_inventory(Database *db, ParseNode *root, int64_t user_id) {
    (void)db; (void)root; (void)user_id;
    /* TODO: detect column names, map to appropriate lrm_create_*() calls */
    return LRM_OK;
}

/* ═══ FILE DISPATCHER ═══════════════════════════════════ */

int lrm_import_file(Database *db, const char *filepath,
                    int64_t system_id, int64_t user_id) {
    size_t len = 0;
    char *buf = read_file(filepath, &len);
    if (!buf) return LRM_ERR_IO;

    FileFormat fmt = detect_format(filepath, buf, len);
    Arena *a = arena_create();
    if (!a) { free(buf); return LRM_ERR_IO; }

    ParseNode *ast = NULL;
    int rc = LRM_ERR_SCHEMA;

    switch (fmt) {
    case FMT_XML:
        ast = parse_xml(a, buf, len);
        if (ast) {
            /* try system.hw first, then system.cfg */
            ParseNode *hw = node_find_child(ast, "SystemHardware");
            if (hw) {
                rc = map_system_hw(db, ast, user_id);
            } else {
                ParseNode *cfg = node_find_child(ast, "SystemConfiguration");
                if (cfg) {
                    rc = map_system_cfg(db, ast, system_id, user_id);
                }
            }
        }
        break;

    case FMT_INI:
        ast = parse_ini(a, buf, len);
        if (ast && system_id > 0)
            rc = map_run_ini(db, ast, system_id, user_id);
        break;

    case FMT_CSV:
        ast = parse_csv(a, buf, len, ',');
        if (ast) rc = map_csv_inventory(db, ast, user_id);
        break;

    case FMT_TSV:
        ast = parse_csv(a, buf, len, '\t');
        if (ast) rc = map_csv_inventory(db, ast, user_id);
        break;

    case FMT_MAP:
        ast = parse_map(a, buf, len);
        if (ast && system_id > 0)
            rc = map_pin_file(db, ast, system_id, user_id);
        break;

    default:
        rc = LRM_ERR_SCHEMA;
        break;
    }

    arena_free(a);
    free(buf);
    return rc;
}
