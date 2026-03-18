/*
 * onetwo.c — ONETWO Reasoning Scaffold
 *
 * State machine for structured thinking.
 * Tracks phase (ONE/TWO), claims, verification, session log.
 * Enforces: you don't build until you've decomposed.
 * Enforces: you don't ship until you've verified.
 *
 * This tool doesn't think. Claude thinks.
 * This tool tracks what Claude has and hasn't done,
 * and flags when steps are being skipped.
 *
 * Compile: gcc -O3 -o onetwo onetwo.c
 * Install: cp onetwo ~/.local/bin/
 *
 * Commands:
 *   onetwo init <problem>          Start new problem session
 *   onetwo known <fact>            Register something known
 *   onetwo unknown <gap>           Register something unknown
 *   onetwo probe <finding>         Log a ONE-phase finding
 *   onetwo bedrock                 Declare bedrock reached (enables TWO)
 *   onetwo claim <stmt> <tier>     Register a claim (T1-T4)
 *   onetwo verify <id> <pass|fail> Mark claim verified or failed
 *   onetwo log <message>           Append to session log
 *   onetwo check                   Pre-ship checklist
 *   onetwo status                  Current session state
 *   onetwo history                 Full session log
 *   onetwo reset                   Clear session
 *
 * Session: ~/.xyzt/onetwo_session.bin
 *
 * Isaac & Claude — February 2026
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include "wire.h"

/* ═══════════════════════════════════════════════════════
   Constants
   ═══════════════════════════════════════════════════════ */

#define MAX_TEXT      256
#define MAX_KNOWNS    64
#define MAX_UNKNOWNS  64
#define MAX_PROBES    64
#define MAX_CLAIMS    64
#define MAX_LOG       256

#define SESSION_MAGIC  0x4F4E4532  /* "ONE2" */
#define SESSION_VER    1

/* Phases */
#define PHASE_EMPTY    0  /* no problem loaded */
#define PHASE_ONE      1  /* decomposing */
#define PHASE_BEDROCK  2  /* bedrock reached, TWO enabled */
#define PHASE_TWO      3  /* building */
#define PHASE_VERIFY   4  /* verifying claims */
#define PHASE_DONE     5  /* all claims verified */

static const char *PHASE_NAME[] = {
    "EMPTY", "ONE", "BEDROCK", "TWO", "VERIFY", "DONE"
};

/* Claim tiers */
#define TIER_T1  1  /* proven */
#define TIER_T2  2  /* strong signal */
#define TIER_T3  3  /* falsifiable, untested */
#define TIER_T4  4  /* interpretation */

/* Verification status */
#define VSTAT_PENDING   0
#define VSTAT_PASS      1
#define VSTAT_FAIL      2
#define VSTAT_SKIP      3  /* explicitly skipped with reason */

static const char *VSTAT_NAME[] = { "PENDING", "PASS", "FAIL", "SKIP" };

/* ═══════════════════════════════════════════════════════
   Data structures
   ═══════════════════════════════════════════════════════ */

typedef struct {
    char text[MAX_TEXT];
    uint32_t timestamp;
} Entry;

typedef struct {
    char text[MAX_TEXT];
    uint8_t tier;        /* T1-T4 */
    uint8_t verified;    /* VSTAT_* */
    uint8_t _pad[2];
    uint32_t timestamp;
} Claim;

typedef struct {
    char text[MAX_TEXT];
    uint32_t timestamp;
    uint8_t  type;       /* 0=general, 1=decision, 2=correction, 3=finding */
    uint8_t  _pad[3];
} LogEntry;

typedef struct {
    /* Header */
    uint32_t magic;
    uint8_t  version;
    uint8_t  phase;
    uint8_t  _pad[2];
    uint32_t created;
    uint32_t modified;

    /* Problem */
    char problem[MAX_TEXT];

    /* ONE phase */
    Entry   knowns[MAX_KNOWNS];
    uint32_t n_knowns;

    Entry   unknowns[MAX_UNKNOWNS];
    uint32_t n_unknowns;

    Entry   probes[MAX_PROBES];
    uint32_t n_probes;

    /* TWO phase */
    Claim   claims[MAX_CLAIMS];
    uint32_t n_claims;

    /* Log */
    LogEntry log[MAX_LOG];
    uint32_t n_log;

} Session;

/* ═══════════════════════════════════════════════════════
   Time
   ═══════════════════════════════════════════════════════ */

static uint32_t now_ts(void) { return (uint32_t)time(NULL); }

static const char *fmt_ts(uint32_t ts) {
    static char buf[32];
    if (ts == 0) { strcpy(buf, "---"); return buf; }
    time_t t = (time_t)ts;
    struct tm *tm = localtime(&t);
    strftime(buf, sizeof(buf), "%m/%d %H:%M", tm);
    return buf;
}

static const char *fmt_ago(uint32_t ts) {
    static char buf[32];
    if (ts == 0) { strcpy(buf, "never"); return buf; }
    uint32_t delta = now_ts() - ts;
    if (delta < 60)        snprintf(buf, 32, "%ds ago", delta);
    else if (delta < 3600) snprintf(buf, 32, "%dm ago", delta / 60);
    else if (delta < 86400)snprintf(buf, 32, "%dh ago", delta / 3600);
    else                   snprintf(buf, 32, "%dd ago", delta / 86400);
    return buf;
}

/* ═══════════════════════════════════════════════════════
   Session persistence
   ═══════════════════════════════════════════════════════ */

#define DEFAULT_SESSION ".xyzt/onetwo_session.bin"

static const char *get_session_path(void) {
    const char *env = getenv("ONETWO_SESSION");
    return env ? env : DEFAULT_SESSION;
}

static Session *session_new(void) {
    Session *s = calloc(1, sizeof(Session));
    s->magic = SESSION_MAGIC;
    s->version = SESSION_VER;
    s->phase = PHASE_EMPTY;
    s->created = now_ts();
    s->modified = now_ts();
    return s;
}

static Session *session_load(const char *path) {
    FILE *fp = fopen(path, "rb");
    if (!fp) return session_new();

    Session *s = calloc(1, sizeof(Session));
    if (fread(s, sizeof(Session), 1, fp) != 1 || s->magic != SESSION_MAGIC) {
        free(s);
        fclose(fp);
        return session_new();
    }
    fclose(fp);
    return s;
}

static int session_save(Session *s, const char *path) {
    /* Ensure directory */
    char dir[512];
    strncpy(dir, path, 511);
    char *slash = strrchr(dir, '/');
    if (slash) {
        *slash = '\0';
        char cmd[600];
        snprintf(cmd, sizeof(cmd), "mkdir -p %s", dir);
        system(cmd);
    }

    s->modified = now_ts();

    FILE *fp = fopen(path, "wb");
    if (!fp) { fprintf(stderr, "Cannot write: %s\n", path); return -1; }
    fwrite(s, sizeof(Session), 1, fp);
    fclose(fp);
    return 0;
}

/* ═══════════════════════════════════════════════════════
   Log helper
   ═══════════════════════════════════════════════════════ */

static void session_log(Session *s, uint8_t type, const char *msg) {
    if (s->n_log >= MAX_LOG) {
        /* Shift: drop oldest 64 entries */
        memmove(s->log, s->log + 64, (MAX_LOG - 64) * sizeof(LogEntry));
        s->n_log -= 64;
    }
    LogEntry *e = &s->log[s->n_log++];
    strncpy(e->text, msg, MAX_TEXT - 1);
    e->timestamp = now_ts();
    e->type = type;
}

/* ═══════════════════════════════════════════════════════
   Concat argv into single string
   ═══════════════════════════════════════════════════════ */

static char *concat_args(int argc, char **argv, int start) {
    static char buf[MAX_TEXT];
    buf[0] = '\0';
    int pos = 0;
    for (int i = start; i < argc && pos < MAX_TEXT - 2; i++) {
        if (i > start) buf[pos++] = ' ';
        int len = strlen(argv[i]);
        if (pos + len >= MAX_TEXT - 1) len = MAX_TEXT - 1 - pos;
        memcpy(buf + pos, argv[i], len);
        pos += len;
    }
    buf[pos] = '\0';
    return buf;
}

/* ═══════════════════════════════════════════════════════
   Commands
   ═══════════════════════════════════════════════════════ */

static void cmd_init(Session *s, const char *problem) {
    /* Reset everything */
    memset(s, 0, sizeof(Session));
    s->magic = SESSION_MAGIC;
    s->version = SESSION_VER;
    s->phase = PHASE_ONE;
    s->created = now_ts();
    s->modified = now_ts();
    strncpy(s->problem, problem, MAX_TEXT - 1);

    char msg[MAX_TEXT + 32];
    snprintf(msg, sizeof(msg), "INIT: %s", problem);
    session_log(s, 1, msg);

    printf("---ONETWO---\n");
    printf("phase: ONE\n");
    printf("problem: %s\n", s->problem);
    printf("action: decompose before building\n");
    printf("---END---\n");
}

static void cmd_known(Session *s, const char *fact) {
    if (s->phase == PHASE_EMPTY) {
        printf("ERROR: no problem loaded. Run: onetwo init <problem>\n");
        return;
    }
    if (s->n_knowns >= MAX_KNOWNS) {
        printf("ERROR: max knowns reached (%d)\n", MAX_KNOWNS);
        return;
    }
    Entry *e = &s->knowns[s->n_knowns++];
    strncpy(e->text, fact, MAX_TEXT - 1);
    e->timestamp = now_ts();
    session_log(s, 3, fact);
    printf("known[%d]: %s\n", s->n_knowns - 1, fact);
}

static void cmd_unknown(Session *s, const char *gap) {
    if (s->phase == PHASE_EMPTY) {
        printf("ERROR: no problem loaded. Run: onetwo init <problem>\n");
        return;
    }
    if (s->n_unknowns >= MAX_UNKNOWNS) {
        printf("ERROR: max unknowns reached (%d)\n", MAX_UNKNOWNS);
        return;
    }
    Entry *e = &s->unknowns[s->n_unknowns++];
    strncpy(e->text, gap, MAX_TEXT - 1);
    e->timestamp = now_ts();
    session_log(s, 3, gap);
    printf("unknown[%d]: %s\n", s->n_unknowns - 1, gap);
}

static void cmd_probe(Session *s, const char *finding) {
    if (s->phase == PHASE_EMPTY) {
        printf("ERROR: no problem loaded. Run: onetwo init <problem>\n");
        return;
    }
    if (s->n_probes >= MAX_PROBES) {
        printf("ERROR: max probes reached (%d)\n", MAX_PROBES);
        return;
    }
    Entry *e = &s->probes[s->n_probes++];
    strncpy(e->text, finding, MAX_TEXT - 1);
    e->timestamp = now_ts();
    session_log(s, 3, finding);
    printf("probe[%d]: %s\n", s->n_probes - 1, finding);
}

static void cmd_bedrock(Session *s) {
    if (s->phase == PHASE_EMPTY) {
        printf("ERROR: no problem loaded.\n");
        return;
    }

    /* Warnings, not blockers */
    int warnings = 0;
    if (s->n_knowns == 0) {
        printf("WARNING: no knowns registered. Did you skip ONE?\n");
        warnings++;
    }
    if (s->n_unknowns == 0) {
        printf("WARNING: no unknowns registered. Everything is known? Really?\n");
        warnings++;
    }
    if (s->n_probes == 0) {
        printf("WARNING: no probes logged. Decomposition without findings.\n");
        warnings++;
    }

    s->phase = PHASE_BEDROCK;
    session_log(s, 1, "BEDROCK DECLARED");

    printf("---ONETWO---\n");
    printf("phase: BEDROCK → TWO enabled\n");
    printf("knowns: %d\n", s->n_knowns);
    printf("unknowns: %d\n", s->n_unknowns);
    printf("probes: %d\n", s->n_probes);
    if (warnings > 0)
        printf("warnings: %d (decomposition may be shallow)\n", warnings);
    printf("action: build from bedrock up\n");
    printf("---END---\n");
}

static void cmd_claim(Session *s, const char *stmt, int tier) {
    if (s->phase < PHASE_BEDROCK) {
        printf("WARNING: claiming before bedrock. Premature TWO detected.\n");
        session_log(s, 2, "PREMATURE TWO: claim before bedrock");
    }
    if (s->n_claims >= MAX_CLAIMS) {
        printf("ERROR: max claims reached (%d)\n", MAX_CLAIMS);
        return;
    }
    if (tier < 1 || tier > 4) {
        printf("ERROR: tier must be 1-4 (T1=proven, T2=strong, T3=falsifiable, T4=interpretation)\n");
        return;
    }

    Claim *c = &s->claims[s->n_claims++];
    strncpy(c->text, stmt, MAX_TEXT - 1);
    c->tier = tier;
    c->verified = VSTAT_PENDING;
    c->timestamp = now_ts();

    if (s->phase == PHASE_BEDROCK) s->phase = PHASE_TWO;

    char msg[MAX_TEXT + 16];
    snprintf(msg, sizeof(msg), "CLAIM T%d: %s", tier, stmt);
    session_log(s, 3, msg);

    printf("claim[%d] T%d: %s\n", s->n_claims - 1, tier, stmt);
}

static void cmd_verify(Session *s, int id, int pass) {
    if (id < 0 || id >= (int)s->n_claims) {
        printf("ERROR: invalid claim id %d (have %d claims)\n", id, s->n_claims);
        return;
    }
    Claim *c = &s->claims[id];
    c->verified = pass ? VSTAT_PASS : VSTAT_FAIL;

    char msg[MAX_TEXT + 32];
    snprintf(msg, sizeof(msg), "VERIFY[%d] %s: %s",
             id, pass ? "PASS" : "FAIL", c->text);
    session_log(s, pass ? 3 : 2, msg);

    printf("claim[%d] T%d %s: %s\n", id, c->tier,
           pass ? "✓ PASS" : "✗ FAIL", c->text);

    if (!pass) {
        printf("ACTION: failed claim needs revision or demotion\n");
    }

    /* ═══ WIRE: auto-learn ═══
     * The loop closes here. Verify outcome propagates to
     * the shared graph. Files linked to this problem get
     * their weights adjusted. No manual step needed. */
    int affected = wire_auto_learn(s->problem, pass);
    if (affected > 0) {
        printf("wire: %s %d edges\n", pass ? "strengthened" : "weakened", affected);
    }

    /* Check if all claims verified */
    int all_done = 1;
    for (uint32_t i = 0; i < s->n_claims; i++) {
        if (s->claims[i].verified == VSTAT_PENDING) {
            all_done = 0;
            break;
        }
    }
    if (all_done && s->n_claims > 0) {
        s->phase = PHASE_DONE;
        session_log(s, 1, "ALL CLAIMS VERIFIED");
        printf("phase: DONE — all claims resolved\n");
    }
}

static void cmd_check(Session *s) {
    /* Pre-ship checklist */
    printf("---ONETWO-CHECK---\n");
    printf("problem: %s\n", s->problem);
    printf("phase: %s\n", PHASE_NAME[s->phase]);

    int issues = 0;

    /* ONE completeness */
    if (s->n_knowns == 0) {
        printf("⚠ NO KNOWNS: what's established?\n");
        issues++;
    }
    if (s->n_unknowns == 0) {
        printf("⚠ NO UNKNOWNS: what gaps remain?\n");
        issues++;
    }
    if (s->n_probes == 0) {
        printf("⚠ NO PROBES: decomposition has no findings\n");
        issues++;
    }
    if (s->phase < PHASE_BEDROCK) {
        printf("⚠ BEDROCK NOT REACHED: still in ONE\n");
        issues++;
    }

    /* TWO completeness */
    int pending = 0, passed = 0, failed = 0;
    int t1_unverified = 0;
    for (uint32_t i = 0; i < s->n_claims; i++) {
        switch (s->claims[i].verified) {
            case VSTAT_PENDING: pending++; break;
            case VSTAT_PASS:    passed++;  break;
            case VSTAT_FAIL:    failed++;  break;
        }
        if (s->claims[i].tier == TIER_T1 && s->claims[i].verified == VSTAT_PENDING) {
            printf("⚠ T1 UNVERIFIED: claim[%d] %s\n", i, s->claims[i].text);
            t1_unverified++;
            issues++;
        }
    }
    if (failed > 0) {
        printf("⚠ %d FAILED CLAIMS need revision\n", failed);
        issues++;
    }

    printf("\nclaims: %d total (%d pass, %d fail, %d pending)\n",
           s->n_claims, passed, failed, pending);
    printf("knowns: %d  unknowns: %d  probes: %d\n",
           s->n_knowns, s->n_unknowns, s->n_probes);

    if (issues == 0) {
        printf("\n✓ CLEAR TO SHIP\n");
    } else {
        printf("\n✗ %d ISSUES — address before shipping\n", issues);
    }
    printf("---END-CHECK---\n");
}

static void cmd_status(Session *s) {
    printf("---ONETWO-STATUS---\n");
    printf("problem: %s\n", s->problem[0] ? s->problem : "(none)");
    printf("phase: %s\n", PHASE_NAME[s->phase]);
    printf("created: %s\n", fmt_ts(s->created));
    printf("modified: %s (%s)\n", fmt_ts(s->modified), fmt_ago(s->modified));
    printf("\n");

    if (s->n_knowns > 0) {
        printf("KNOWNS (%d):\n", s->n_knowns);
        for (uint32_t i = 0; i < s->n_knowns; i++)
            printf("  [%d] %s\n", i, s->knowns[i].text);
    }

    if (s->n_unknowns > 0) {
        printf("UNKNOWNS (%d):\n", s->n_unknowns);
        for (uint32_t i = 0; i < s->n_unknowns; i++)
            printf("  [%d] %s\n", i, s->unknowns[i].text);
    }

    if (s->n_probes > 0) {
        printf("PROBES (%d):\n", s->n_probes);
        for (uint32_t i = 0; i < s->n_probes; i++)
            printf("  [%d] %s\n", i, s->probes[i].text);
    }

    if (s->n_claims > 0) {
        printf("CLAIMS (%d):\n", s->n_claims);
        for (uint32_t i = 0; i < s->n_claims; i++) {
            Claim *c = &s->claims[i];
            const char *mark = c->verified == VSTAT_PASS ? "✓" :
                               c->verified == VSTAT_FAIL ? "✗" : "?";
            printf("  [%d] T%d %s %s %s\n",
                   i, c->tier, VSTAT_NAME[c->verified], mark, c->text);
        }
    }

    printf("log: %d entries\n", s->n_log);
    printf("---END-STATUS---\n");
}

static void cmd_history(Session *s) {
    static const char *TYPE_TAG[] = { "", "[DECISION] ", "[CORRECTION] ", "[FINDING] " };

    printf("---ONETWO-HISTORY---\n");
    printf("problem: %s\n", s->problem[0] ? s->problem : "(none)");
    printf("entries: %d\n\n", s->n_log);

    for (uint32_t i = 0; i < s->n_log; i++) {
        LogEntry *e = &s->log[i];
        printf("  %s  %s%s\n", fmt_ts(e->timestamp),
               TYPE_TAG[e->type < 4 ? e->type : 0], e->text);
    }
    printf("---END-HISTORY---\n");
}

static void cmd_log_msg(Session *s, const char *msg) {
    session_log(s, 0, msg);
    printf("logged: %s\n", msg);
}

static void cmd_reset(Session *s) {
    memset(s, 0, sizeof(Session));
    s->magic = SESSION_MAGIC;
    s->version = SESSION_VER;
    s->phase = PHASE_EMPTY;
    printf("session cleared\n");
}

/* ═══════════════════════════════════════════════════════
   MAIN
   ═══════════════════════════════════════════════════════ */

static void usage(void) {
    printf("ONETWO — Reasoning Scaffold\n\n");
    printf("ONE phase (decompose):\n");
    printf("  onetwo init <problem>          Start new problem\n");
    printf("  onetwo known <fact>            Register known fact\n");
    printf("  onetwo unknown <gap>           Register unknown/gap\n");
    printf("  onetwo probe <finding>         Log decomposition finding\n");
    printf("  onetwo bedrock                 Declare bedrock reached\n");
    printf("\nTWO phase (build):\n");
    printf("  onetwo claim <statement> <T>   Register claim (T=1-4)\n");
    printf("  onetwo verify <id> <pass|fail> Verify claim\n");
    printf("\nSession:\n");
    printf("  onetwo check                   Pre-ship checklist\n");
    printf("  onetwo status                  Full session state\n");
    printf("  onetwo history                 Session log\n");
    printf("  onetwo log <message>           Append to log\n");
    printf("  onetwo reset                   Clear session\n");
    printf("\nSession path: $ONETWO_SESSION or %s\n", DEFAULT_SESSION);
    printf("\nIsaac & Claude — February 2026\n");
}

int main(int argc, char **argv) {
    if (argc < 2) { usage(); return 0; }

    const char *cmd = argv[1];
    const char *sp = get_session_path();

    Session *s = session_load(sp);

    if (strcmp(cmd, "init") == 0 && argc >= 3) {
        cmd_init(s, concat_args(argc, argv, 2));
    }
    else if (strcmp(cmd, "known") == 0 && argc >= 3) {
        cmd_known(s, concat_args(argc, argv, 2));
    }
    else if (strcmp(cmd, "unknown") == 0 && argc >= 3) {
        cmd_unknown(s, concat_args(argc, argv, 2));
    }
    else if (strcmp(cmd, "probe") == 0 && argc >= 3) {
        cmd_probe(s, concat_args(argc, argv, 2));
    }
    else if (strcmp(cmd, "bedrock") == 0) {
        cmd_bedrock(s);
    }
    else if (strcmp(cmd, "claim") == 0 && argc >= 4) {
        /* Last arg is tier number */
        int tier = atoi(argv[argc - 1]);
        /* If tier is 0, user forgot — check if it's 1-4 */
        if (tier < 1 || tier > 4) {
            /* Maybe they didn't pass a tier, default to T3 */
            cmd_claim(s, concat_args(argc, argv, 2), 3);
        } else {
            /* Build statement from all args except last */
            char stmt[MAX_TEXT];
            stmt[0] = '\0';
            int pos = 0;
            for (int i = 2; i < argc - 1 && pos < MAX_TEXT - 2; i++) {
                if (i > 2) stmt[pos++] = ' ';
                int len = strlen(argv[i]);
                if (pos + len >= MAX_TEXT - 1) len = MAX_TEXT - 1 - pos;
                memcpy(stmt + pos, argv[i], len);
                pos += len;
            }
            stmt[pos] = '\0';
            cmd_claim(s, stmt, tier);
        }
    }
    else if (strcmp(cmd, "verify") == 0 && argc >= 4) {
        int id = atoi(argv[2]);
        int pass = (strcmp(argv[3], "pass") == 0 || strcmp(argv[3], "1") == 0);
        cmd_verify(s, id, pass);
    }
    else if (strcmp(cmd, "check") == 0) {
        cmd_check(s);
    }
    else if (strcmp(cmd, "status") == 0) {
        cmd_status(s);
    }
    else if (strcmp(cmd, "history") == 0) {
        cmd_history(s);
    }
    else if (strcmp(cmd, "log") == 0 && argc >= 3) {
        cmd_log_msg(s, concat_args(argc, argv, 2));
    }
    else if (strcmp(cmd, "reset") == 0) {
        cmd_reset(s);
    }
    else {
        usage();
        free(s);
        return 1;
    }

    session_save(s, sp);
    free(s);
    return 0;
}
