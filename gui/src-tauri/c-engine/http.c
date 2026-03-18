/*
 * http.c — Minimal HTTP/JSON API server
 *
 * Single-threaded, POSIX sockets, no dependencies.
 * Parses HTTP requests, routes to handlers, returns JSON.
 * CORS enabled for browser access from any origin.
 */

#include "lrm_http.h"
#include "lrm_schema.h"
#include "lrm_import.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#ifdef _WIN32
  #define WIN32_LEAN_AND_MEAN
  #include <winsock2.h>
  #include <ws2tcpip.h>
  typedef int socklen_t;
  #define close_socket closesocket
  #define sock_read(fd,buf,len) recv(fd,buf,len,0)
  #define sock_write(fd,buf,len) send(fd,buf,len,0)
  static void sock_init(void) {
      WSADATA wsa; WSAStartup(MAKEWORD(2,2), &wsa);
  }
  static void sock_cleanup(void) { WSACleanup(); }
  #define SIGPIPE 0
  #define MSG_NOSIGNAL 0
#else
  #include <unistd.h>
  #include <sys/socket.h>
  #include <netinet/in.h>
  #include <arpa/inet.h>
  #include <signal.h>
  #define close_socket close
  #define sock_read(fd,buf,len) read(fd,buf,len)
  #define sock_write(fd,buf,len) write(fd,buf,len)
  static void sock_init(void) {}
  static void sock_cleanup(void) {}
#endif
#include <ctype.h>
#ifdef _WIN32
  /* GetTickCount64 from windows.h (via winsock2.h) */
#else
  #include <time.h>
#endif

extern void btree_encode_i64(int64_t val, uint8_t *buf);

/* ═══ Rate Limiting ════════════════════════════════════════ */

#define RATE_GLOBAL_RPS     200   /* max requests/second, all clients */
#define RATE_PER_IP_RPS      50   /* max requests/second, single IP  */
#define RATE_IP_SLOTS        64   /* hash table slots (power of 2)   */

typedef struct {
    uint32_t ip;            /* IPv4 address (network order)      */
    int      tokens;        /* remaining tokens this window       */
    uint64_t window_start;  /* ms when current window began       */
} IpSlot;

static struct {
    int      tokens;        /* global tokens remaining            */
    uint64_t window_start;  /* global window start (ms)           */
    IpSlot   slots[RATE_IP_SLOTS];
} rl;  /* zero-initialized */

static uint64_t rl_now_ms(void) {
#ifdef _WIN32
    return GetTickCount64();
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000 + (uint64_t)ts.tv_nsec / 1000000;
#endif
}

/* Returns 1=allowed, 0=rate-limited. O(1) per call. */
static int rate_check(uint32_t ip_net) {
    uint64_t now = rl_now_ms();

    /* Global window: reset every second */
    if (now - rl.window_start >= 1000 || rl.window_start == 0) {
        rl.tokens = RATE_GLOBAL_RPS;
        rl.window_start = now;
    }
    if (rl.tokens <= 0) return 0;

    /* Per-IP slot (Knuth multiplicative hash) */
    uint32_t idx = (ip_net * 2654435761u) >> (32 - 6); /* >>26 = mod 64 */
    IpSlot *s = &rl.slots[idx];

    if (s->ip != ip_net || now - s->window_start >= 1000 || s->window_start == 0) {
        s->ip = ip_net;
        s->tokens = RATE_PER_IP_RPS;
        s->window_start = now;
    }
    if (s->tokens <= 0) return 0;

    /* Consume */
    rl.tokens--;
    s->tokens--;
    return 1;
}

/* ═══ JSON Builder ══════════════════════════════════════ */

typedef struct {
    char *buf;
    int   len;
    int   cap;
} Json;

static void j_init(Json *j) { j->cap=8192; j->buf=malloc(j->cap); j->len=0; j->buf[0]=0; }
static void j_grow(Json *j, int need) {
    while (j->len+need >= j->cap) { j->cap*=2; j->buf=realloc(j->buf,j->cap); }
}
static void j_raw(Json *j, const char *s) {
    int n=strlen(s); j_grow(j,n+1); memcpy(j->buf+j->len,s,n); j->len+=n; j->buf[j->len]=0;
}
static void j_str(Json *j, const char *key, const char *val) {
    j_grow(j,strlen(key)+strlen(val)+10);
    /* escape special chars in val (matches send_error escaping) */
    j->len+=sprintf(j->buf+j->len,"\"%s\":\"",key);
    for (const char *p=val;*p;p++) {
        if (*p=='"') { j_raw(j,"\\\""); }
        else if (*p=='\\') { j_raw(j,"\\\\"); }
        else if (*p=='\n') { j_raw(j,"\\n"); }
        else if (*p=='\r') { j_raw(j,"\\r"); }
        else if (*p=='\t') { j_raw(j,"\\t"); }
        else if ((unsigned char)*p < 0x20) {
            /* control chars → \u00XX */
            char esc[8]; snprintf(esc,sizeof(esc),"\\u%04x",(unsigned char)*p);
            j_raw(j,esc);
        }
        else { j_grow(j,2); j->buf[j->len++]=*p; j->buf[j->len]=0; }
    }
    j_raw(j,"\"");
}
static void j_int(Json *j, const char *key, int64_t val) {
    j_grow(j,strlen(key)+24); j->len+=sprintf(j->buf+j->len,"\"%s\":%lld",key,(long long)val);
}
static void j_comma(Json *j) { j_raw(j,","); }

/* ═══ HTTP Response Helpers ═════════════════════════════ */

static void send_response(int fd, int status, const char *status_text,
                          const char *content_type, const char *body) {
    char header[1024];
    int body_len = body ? (int)strlen(body) : 0;
    int hlen = snprintf(header, sizeof(header),
        "HTTP/1.1 %d %s\r\n"
        "Content-Type: %s\r\n"
        "Content-Length: %d\r\n"
        "Access-Control-Allow-Origin: *\r\n"
        "Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n"
        "Access-Control-Allow-Headers: Content-Type, X-LRM-User-Id\r\n"
        "Connection: close\r\n"
        "\r\n", status, status_text, content_type, body_len);
    sock_write(fd, header, hlen);
    if (body && body_len > 0) sock_write(fd, body, body_len);
}

static void send_json(int fd, int status, const char *body) {
    send_response(fd, status, status >= 400 ? "Error" : "OK",
                  "application/json", body);
}

static void send_error(int fd, int status, const char *msg) {
    char buf[512]; char safe[256];
    /* Escape quotes, backslashes, and control chars for valid JSON */
    int j=0;
    for (int i=0; msg[i] && j<(int)sizeof(safe)-6; i++) {
        if (msg[i]=='"' || msg[i]=='\\') { safe[j++]='\\'; safe[j++]=msg[i]; }
        else if (msg[i]=='\n') { safe[j++]='\\'; safe[j++]='n'; }
        else if (msg[i]=='\r') { safe[j++]='\\'; safe[j++]='r'; }
        else if (msg[i]=='\t') { safe[j++]='\\'; safe[j++]='t'; }
        else if ((unsigned char)msg[i] < 0x20) { /* skip other control chars */ }
        else safe[j++]=msg[i];
    }
    safe[j]=0;
    snprintf(buf, sizeof(buf), "{\"error\":\"%s\"}", safe);
    send_json(fd, status, buf);
}

static void send_ok(int fd, const char *msg) {
    char buf[512]; char safe[256];
    /* Escape same as send_error — prevent JSON injection on success path */
    int j=0;
    for (int i=0; msg[i] && j<(int)sizeof(safe)-6; i++) {
        if (msg[i]=='"' || msg[i]=='\\') { safe[j++]='\\'; safe[j++]=msg[i]; }
        else if (msg[i]=='\n') { safe[j++]='\\'; safe[j++]='n'; }
        else if (msg[i]=='\r') { safe[j++]='\\'; safe[j++]='r'; }
        else if (msg[i]=='\t') { safe[j++]='\\'; safe[j++]='t'; }
        else if ((unsigned char)msg[i] < 0x20) { /* skip control chars */ }
        else safe[j++]=msg[i];
    }
    safe[j]=0;
    snprintf(buf, sizeof(buf), "{\"ok\":true,\"message\":\"%s\"}", safe);
    send_json(fd, 200, buf);
}

/* ═══ JSON Serializers for Domain Types ═════════════════ */

static void json_system(Json *j, const System *s) {
    j_raw(j,"{"); j_int(j,"system_id",s->system_id); j_comma(j);
    j_str(j,"name",s->name); j_comma(j);
    j_str(j,"system_type",system_type_str(s->system_type)); j_comma(j);
    j_int(j,"system_type_id",s->system_type); j_comma(j);
    j_str(j,"status",system_status_str(s->status)); j_comma(j);
    j_int(j,"status_id",s->status); j_comma(j);
    j_str(j,"cooling",cooling_str(s->cooling)); j_comma(j);
    j_str(j,"ip_base",s->ip_base); j_comma(j);
    j_int(j,"chamber_count",s->chamber_count); j_comma(j);
    j_int(j,"shelves_per_chamber",s->shelves_per_chamber); j_comma(j);
    j_int(j,"slots_per_shelf",s->slots_per_shelf); j_comma(j);
    j_str(j,"notes",s->notes); j_raw(j,"}");
}

static void json_location(Json *j, const Location *l) {
    j_raw(j,"{"); j_int(j,"location_id",l->location_id); j_comma(j);
    j_int(j,"system_id",l->system_id); j_comma(j);
    j_int(j,"parent_id",l->parent_id); j_comma(j);
    j_str(j,"name",l->name); j_comma(j);
    j_str(j,"loc_type",loc_type_str(l->loc_type)); j_comma(j);
    j_int(j,"position",l->position); j_comma(j);
    j_str(j,"status",loc_status_str(l->status)); j_comma(j);
    j_str(j,"path_cache",l->path_cache); j_raw(j,"}");
}

static void json_device(Json *j, const Device *d) {
    j_raw(j,"{"); j_int(j,"device_id",d->device_id); j_comma(j);
    j_str(j,"customer",d->customer); j_comma(j);
    j_str(j,"device_name",d->device_name); j_comma(j);
    j_str(j,"device_number",d->device_number); j_comma(j);
    j_str(j,"device_family",d->device_family); j_comma(j);
    j_str(j,"package_type",d->package_type); j_raw(j,"}");
}

static void json_project(Json *j, const Project *p) {
    j_raw(j,"{"); j_int(j,"project_id",p->project_id); j_comma(j);
    j_int(j,"device_id",p->device_id); j_comma(j);
    j_str(j,"project_number",p->project_number); j_comma(j);
    j_int(j,"system_id",p->system_id); j_comma(j);
    j_str(j,"status",project_status_str(p->status)); j_comma(j);
    j_str(j,"cooling",cooling_str(p->cooling)); j_comma(j);
    j_int(j,"start_date_ms",p->start_date_ms); j_comma(j);
    j_int(j,"end_date_ms",p->end_date_ms); j_raw(j,"}");
}

static void json_lot(Json *j, const Lot *l) {
    j_raw(j,"{"); j_int(j,"lot_id",l->lot_id); j_comma(j);
    j_int(j,"project_id",l->project_id); j_comma(j);
    j_int(j,"system_id",l->system_id); j_comma(j);
    j_str(j,"lot_number",l->lot_number); j_comma(j);
    j_str(j,"customer_lot",l->customer_lot); j_comma(j);
    j_str(j,"step",lot_step_str(l->step)); j_comma(j);
    j_int(j,"step_id",l->step); j_comma(j);
    j_str(j,"lot_status",lot_status_str(l->lot_status)); j_comma(j);
    j_int(j,"expected_qty",l->expected_qty); j_comma(j);
    j_int(j,"running_qty",l->running_qty); j_comma(j);
    j_int(j,"good",l->good); j_comma(j);
    j_int(j,"reject",l->reject); j_comma(j);
    j_int(j,"missing",l->missing); j_comma(j);
    j_int(j,"received_ms",l->received_ms); j_comma(j);
    j_int(j,"started_ms",l->started_ms); j_comma(j);
    j_int(j,"completed_ms",l->completed_ms); j_raw(j,"}");
}

static void json_serialized(Json *j, const SerializedHw *s) {
    j_raw(j,"{"); j_int(j,"item_id",s->item_id); j_comma(j);
    j_int(j,"type_id",s->type_id); j_comma(j);
    j_str(j,"serial_no",s->serial_no); j_comma(j);
    j_int(j,"system_id",s->system_id); j_comma(j);
    j_int(j,"location_id",s->location_id); j_comma(j);
    j_int(j,"project_id",s->project_id); j_comma(j);
    j_str(j,"status",item_status_str(s->status)); j_comma(j);
    j_int(j,"status_id",s->status); j_comma(j);
    j_int(j,"date_created_ms",s->date_created_ms); j_comma(j);
    j_int(j,"last_moved_ms",s->last_moved_ms); j_comma(j);
    j_str(j,"notes",s->notes); j_comma(j);
    /* socket_mask as hex string, socket_count */
    char hex[33]; hex[0]=0;
    for (int i=0;i<16;i++) sprintf(hex+i*2,"%02x",s->socket_mask[i]);
    j_str(j,"socket_mask",hex); j_comma(j);
    j_int(j,"socket_count",s->socket_count); j_raw(j,"}");
}

static void json_quantity(Json *j, const QuantityHw *q) {
    j_raw(j,"{"); j_int(j,"qty_id",q->qty_id); j_comma(j);
    j_int(j,"type_id",q->type_id); j_comma(j);
    j_int(j,"system_id",q->system_id); j_comma(j);
    j_int(j,"total",q->total); j_comma(j);
    j_int(j,"good",q->good); j_comma(j);
    j_int(j,"bad",q->bad); j_raw(j,"}");
}

static void json_configured(Json *j, const ConfiguredHw *c) {
    j_raw(j,"{"); j_int(j,"config_id",c->config_id); j_comma(j);
    j_int(j,"type_id",c->type_id); j_comma(j);
    j_int(j,"system_id",c->system_id); j_comma(j);
    j_int(j,"project_id",c->project_id); j_comma(j);
    j_int(j,"r0_ohms",c->r0_ohms); j_comma(j);
    j_int(j,"r4_ohms",c->r4_ohms); j_comma(j);
    j_int(j,"vout_mv",c->vout_mv); j_comma(j);
    j_str(j,"role",c->role>=0?core_role_str(c->role):"N/A"); j_comma(j);
    j_int(j,"quantity",c->quantity); j_raw(j,"}");
}

static void json_hw_type(Json *j, const HardwareType *h) {
    j_raw(j,"{"); j_int(j,"type_id",h->type_id); j_comma(j);
    j_str(j,"name",h->name); j_comma(j);
    j_str(j,"category",hw_category_str(h->category)); j_comma(j);
    j_str(j,"tracking",tracking_mode_str(h->tracking)); j_comma(j);
    j_int(j,"for_system_type",h->for_system_type); j_raw(j,"}");
}

static void json_audit(Json *j, const AuditEntry *a) {
    j_raw(j,"{"); j_int(j,"log_id",a->log_id); j_comma(j);
    j_int(j,"user_id",a->user_id); j_comma(j);
    j_str(j,"action",audit_action_str(a->action)); j_comma(j);
    j_str(j,"entity_table",a->entity_table); j_comma(j);
    j_int(j,"entity_id",a->entity_id); j_comma(j);
    j_int(j,"timestamp_ms",a->timestamp_ms); j_comma(j);
    j_str(j,"detail",a->detail); j_raw(j,"}");
}

static void json_user(Json *j, const User *u) {
    j_raw(j,"{"); j_int(j,"user_id",u->user_id); j_comma(j);
    j_str(j,"username",u->username); j_comma(j);
    j_str(j,"display_name",u->display_name); j_comma(j);
    j_str(j,"role",user_role_str(u->role)); j_comma(j);
    j_int(j,"active",u->active); j_raw(j,"}");
}

/* ═══ Tracker JSON Serializers ══════════════════════════ */

static void json_team_member(Json *j, const TeamMember *t) {
    j_raw(j,"{"); j_int(j,"team_member_id",t->team_member_id); j_comma(j);
    j_str(j,"name",t->name); j_comma(j);
    j_str(j,"role",t->role); j_comma(j);
    j_str(j,"primary_systems",t->primary_systems); j_comma(j);
    j_str(j,"board_patterns",t->board_patterns); j_raw(j,"}");
}

static void json_daily_activity(Json *j, const DailyActivity *a) {
    j_raw(j,"{"); j_int(j,"activity_id",a->activity_id); j_comma(j);
    j_str(j,"date",a->date); j_comma(j);
    j_int(j,"created_at_ms",a->created_at_ms); j_raw(j,"}");
}

static void json_upload_task(Json *j, const UploadTask *t) {
    j_raw(j,"{"); j_int(j,"upload_id",t->upload_id); j_comma(j);
    j_int(j,"activity_id",t->activity_id); j_comma(j);
    j_str(j,"load_date",t->load_date); j_comma(j);
    j_str(j,"customer",t->customer); j_comma(j);
    j_str(j,"lot",t->lot); j_comma(j);
    j_str(j,"ise_id",t->ise_id); j_comma(j);
    j_int(j,"qty",t->qty); j_comma(j);
    j_str(j,"device",t->device); j_comma(j);
    j_str(j,"time_at_lab",t->time_at_lab); j_comma(j);
    j_str(j,"notes",t->notes); j_comma(j);
    j_str(j,"status",upload_status_str(t->status)); j_comma(j);
    j_int(j,"status_id",t->status); j_comma(j);
    j_int(j,"assigned_to",t->assigned_to); j_comma(j);
    j_int(j,"completed_at_ms",t->completed_at_ms); j_comma(j);
    j_int(j,"created_at_ms",t->created_at_ms); j_raw(j,"}");
}

static void json_download_task(Json *j, const DownloadTask *t) {
    j_raw(j,"{"); j_int(j,"download_id",t->download_id); j_comma(j);
    j_int(j,"activity_id",t->activity_id); j_comma(j);
    j_str(j,"customer",t->customer); j_comma(j);
    j_str(j,"lot",t->lot); j_comma(j);
    j_str(j,"ise_id",t->ise_id); j_comma(j);
    j_int(j,"qty",t->qty); j_comma(j);
    j_str(j,"device",t->device); j_comma(j);
    j_str(j,"download_time",t->download_time); j_comma(j);
    j_str(j,"notes",t->notes); j_comma(j);
    j_str(j,"status",task_status_str(t->status)); j_comma(j);
    j_int(j,"status_id",t->status); j_comma(j);
    j_int(j,"assigned_to",t->assigned_to); j_comma(j);
    j_int(j,"completed_at_ms",t->completed_at_ms); j_comma(j);
    j_int(j,"created_at_ms",t->created_at_ms); j_raw(j,"}");
}

static void json_eng_activity_task(Json *j, const EngActivityTask *t) {
    j_raw(j,"{"); j_int(j,"eng_task_id",t->eng_task_id); j_comma(j);
    j_int(j,"activity_id",t->activity_id); j_comma(j);
    j_str(j,"customer",t->customer); j_comma(j);
    j_str(j,"device",t->device); j_comma(j);
    j_str(j,"description",t->description); j_comma(j);
    j_str(j,"ise_numbers",t->ise_numbers); j_comma(j);
    j_str(j,"status",task_status_str(t->status)); j_comma(j);
    j_int(j,"status_id",t->status); j_comma(j);
    j_int(j,"assigned_to",t->assigned_to); j_comma(j);
    j_int(j,"completed_at_ms",t->completed_at_ms); j_comma(j);
    j_int(j,"created_at_ms",t->created_at_ms); j_raw(j,"}");
}

static void json_eng_hours(Json *j, const EngHoursEntry *e) {
    j_raw(j,"{"); j_int(j,"entry_id",e->entry_id); j_comma(j);
    j_str(j,"date",e->date); j_comma(j);
    j_str(j,"customer",e->customer); j_comma(j);
    j_str(j,"project",e->project); j_comma(j);
    j_str(j,"pcb_number",e->pcb_number); j_comma(j);
    j_str(j,"description",e->description); j_comma(j);
    j_str(j,"engineer",e->engineer); j_comma(j);
    /* hours as decimal */
    j_grow(j, 64);
    j->len += sprintf(j->buf+j->len,"\"hours_spent\":%.2f",
                      e->hours_hundredths / 100.0);
    j_comma(j);
    j_int(j,"billable",e->billable); j_comma(j);
    j_grow(j, 64);
    j->len += sprintf(j->buf+j->len,"\"quoted_hours\":%.2f",
                      e->quoted_hours_hundredths / 100.0);
    j_comma(j);
    j_str(j,"po_number",e->po_number); j_comma(j);
    j_int(j,"source_task_id",e->source_task_id); j_comma(j);
    j_int(j,"created_at_ms",e->created_at_ms); j_raw(j,"}");
}

/* ═══ V1 Compat + User/Location JSON Serializers ══════════ */

static void json_v1_board(Json *j, const V1Board *b) {
    j_raw(j,"{"); j_int(j,"board_id",b->board_id); j_comma(j);
    j_str(j,"customer",b->customer); j_comma(j);
    j_str(j,"platform",b->platform); j_comma(j);
    j_str(j,"pcb_number_text",b->pcb_number_text); j_comma(j);
    j_str(j,"revision",b->revision); j_comma(j);
    j_str(j,"serial_no",b->serial_no); j_comma(j);
    j_int(j,"power_qty",b->power_qty); j_comma(j);
    j_str(j,"status",b->status); j_comma(j);
    j_int(j,"location_id",b->location_id); j_comma(j);
    j_int(j,"socket_rows",b->socket_rows); j_comma(j);
    j_int(j,"socket_cols",b->socket_cols); j_comma(j);
    j_str(j,"notes",b->notes); j_comma(j);
    j_str(j,"individual_notes",b->individual_notes); j_comma(j);
    j_str(j,"date_created",b->date_created); j_comma(j);
    j_str(j,"last_used_date",b->last_used_date); j_comma(j);
    j_int(j,"sockets_working",b->sockets_working); j_comma(j);
    j_int(j,"sockets_bad",b->sockets_bad); j_comma(j);
    j_int(j,"sockets_not_installed",b->sockets_not_installed);
    j_raw(j,"}");
}

static void json_v1_board_type(Json *j, const V1BoardType *bt) {
    j_raw(j,"{"); j_int(j,"board_type_id",bt->board_type_id); j_comma(j);
    j_str(j,"customer",bt->customer); j_comma(j);
    j_str(j,"pcb_number_text",bt->pcb_number_text); j_comma(j);
    j_str(j,"revision",bt->revision); j_comma(j);
    j_str(j,"platform",bt->platform); j_comma(j);
    j_int(j,"power_qty",bt->power_qty); j_comma(j);
    j_int(j,"socket_rows",bt->socket_rows); j_comma(j);
    j_int(j,"socket_cols",bt->socket_cols); j_comma(j);
    j_str(j,"notes",bt->notes); j_comma(j);
    j_int(j,"is_default",bt->is_default); j_comma(j);
    j_str(j,"devices",bt->devices); j_raw(j,"}");
}

static void json_v1_board_log(Json *j, const V1BoardLog *l) {
    j_raw(j,"{"); j_int(j,"log_id",l->log_id); j_comma(j);
    j_int(j,"board_id",l->board_id); j_comma(j);
    j_str(j,"timestamp",l->timestamp); j_comma(j);
    j_str(j,"user",l->user); j_comma(j);
    j_str(j,"action",l->action); j_comma(j);
    j_str(j,"details",l->details); j_comma(j);
    j_int(j,"from_location_id",l->from_location_id); j_comma(j);
    j_int(j,"to_location_id",l->to_location_id); j_raw(j,"}");
}

static void json_v1_socket_note(Json *j, const V1SocketNote *s) {
    j_raw(j,"{"); j_int(j,"note_id",s->note_id); j_comma(j);
    j_int(j,"board_id",s->board_id); j_comma(j);
    j_int(j,"socket_number",s->socket_number); j_comma(j);
    j_str(j,"status",s->status); j_comma(j);
    j_str(j,"note",s->note); j_raw(j,"}");
}

static void json_user_auth(Json *j, const User *u) {
    j_raw(j,"{"); j_int(j,"user_id",u->user_id); j_comma(j);
    j_str(j,"username",u->username); j_comma(j);
    j_str(j,"display_name",u->display_name); j_comma(j);
    j_str(j,"password_hash",u->password_hash); j_comma(j);
    j_str(j,"role",user_role_str(u->role)); j_comma(j);
    j_int(j,"role_id",u->role); j_comma(j);
    j_int(j,"active",u->active); j_comma(j);
    j_int(j,"created_ms",u->created_ms); j_comma(j);
    j_int(j,"last_login_ms",u->last_login_ms); j_raw(j,"}");
}

/* ═══ Simple JSON Parser (for POST bodies) ══════════════ */
/* Extracts string/int values from flat JSON objects.
 * Properly skips string values so keys inside values don't match. */

/* Find "key" as a JSON key (not inside a string value).
 * Returns pointer to first char after closing quote of key, or NULL. */
static const char *json_find_key(const char *body, const char *key) {
    char search[128]; snprintf(search,128,"\"%s\"",key);
    int slen = (int)strlen(search);
    const char *p = body;
    int in_str = 0;
    while (*p) {
        if (*p == '\\' && in_str) { p += 2; continue; }
        if (*p == '"') {
            if (!in_str) {
                /* Start of a string — check if it matches our key */
                if (strncmp(p, search, slen) == 0) {
                    /* Check that a ':' follows (it's a key, not a value) */
                    const char *after = p + slen;
                    while (*after == ' ' || *after == '\t') after++;
                    if (*after == ':') return after;
                }
                in_str = 1;
            } else {
                in_str = 0;
            }
            p++;
        } else {
            p++;
        }
    }
    return NULL;
}

static const char *json_get_str(const char *body, const char *key, char *out, int max) {
    const char *p = json_find_key(body, key);
    if (!p) { out[0]=0; return out; }
    /* p points at ':' */
    p++;
    while (*p && (*p==' ' || *p=='\t')) p++;
    if (*p=='"') {
        p++;
        int i=0;
        while (*p && *p!='"' && i<max-1) {
            if (*p=='\\' && *(p+1)) {
                p++; /* skip backslash, store next char */
                if (*p == 'n') { out[i++] = '\n'; p++; continue; }
                if (*p == 't') { out[i++] = '\t'; p++; continue; }
                if (*p == 'r') { out[i++] = '\r'; p++; continue; }
            }
            out[i++] = *p++;
        }
        out[i]=0;
    } else { out[0]=0; }
    return out;
}

static int64_t json_get_int(const char *body, const char *key) {
    const char *p = json_find_key(body, key);
    if (!p) return 0;
    p++;
    while (*p && (*p==' ' || *p=='\t')) p++;
    return atoll(p);
}

static double json_get_float(const char *body, const char *key) {
    const char *p = json_find_key(body, key);
    if (!p) return 0.0;
    p++;
    while (*p && (*p==' ' || *p=='\t')) p++;
    return atof(p);
}

/* ═══ JSON Array Iterator (for batch endpoints) ════════ */

/* Extract next {...} object from a JSON array.
 * cursor starts after '[', advances past each object.
 * Returns 1 if object found, 0 if end of array. */
static int json_next_object(const char **cursor, char *obj_buf, int obj_max) {
    const char *p = *cursor;
    while (*p && (*p==' '||*p==','||*p=='\n'||*p=='\r'||*p=='\t')) p++;
    if (*p==']' || *p=='\0') return 0;
    if (*p!='{') return 0;
    int depth=0, i=0, in_str=0;
    while (*p) {
        char c = *p;
        /* Track string state — count consecutive backslashes for proper escaping */
        if (c=='"') {
            int backslashes = 0;
            const char *bp = p - 1;
            while (bp >= *cursor && *bp == '\\') { backslashes++; bp--; }
            if (backslashes % 2 == 0) in_str = !in_str;
        }
        if (!in_str) {
            if (c=='{') depth++;
            if (c=='}') {
                depth--;
                if (i < obj_max-1) obj_buf[i++] = *p;
                p++;
                if (depth==0) { obj_buf[i]=0; *cursor=p; return 1; }
                continue;
            }
        }
        if (i < obj_max-1) obj_buf[i++] = *p;
        p++;
    }
    /* Truncated or malformed — skip to end, return failure */
    obj_buf[0]=0;
    *cursor = p;
    return 0;
}

/* Find "items":[ in body and return pointer past '[', or NULL */
static const char *json_find_array(const char *body, const char *key) {
    char search[64]; snprintf(search,64,"\"%s\"",key);
    const char *p = strstr(body, search);
    if (!p) return NULL;
    p = strchr(p, '[');
    if (!p) return NULL;
    return p+1; /* past '[' */
}

/* ═══ Request Parser ════════════════════════════════════ */

typedef struct {
    char method[8];
    char path[HTTP_MAX_PATH];
    char body[HTTP_MAX_BODY];
    int  body_len;
    int64_t actor_user_id;  /* per-request actor from X-LRM-User-Id header */
} HttpReq;

static int parse_content_length(const char *headers, int header_len) {
    const char *p = headers;
    const char *end = headers + header_len;
    while (p < end) {
        /* case-insensitive search for Content-Length: */
        if ((p[0]=='C'||p[0]=='c') && (end-p) > 16) {
            if (strncasecmp(p, "Content-Length:", 15) == 0) {
                p += 15;
                while (*p == ' ') p++;
                return atoi(p);
            }
        }
        /* advance to next line */
        while (p < end && *p != '\n') p++;
        p++;
    }
    return 0;
}

static int64_t parse_actor_header(const char *headers, int header_len) {
    const char *p = headers;
    const char *end = headers + header_len;
    while (p < end) {
        if ((p[0]=='X'||p[0]=='x') && (end-p) > 18) {
            if (strncasecmp(p, "X-LRM-User-Id:", 14) == 0) {
                p += 14;
                while (*p == ' ') p++;
                return atoll(p);
            }
        }
        while (p < end && *p != '\n') p++;
        p++;
    }
    return 0;
}

static int parse_request(int fd, HttpReq *req) {
    char buf[HTTP_MAX_HEADER + HTTP_MAX_BODY];
    memset(req, 0, sizeof(HttpReq));

    /* First read — get at least the headers */
    int total = 0;
    int n = sock_read(fd, buf, sizeof(buf)-1);
    if (n <= 0) return -1;
    total = n;
    buf[total] = 0;

    /* parse method and path */
    sscanf(buf, "%7s %511s", req->method, req->path);

    /* find body (after \r\n\r\n) */
    char *body_start = strstr(buf, "\r\n\r\n");
    if (!body_start) {
        /* GET/DELETE/OPTIONS with no body — ok if not POST/PUT */
        if (strcmp(req->method,"POST")==0 || strcmp(req->method,"PUT")==0)
            return -1; /* POST/PUT without proper headers — reject */
        return 0;
    }

    body_start += 4;
    int header_len = (int)(body_start - buf);
    int body_received = total - header_len;

    /* Parse X-LRM-User-Id from headers */
    req->actor_user_id = parse_actor_header(buf, header_len);

    /* Parse Content-Length from headers */
    int content_length = parse_content_length(buf, header_len);
    if (content_length <= 0) {
        /* No Content-Length or zero — use what we got */
        if (body_received > 0 && body_received < HTTP_MAX_BODY) {
            memcpy(req->body, body_start, body_received);
            req->body_len = body_received;
            req->body[body_received] = 0;
        }
        return 0;
    }

    /* Clamp to our max */
    if (content_length >= HTTP_MAX_BODY) {
        content_length = HTTP_MAX_BODY - 1;
    }

    /* Read remaining body bytes if needed */
    while (body_received < content_length) {
        int remaining = content_length - body_received;
        int space = (int)(sizeof(buf) - 1 - total);
        if (space <= 0) break; /* buffer full */
        if (remaining > space) remaining = space;
        n = sock_read(fd, buf + total, remaining);
        if (n <= 0) break; /* connection closed or error */
        total += n;
        body_received += n;
        buf[total] = 0;
    }

    /* Reject truncated body — handler must not run on partial JSON */
    if (body_received < content_length) {
        return -1;
    }

    if (body_received > 0) {
        int copy_len = body_received < HTTP_MAX_BODY ? body_received : HTTP_MAX_BODY - 1;
        memcpy(req->body, body_start, copy_len);
        req->body_len = copy_len;
        req->body[copy_len] = 0;
    }

    return 0;
}

/* ═══ Path Matching ═════════════════════════════════════ */

/* Match "/api/systems/123" → extracts "123" as id */
static int match_path(const char *path, const char *pattern, int64_t *id) {
    /* pattern like "/api/systems/:id" */
    const char *pp = pattern, *pa = path;
    while (*pp && *pa) {
        if (*pp == ':') {
            /* extract numeric ID */
            if (id) *id = atoll(pa);
            /* skip to next / in both */
            while (*pa && *pa != '/') pa++;
            while (*pp && *pp != '/') pp++;
        } else {
            if (*pp != *pa) return 0;
            pp++; pa++;
        }
    }
    /* both should be exhausted or both at end */
    return (*pp == 0 && *pa == 0) ||
           (*pp == 0 && *pa == '/') ||
           (*pp == '/' && *pa == 0);
}

/* Match with string parameter */
static int match_path_str(const char *path, const char *prefix, char *param, int max) {
    int plen = strlen(prefix);
    if (strncmp(path, prefix, plen) != 0) return 0;
    const char *rest = path + plen;
    if (*rest == '/') rest++;
    strncpy(param, rest, max-1);
    param[max-1] = 0;
    /* URL decode basic: %20 → space */
    /* (skip for now — serials shouldn't have special chars) */
    return param[0] != 0;
}

/* ═══ Route Handlers ════════════════════════════════════ */

/* Default user_id for API calls (simplified — real auth would use tokens) */
static int64_t api_user_id = 1;

static void handle_request(Database *db, int fd, HttpReq *req) {
    const char *m = req->method;
    const char *p = req->path;
    int64_t id = 0;
    char sparam[256];

    /* Per-request actor: prefer header, fall back to global (last login) */
    int64_t actor = (req->actor_user_id > 0) ? req->actor_user_id : api_user_id;

    /* ── CORS preflight ────────────────────────────── */
    if (strcmp(m, "OPTIONS") == 0) {
        send_response(fd, 204, "No Content", "text/plain", "");
        return;
    }

    /* ── Health (readiness: touches DB) ──────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/health")==0) {
        /* Readiness: verify DB is open by reading page 0 */
        Page *hdr = pool_get(&db->pool, 0);
        if (!hdr || !db->open) {
            send_error(fd, 503, "Database not ready");
            return;
        }
        User users[1]; uint32_t ucnt = 0;
        lrm_list_users(db, users, &ucnt, 1);
        char hbuf[128];
        snprintf(hbuf, sizeof(hbuf),
            "{\"ok\":true,\"version\":\"2\",\"db_open\":true,\"tables\":%u}",
            db->num_tables);
        send_json(fd, 200, hbuf);
        return;
    }

    /* ── Systems ───────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/systems")==0) {
        System sys[64]; uint32_t cnt=0;
        lrm_list_systems(db, sys, &cnt, 64);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_system(&j, &sys[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && match_path(p,"/api/systems/:id",NULL) &&
        sscanf(p,"/api/systems/%lld",(long long*)&id)==1) {
        System sys;
        if (lrm_get_system(db,id,&sys)==LRM_OK) {
            Json j; j_init(&j); json_system(&j,&sys);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"System not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/systems")==0) {
        System sys={0};
        json_get_str(req->body,"name",sys.name,MAX_TEXT_LEN);
        sys.system_type = (int32_t)json_get_int(req->body,"system_type");
        sys.cooling = (int32_t)json_get_int(req->body,"cooling");
        json_get_str(req->body,"ip_base",sys.ip_base,64);
        sys.chamber_count = (int32_t)json_get_int(req->body,"chamber_count");
        sys.shelves_per_chamber = (int32_t)json_get_int(req->body,"shelves_per_chamber");
        sys.slots_per_shelf = (int32_t)json_get_int(req->body,"slots_per_shelf");
        int rc = lrm_create_system(db,&sys,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_system(&j,&sys);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_EXISTS?"Duplicate name":"Invalid data");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/systems/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_system_status(db,id,st,actor);
        if (rc==LRM_OK) send_ok(fd,"Status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/systems/%lld/generate-tree",(long long*)&id)==1) {
        int rc = lrm_generate_system_tree(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Tree generated");
        else send_error(fd,400,"Failed");
        return;
    }

    /* ── Locations ─────────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/locations/%lld/children",(long long*)&id)==1) {
        Location locs[256]; uint32_t cnt=0;
        lrm_list_children(db,id,locs,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_location(&j,&locs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/locations/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_location_status(db,id,st,actor);
        if (rc==LRM_OK) send_ok(fd,"Location status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/locations/%lld/rename",(long long*)&id)==1) {
        char name[MAX_TEXT_LEN];
        json_get_str(req->body,"name",name,MAX_TEXT_LEN);
        int rc = lrm_rename_location(db,id,name,actor);
        if (rc==LRM_OK) send_ok(fd,"Renamed");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/locations/%lld/move",(long long*)&id)==1) {
        int64_t parent = json_get_int(req->body,"parent_id");
        int rc = lrm_move_location(db,id,parent,actor);
        if (rc==LRM_OK) send_ok(fd,"Moved");
        else send_error(fd,400,"Failed");
        return;
    }

    /* ── Devices ───────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/devices")==0) {
        Device devs[256]; uint32_t cnt=0;
        lrm_list_devices(db,devs,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_device(&j,&devs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/devices")==0) {
        Device dev={0};
        json_get_str(req->body,"customer",dev.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"device_name",dev.device_name,MAX_TEXT_LEN);
        json_get_str(req->body,"device_number",dev.device_number,MAX_TEXT_LEN);
        json_get_str(req->body,"device_family",dev.device_family,MAX_TEXT_LEN);
        json_get_str(req->body,"package_type",dev.package_type,64);
        int rc = lrm_create_device(db,&dev,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_device(&j,&dev);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_EXISTS?"Duplicate device":"Invalid");
        return;
    }

    /* ── Projects ──────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/projects")==0) {
        Project projs[256]; uint32_t cnt=0;
        lrm_list_projects(db,projs,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_project(&j,&projs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && match_path_str(p,"/api/projects/number",sparam,64)) {
        Project proj;
        if (lrm_find_project_by_number(db,sparam,&proj)==LRM_OK) {
            Json j; j_init(&j); json_project(&j,&proj);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Project not found");
        return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/projects/%lld",(long long*)&id)==1) {
        Project proj;
        if (lrm_get_project(db,id,&proj)==LRM_OK) {
            Json j; j_init(&j); json_project(&j,&proj);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/projects")==0) {
        Project proj={0};
        proj.device_id = json_get_int(req->body,"device_id");
        json_get_str(req->body,"project_number",proj.project_number,64);
        proj.cooling = (int32_t)json_get_int(req->body,"cooling");
        int rc = lrm_create_project(db,&proj,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_project(&j,&proj);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_FK?"Bad device_id":rc==LRM_ERR_EXISTS?"Duplicate":"Invalid");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/projects/%lld/assign",(long long*)&id)==1) {
        int64_t sid = json_get_int(req->body,"system_id");
        int rc = lrm_assign_project_to_system(db,id,sid,actor);
        if (rc==LRM_OK) send_ok(fd,"Project assigned to system");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/projects/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_project_status(db,id,st,actor);
        if (rc==LRM_OK) send_ok(fd,"Project status updated");
        else send_error(fd,400,"Failed");
        return;
    }

    /* ── Lots ──────────────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/lots/project/%lld",(long long*)&id)==1) {
        Lot lots[256]; uint32_t cnt=0;
        lrm_list_lots_for_project(db,id,lots,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_lot(&j,&lots[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/lots/%lld",(long long*)&id)==1) {
        Lot lot;
        if (lrm_get_lot(db,id,&lot)==LRM_OK) {
            Json j; j_init(&j); json_lot(&j,&lot);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Lot not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/lots")==0) {
        Lot lot={0};
        lot.project_id = json_get_int(req->body,"project_id");
        json_get_str(req->body,"lot_number",lot.lot_number,MAX_TEXT_LEN);
        json_get_str(req->body,"customer_lot",lot.customer_lot,MAX_TEXT_LEN);
        lot.expected_qty = (int32_t)json_get_int(req->body,"expected_qty");
        int rc = lrm_create_lot(db,&lot,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_lot(&j,&lot);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/lots/%lld/advance",(long long*)&id)==1) {
        int32_t step = (int32_t)json_get_int(req->body,"step");
        int rc = lrm_advance_lot(db,id,step,actor);
        if (rc==LRM_OK) send_ok(fd,"Lot advanced");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/lots/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_lot_status(db,id,(LotStatus)st,actor);
        if (rc==LRM_OK) send_ok(fd,"Lot status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/lots/%lld/qty",(long long*)&id)==1) {
        int32_t good = (int32_t)json_get_int(req->body,"good");
        int32_t reject = (int32_t)json_get_int(req->body,"reject");
        int32_t missing = (int32_t)json_get_int(req->body,"missing");
        int rc = lrm_update_lot_qty(db,id,good,reject,missing,actor);
        if (rc==LRM_OK) send_ok(fd,"Quantities updated");
        else send_error(fd,400,"Failed");
        return;
    }

    /* ── Serialized Hardware ───────────────────────── */
    if (strcmp(m,"GET")==0 && match_path_str(p,"/api/hardware/serial",sparam,256)) {
        SerializedHw item;
        if (lrm_find_by_serial(db,sparam,&item)==LRM_OK) {
            Json j; j_init(&j); json_serialized(&j,&item);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Not found");
        return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/hardware/system/%lld",(long long*)&id)==1) {
        SerializedHw items[512]; uint32_t cnt=0;
        lrm_list_serialized_at(db,id,items,&cnt,512);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_serialized(&j,&items[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/hardware/project/%lld",(long long*)&id)==1) {
        SerializedHw items[512]; uint32_t cnt=0;
        lrm_list_serialized_for_project(db,id,items,&cnt,512);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_serialized(&j,&items[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/hardware")==0) {
        SerializedHw item={0};
        item.type_id = json_get_int(req->body,"type_id");
        json_get_str(req->body,"serial_no",item.serial_no,MAX_TEXT_LEN);
        item.system_id = json_get_int(req->body,"system_id");
        item.location_id = json_get_int(req->body,"location_id");
        int rc = lrm_create_serialized(db,&item,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_serialized(&j,&item);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_EXISTS?"Dup serial":rc==LRM_ERR_CHECK?"Bad type":"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/hardware/%lld/move",(long long*)&id)==1) {
        int64_t sys = json_get_int(req->body,"system_id");
        int64_t loc = json_get_int(req->body,"location_id");
        int rc = lrm_move_serialized(db,id,sys,loc,actor);
        if (rc==LRM_OK) send_ok(fd,"Moved");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/hardware/%lld/assign",(long long*)&id)==1) {
        int64_t pid = json_get_int(req->body,"project_id");
        int rc = lrm_assign_to_project(db,id,pid,actor);
        if (rc==LRM_OK) send_ok(fd,"Assigned");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/hardware/%lld/unassign",(long long*)&id)==1) {
        int rc = lrm_unassign_from_project(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Unassigned");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/hardware/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_item_status(db,id,st,actor);
        if (rc==LRM_OK) send_ok(fd,"Status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/hardware/%lld/sockets",(long long*)&id)==1) {
        SerializedHw item;
        if (lrm_get_serialized(db,id,&item)==LRM_OK) {
            Json j; j_init(&j); j_raw(&j,"{");
            j_int(&j,"item_id",id); j_comma(&j);
            j_int(&j,"socket_count",item.socket_count); j_comma(&j);
            j_raw(&j,"\"sockets\":[");
            int max_sock = item.socket_count > 0 ? item.socket_count : 64;
            if (max_sock > 64) max_sock = 64;
            for (int i=0;i<max_sock;i++) {
                if (i>0) j_comma(&j);
                SocketStatus st;
                lrm_get_socket_status(db,id,i,&st);
                j_raw(&j,"{"); j_int(&j,"index",i); j_comma(&j);
                j_int(&j,"status_id",st); j_comma(&j);
                j_str(&j,"status",socket_status_str(st)); j_raw(&j,"}");
            }
            j_raw(&j,"]}"); send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Item not found");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/hardware/%lld/sockets",(long long*)&id)==1) {
        int32_t idx = (int32_t)json_get_int(req->body,"socket_index");
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_socket_status(db,id,idx,(SocketStatus)st,actor);
        if (rc==LRM_OK) send_ok(fd,"Socket updated");
        else send_error(fd,400,rc==LRM_ERR_CHECK?"Bad index or status":"Failed");
        return;
    }

    /* ── Quantity HW ───────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/quantity/system/%lld",(long long*)&id)==1) {
        QuantityHw qtys[128]; uint32_t cnt=0;
        lrm_list_quantity_at(db,id,qtys,&cnt,128);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_quantity(&j,&qtys[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/quantity")==0) {
        QuantityHw qty={0};
        qty.type_id = json_get_int(req->body,"type_id");
        qty.system_id = json_get_int(req->body,"system_id");
        qty.total = (int32_t)json_get_int(req->body,"total");
        qty.good = (int32_t)json_get_int(req->body,"good");
        qty.bad = (int32_t)json_get_int(req->body,"bad");
        int rc = lrm_set_quantity(db,&qty,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_quantity(&j,&qty);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/quantity/%lld/adjust",(long long*)&id)==1) {
        int32_t gd = (int32_t)json_get_int(req->body,"good_delta");
        int32_t bd = (int32_t)json_get_int(req->body,"bad_delta");
        int rc = lrm_adjust_quantity(db,id,gd,bd,actor);
        if (rc==LRM_OK) send_ok(fd,"Adjusted");
        else send_error(fd,400,"Failed");
        return;
    }

    /* ── Configured HW ─────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/configured/project/%lld",(long long*)&id)==1) {
        ConfiguredHw cfgs[128]; uint32_t cnt=0;
        lrm_list_configured_for_project(db,id,cfgs,&cnt,128);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_configured(&j,&cfgs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/configured")==0) {
        ConfiguredHw cfg={0};
        cfg.type_id = json_get_int(req->body,"type_id");
        cfg.system_id = json_get_int(req->body,"system_id");
        cfg.project_id = json_get_int(req->body,"project_id");
        cfg.r0_ohms = (int32_t)json_get_int(req->body,"r0_ohms");
        cfg.r4_ohms = (int32_t)json_get_int(req->body,"r4_ohms");
        cfg.vout_mv = (int32_t)json_get_int(req->body,"vout_mv");
        cfg.role = (int32_t)json_get_int(req->body,"role");
        cfg.quantity = (int32_t)json_get_int(req->body,"quantity");
        int rc = lrm_create_configured(db,&cfg,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_configured(&j,&cfg);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/configured/%lld",(long long*)&id)==1) {
        ConfiguredHw cfg={0};
        if (lrm_get_configured(db,id,&cfg)!=LRM_OK) {
            send_error(fd,404,"Configured hardware not found");
            return;
        }
        if (json_find_key(req->body,"r0_ohms")) {
            cfg.r0_ohms = (int32_t)json_get_int(req->body,"r0_ohms");
        }
        if (json_find_key(req->body,"r4_ohms")) {
            cfg.r4_ohms = (int32_t)json_get_int(req->body,"r4_ohms");
        }
        if (json_find_key(req->body,"vout_mv")) {
            cfg.vout_mv = (int32_t)json_get_int(req->body,"vout_mv");
        }
        if (json_find_key(req->body,"role")) {
            cfg.role = (int32_t)json_get_int(req->body,"role");
        }
        /* notes field is ignored at engine level for now */
        int rc = lrm_update_configured(db,&cfg,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_configured(&j,&cfg);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }

    /* ── Hardware Types ────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/hw-types")==0) {
        HardwareType types[64]; uint32_t cnt=0;
        lrm_list_hw_types(db,types,&cnt,64);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_hw_type(&j,&types[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/hw-types")==0) {
        HardwareType ht={0};
        json_get_str(req->body,"name",ht.name,MAX_TEXT_LEN);
        ht.category = (int32_t)json_get_int(req->body,"category");
        ht.tracking = (int32_t)json_get_int(req->body,"tracking");
        ht.for_system_type = (int32_t)json_get_int(req->body,"for_system_type");
        int rc = lrm_create_hw_type(db,&ht,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_hw_type(&j,&ht);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }

    /* ── Audit ─────────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/audit/recent")==0) {
        AuditEntry logs[256]; uint32_t cnt=0;
        lrm_get_recent_log(db,logs,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_audit(&j,&logs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && match_path_str(p,"/api/audit/entity",sparam,256)) {
        /* /api/audit/entity/serialized_hw/42 → table=serialized_hw, id=42 */
        char tbl[128]={0}; int64_t eid=0;
        const char *slash = strchr(sparam,'/');
        if (slash) {
            int tlen = (int)(slash-sparam);
            if (tlen>127) tlen=127;
            memcpy(tbl,sparam,tlen); tbl[tlen]=0;
            eid = atoll(slash+1);
        }
        if (tbl[0] && eid>0) {
            AuditEntry logs[256]; uint32_t cnt=0;
            lrm_get_entity_log(db,tbl,eid,logs,&cnt,256);
            Json j; j_init(&j); j_raw(&j,"[");
            for (uint32_t i=0;i<cnt;i++) {
                if (i>0) j_comma(&j);
            json_audit(&j,&logs[i]);
            }
            j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Use /api/audit/entity/{table}/{id}");
        return;
    }

    /* ── Auth / Users ──────────────────────────────── */
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/auth/login")==0) {
        char uname[256], pw[256];
        json_get_str(req->body,"username",uname,256);
        json_get_str(req->body,"password",pw,256);
        User user;
        int rc = lrm_authenticate(db,uname,pw,&user);
        if (rc==LRM_OK) {
            api_user_id = user.user_id;
            Json j; j_init(&j); json_user(&j,&user);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,401,"Invalid credentials");
        return;
    }
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/users")==0) {
        User users[64]; uint32_t cnt=0;
        lrm_list_users(db,users,&cnt,64);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_user(&j,&users[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/users")==0) {
        User user={0}; char pw[256];
        json_get_str(req->body,"username",user.username,MAX_TEXT_LEN);
        json_get_str(req->body,"display_name",user.display_name,MAX_TEXT_LEN);
        json_get_str(req->body,"password",pw,256);
        user.role = (int32_t)json_get_int(req->body,"role");
        int rc = lrm_create_user(db,&user,pw);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_user(&j,&user);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }

    /* ── Team Members ─────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/team-members")==0) {
        TeamMember tms[64]; uint32_t cnt=0;
        lrm_list_team_members(db,tms,&cnt,64);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_team_member(&j,&tms[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/team-members")==0) {
        TeamMember tm={0};
        json_get_str(req->body,"name",tm.name,MAX_TEXT_LEN);
        json_get_str(req->body,"role",tm.role,MAX_TEXT_LEN);
        json_get_str(req->body,"primary_systems",tm.primary_systems,MAX_TEXT_LEN);
        json_get_str(req->body,"board_patterns",tm.board_patterns,MAX_TEXT_LEN);
        int rc = lrm_create_team_member(db,&tm,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_team_member(&j,&tm);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_EXISTS?"Duplicate name":"Invalid");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/team-members/%lld",(long long*)&id)==1) {
        TeamMember tm={0};
        tm.team_member_id = id;
        json_get_str(req->body,"name",tm.name,MAX_TEXT_LEN);
        json_get_str(req->body,"role",tm.role,MAX_TEXT_LEN);
        json_get_str(req->body,"primary_systems",tm.primary_systems,MAX_TEXT_LEN);
        json_get_str(req->body,"board_patterns",tm.board_patterns,MAX_TEXT_LEN);
        int rc = lrm_update_team_member(db,&tm,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_team_member(&j,&tm);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/team-members/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_team_member(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Deleted");
        else send_error(fd,404,"Not found");
        return;
    }

    /* ── Activities ────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/activities")==0) {
        DailyActivity acts[256]; uint32_t cnt=0;
        lrm_list_activities(db,acts,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_daily_activity(&j,&acts[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && match_path_str(p,"/api/activities/date",sparam,16)) {
        DailyActivity act;
        if (lrm_find_activity_by_date(db,sparam,&act)==LRM_OK) {
            /* Build nested response with all child tasks */
            UploadTask ups[64]; uint32_t ucnt=0;
            lrm_list_uploads_for_activity(db,act.activity_id,ups,&ucnt,64);
            DownloadTask dls[64]; uint32_t dcnt=0;
            lrm_list_downloads_for_activity(db,act.activity_id,dls,&dcnt,64);
            EngActivityTask ets[64]; uint32_t ecnt=0;
            lrm_list_eng_tasks_for_activity(db,act.activity_id,ets,&ecnt,64);

            Json j; j_init(&j); j_raw(&j,"{");
            j_raw(&j,"\"activity\":"); json_daily_activity(&j,&act); j_comma(&j);
            j_raw(&j,"\"uploads\":[");
            for (uint32_t i=0;i<ucnt;i++) {
                if (i>0) j_comma(&j);
            json_upload_task(&j,&ups[i]);
            }
            j_raw(&j,"],"); j_raw(&j,"\"downloads\":[");
            for (uint32_t i=0;i<dcnt;i++) {
                if (i>0) j_comma(&j);
            json_download_task(&j,&dls[i]);
            }
            j_raw(&j,"],"); j_raw(&j,"\"eng_activities\":[");
            for (uint32_t i=0;i<ecnt;i++) {
                if (i>0) j_comma(&j);
            json_eng_activity_task(&j,&ets[i]);
            }
            j_raw(&j,"]}"); send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"No activity for that date");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/activities")==0) {
        DailyActivity act={0};
        json_get_str(req->body,"date",act.date,16);
        int rc = lrm_create_activity(db,&act,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_daily_activity(&j,&act);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_EXISTS?"Duplicate date":"Invalid");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/activities/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_activity(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Deleted with children");
        else send_error(fd,404,"Not found");
        return;
    }

    /* ── Upload task create/status/assign ──────────── */
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/uploads")==0) {
        UploadTask t={0};
        t.activity_id = json_get_int(req->body,"activity_id");
        json_get_str(req->body,"load_date",t.load_date,MAX_TEXT_LEN);
        json_get_str(req->body,"customer",t.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"lot",t.lot,MAX_TEXT_LEN);
        json_get_str(req->body,"ise_id",t.ise_id,MAX_TEXT_LEN);
        t.qty = (int32_t)json_get_int(req->body,"qty");
        json_get_str(req->body,"device",t.device,MAX_TEXT_LEN);
        json_get_str(req->body,"time_at_lab",t.time_at_lab,MAX_TEXT_LEN);
        json_get_str(req->body,"notes",t.notes,MAX_TEXT_LEN);
        int rc = lrm_create_upload(db,&t,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_upload_task(&j,&t);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_FK?"Bad activity_id":"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/uploads/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_upload_status(db,id,(UploadStatus)st,actor);
        if (rc==LRM_OK) send_ok(fd,"Status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/uploads/%lld/assign",(long long*)&id)==1) {
        int64_t tmid = json_get_int(req->body,"team_member_id");
        int rc = lrm_assign_upload(db,id,tmid,actor);
        if (rc==LRM_OK) send_ok(fd,"Assigned");
        else send_error(fd,400,rc==LRM_ERR_FK?"Bad team member":"Failed");
        return;
    }

    /* ── Download task create/status/assign ─────────── */
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/downloads")==0) {
        DownloadTask t={0};
        t.activity_id = json_get_int(req->body,"activity_id");
        json_get_str(req->body,"customer",t.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"lot",t.lot,MAX_TEXT_LEN);
        json_get_str(req->body,"ise_id",t.ise_id,MAX_TEXT_LEN);
        t.qty = (int32_t)json_get_int(req->body,"qty");
        json_get_str(req->body,"device",t.device,MAX_TEXT_LEN);
        json_get_str(req->body,"download_time",t.download_time,MAX_TEXT_LEN);
        json_get_str(req->body,"notes",t.notes,MAX_TEXT_LEN);
        int rc = lrm_create_download(db,&t,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_download_task(&j,&t);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_FK?"Bad activity_id":"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/downloads/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_download_status(db,id,(TaskStatus)st,actor);
        if (rc==LRM_OK) send_ok(fd,"Status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/downloads/%lld/assign",(long long*)&id)==1) {
        int64_t tmid = json_get_int(req->body,"team_member_id");
        int rc = lrm_assign_download(db,id,tmid,actor);
        if (rc==LRM_OK) send_ok(fd,"Assigned");
        else send_error(fd,400,rc==LRM_ERR_FK?"Bad team member":"Failed");
        return;
    }

    /* ── Eng task create/status/assign ────────────────── */
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/eng-tasks")==0) {
        EngActivityTask t={0};
        t.activity_id = json_get_int(req->body,"activity_id");
        json_get_str(req->body,"customer",t.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"device",t.device,MAX_TEXT_LEN);
        json_get_str(req->body,"description",t.description,MAX_TEXT_LEN);
        json_get_str(req->body,"ise_numbers",t.ise_numbers,MAX_TEXT_LEN);
        int rc = lrm_create_eng_task(db,&t,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_eng_activity_task(&j,&t);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_FK?"Bad activity_id":"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/eng-tasks/%lld/status",(long long*)&id)==1) {
        int32_t st = (int32_t)json_get_int(req->body,"status");
        int rc = lrm_set_eng_task_status(db,id,(TaskStatus)st,actor);
        if (rc==LRM_OK) send_ok(fd,"Status updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/eng-tasks/%lld/assign",(long long*)&id)==1) {
        int64_t tmid = json_get_int(req->body,"team_member_id");
        int rc = lrm_assign_eng_task(db,id,tmid,actor);
        if (rc==LRM_OK) send_ok(fd,"Assigned");
        else send_error(fd,400,rc==LRM_ERR_FK?"Bad team member":"Failed");
        return;
    }

    /* ── Engineering Hours ─────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/engineering-hours")==0) {
        EngHoursEntry entries[256]; uint32_t cnt=0;
        lrm_list_eng_hours(db,entries,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_eng_hours(&j,&entries[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/engineering-hours/stats")==0) {
        EngHoursEntry entries[256]; uint32_t cnt=0;
        lrm_list_eng_hours(db,entries,&cnt,256);
        int32_t total=0, billable=0, non_billable=0;
        for (uint32_t i=0;i<cnt;i++) {
            total += entries[i].hours_hundredths;
            if (entries[i].billable) billable += entries[i].hours_hundredths;
            else non_billable += entries[i].hours_hundredths;
        }
        Json j; j_init(&j); j_raw(&j,"{");
        j_grow(&j,128);
        j.len += sprintf(j.buf+j.len,
            "\"total_hours\":%.2f,\"billable_hours\":%.2f,"
            "\"non_billable_hours\":%.2f,\"entry_count\":%u",
            total/100.0, billable/100.0, non_billable/100.0, cnt);
        j_raw(&j,"}"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/engineering-hours")==0) {
        EngHoursEntry entry={0};
        json_get_str(req->body,"date",entry.date,16);
        json_get_str(req->body,"customer",entry.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"project",entry.project,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number",entry.pcb_number,MAX_TEXT_LEN);
        json_get_str(req->body,"description",entry.description,MAX_TEXT_LEN);
        json_get_str(req->body,"engineer",entry.engineer,MAX_TEXT_LEN);
        entry.hours_hundredths = (int32_t)(json_get_float(req->body,"hours_spent") * 100.0 + 0.5);
        entry.billable = (int32_t)json_get_int(req->body,"billable");
        entry.quoted_hours_hundredths = (int32_t)(json_get_float(req->body,"quoted_hours") * 100.0 + 0.5);
        json_get_str(req->body,"po_number",entry.po_number,MAX_TEXT_LEN);
        entry.source_task_id = json_get_int(req->body,"source_task_id");
        int rc = lrm_create_eng_hours(db,&entry,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_eng_hours(&j,&entry);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/engineering-hours/%lld",(long long*)&id)==1) {
        EngHoursEntry entry={0};
        entry.entry_id = id;
        json_get_str(req->body,"date",entry.date,16);
        json_get_str(req->body,"customer",entry.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"project",entry.project,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number",entry.pcb_number,MAX_TEXT_LEN);
        json_get_str(req->body,"description",entry.description,MAX_TEXT_LEN);
        json_get_str(req->body,"engineer",entry.engineer,MAX_TEXT_LEN);
        entry.hours_hundredths = (int32_t)(json_get_float(req->body,"hours_spent") * 100.0 + 0.5);
        entry.billable = (int32_t)json_get_int(req->body,"billable");
        entry.quoted_hours_hundredths = (int32_t)(json_get_float(req->body,"quoted_hours") * 100.0 + 0.5);
        json_get_str(req->body,"po_number",entry.po_number,MAX_TEXT_LEN);
        entry.source_task_id = json_get_int(req->body,"source_task_id");
        int rc = lrm_update_eng_hours(db,&entry,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_eng_hours(&j,&entry);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/engineering-hours/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_eng_hours(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Deleted");
        else send_error(fd,404,"Not found");
        return;
    }

    /* ── User additions ───────────────────────────── */
    if (strcmp(m,"GET")==0 && strncmp(p,"/api/users/by-name/",19)==0) {
        const char *uname = p + 19;
        User user;
        if (lrm_find_user_by_name(db,uname,&user)==LRM_OK) {
            Json j; j_init(&j); json_user_auth(&j,&user);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"User not found");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/users/%lld/password-hash",(long long*)&id)==1) {
        char hash[HASH_LEN];
        json_get_str(req->body,"password_hash",hash,HASH_LEN);
        int rc = lrm_set_user_password_hash(db,id,hash);
        if (rc==LRM_OK) send_ok(fd,"Password hash updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/users/%lld/password",(long long*)&id)==1) {
        char pw[256];
        json_get_str(req->body,"password",pw,256);
        int rc = lrm_reset_password(db,id,pw);
        if (rc==LRM_OK) send_ok(fd,"Password updated");
        else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/users/%lld/role",(long long*)&id)==1) {
        int32_t role = (int32_t)json_get_int(req->body,"role");
        int rc = lrm_set_user_role(db, id, role, actor);
        if (rc==LRM_OK) {
            User u;
            if (lrm_get_user(db, id, &u)==LRM_OK) {
                Json j; j_init(&j);
                j_raw(&j,"{\"ok\":true,");
                j_int(&j,"user_id",u.user_id); j_comma(&j);
                j_str(&j,"role",user_role_str((UserRole)u.role)); j_comma(&j);
                j_int(&j,"role_id",u.role);
                j_raw(&j,"}");
                send_json(fd,200,j.buf); free(j.buf);
            } else send_ok(fd,"Role updated");
        } else if (rc==LRM_ERR_CHECK) send_error(fd,400,"Invalid role or cannot demote last admin");
        else if (rc==LRM_ERR_NOTFOUND) send_error(fd,404,"User not found");
        else send_error(fd,400,"Failed");
        return;
    }
    {   int n=0;
        if (strcmp(m,"POST")==0 && sscanf(p,"/api/users/%lld/change-password%n",(long long*)&id,&n)==1 && n>0 && p[n]=='\0') {
            char old_pw[256], new_pw[256];
            json_get_str(req->body,"old_password",old_pw,256);
            json_get_str(req->body,"new_password",new_pw,256);
            int rc = lrm_change_password(db,id,old_pw,new_pw);
            if (rc==LRM_OK) send_ok(fd,"Password changed");
            else if (rc==LRM_ERR_CHECK) send_error(fd,401,"Current password incorrect");
            else send_error(fd,400,"Failed");
            return;
        }
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/users/%lld/toggle-active",(long long*)&id)==1) {
        /* Read current state and flip it */
        User u; int rc = lrm_get_user(db, id, &u);
        if (rc!=LRM_OK) { send_error(fd,404,"User not found"); return; }
        int32_t new_active = u.active ? 0 : 1;
        rc = lrm_set_user_active(db,id,new_active,actor);
        if (rc==LRM_OK) {
            char buf[128]; snprintf(buf,128,"{\"ok\":true,\"active\":%d}",new_active);
            send_json(fd,200,buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/users/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_user(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"User deleted");
        else send_error(fd,404,"Not found");
        return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/users/%lld",(long long*)&id)==1) {
        User user;
        if (lrm_get_user(db,id,&user)==LRM_OK) {
            Json j; j_init(&j); json_user(&j,&user);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"User not found");
        return;
    }

    /* ── Location additions ──────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/locations")==0) {
        Location locs[512]; uint32_t cnt=0;
        lrm_list_all_locations(db,locs,&cnt,512);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_location(&j,&locs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/locations/%lld",(long long*)&id)==1 &&
        !strstr(p,"/children") && !strstr(p,"/status") && !strstr(p,"/rename") && !strstr(p,"/move")) {
        Location loc;
        if (lrm_get_location(db,id,&loc)==LRM_OK) {
            Json j; j_init(&j); json_location(&j,&loc);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Location not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/locations")==0) {
        Location loc={0};
        json_get_str(req->body,"name",loc.name,MAX_TEXT_LEN);
        loc.parent_id = json_get_int(req->body,"parent_id");
        loc.system_id = json_get_int(req->body,"system_id");
        loc.loc_type = LOC_STORAGE;
        loc.status = LSTAT_ACTIVE;
        int rc = lrm_create_location(db,&loc,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_location(&j,&loc);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/locations/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_location(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Location deleted");
        else send_error(fd,404,"Not found");
        return;
    }

    /* ── V1 Boards ───────────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/v1/boards")==0) {
        V1Board boards[512]; uint32_t cnt=0;
        lrm_list_v1_boards(db,boards,&cnt,512);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_v1_board(&j,&boards[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && strncmp(p,"/api/v1/boards/find/",20)==0) {
        char customer[256]={0}, pcb[256]={0}, serial[256]={0};
        const char *rest = p + 20;
        const char *s1 = strchr(rest, '/');
        if (s1) {
            int clen = (int)(s1-rest); if (clen>255) clen=255;
            memcpy(customer, rest, clen); customer[clen]=0;
            const char *s2 = strchr(s1+1, '/');
            if (s2) {
                int plen = (int)(s2-s1-1); if (plen>255) plen=255;
                memcpy(pcb, s1+1, plen); pcb[plen]=0;
                strncpy(serial, s2+1, 255);
            }
        }
        V1Board board;
        if (customer[0] && pcb[0] && serial[0] &&
            lrm_find_v1_board(db,customer,pcb,serial,&board)==LRM_OK) {
            Json j; j_init(&j); json_v1_board(&j,&board);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Board not found");
        return;
    }
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/v1/boards/%lld",(long long*)&id)==1 &&
        !strstr(p,"/find")) {
        V1Board board;
        if (lrm_get_v1_board(db,id,&board)==LRM_OK) {
            Json j; j_init(&j); json_v1_board(&j,&board);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Board not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/boards")==0) {
        V1Board b={0};
        json_get_str(req->body,"customer",b.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"platform",b.platform,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number_text",b.pcb_number_text,MAX_TEXT_LEN);
        json_get_str(req->body,"revision",b.revision,64);
        json_get_str(req->body,"serial_no",b.serial_no,MAX_TEXT_LEN);
        b.power_qty = (int32_t)json_get_int(req->body,"power_qty");
        json_get_str(req->body,"status",b.status,64);
        b.location_id = json_get_int(req->body,"location_id");
        b.socket_rows = (int32_t)json_get_int(req->body,"socket_rows");
        b.socket_cols = (int32_t)json_get_int(req->body,"socket_cols");
        json_get_str(req->body,"notes",b.notes,MAX_TEXT_LEN);
        json_get_str(req->body,"individual_notes",b.individual_notes,MAX_TEXT_LEN);
        json_get_str(req->body,"date_created",b.date_created,32);
        json_get_str(req->body,"last_used_date",b.last_used_date,32);
        b.sockets_working = (int32_t)json_get_int(req->body,"sockets_working");
        b.sockets_bad = (int32_t)json_get_int(req->body,"sockets_bad");
        b.sockets_not_installed = (int32_t)json_get_int(req->body,"sockets_not_installed");
        int rc = lrm_create_v1_board(db,&b,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_board(&j,&b);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_CHECK?"Missing customer/serial":"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/v1/boards/%lld",(long long*)&id)==1) {
        V1Board b={0};
        b.board_id = id;
        json_get_str(req->body,"customer",b.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"platform",b.platform,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number_text",b.pcb_number_text,MAX_TEXT_LEN);
        json_get_str(req->body,"revision",b.revision,64);
        json_get_str(req->body,"serial_no",b.serial_no,MAX_TEXT_LEN);
        b.power_qty = (int32_t)json_get_int(req->body,"power_qty");
        json_get_str(req->body,"status",b.status,64);
        b.location_id = json_get_int(req->body,"location_id");
        b.socket_rows = (int32_t)json_get_int(req->body,"socket_rows");
        b.socket_cols = (int32_t)json_get_int(req->body,"socket_cols");
        json_get_str(req->body,"notes",b.notes,MAX_TEXT_LEN);
        json_get_str(req->body,"individual_notes",b.individual_notes,MAX_TEXT_LEN);
        json_get_str(req->body,"date_created",b.date_created,32);
        json_get_str(req->body,"last_used_date",b.last_used_date,32);
        b.sockets_working = (int32_t)json_get_int(req->body,"sockets_working");
        b.sockets_bad = (int32_t)json_get_int(req->body,"sockets_bad");
        b.sockets_not_installed = (int32_t)json_get_int(req->body,"sockets_not_installed");
        int rc = lrm_update_v1_board(db,&b,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_board(&j,&b);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }

    /* ── V1 Board Types ──────────────────────────────── */
    if (strcmp(m,"GET")==0 && strcmp(p,"/api/v1/board-types")==0) {
        V1BoardType types[256]; uint32_t cnt=0;
        lrm_list_v1_board_types(db,types,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_v1_board_type(&j,&types[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"GET")==0 && strncmp(p,"/api/v1/board-types/find/",25)==0) {
        char customer[256]={0}, pcb[256]={0}, rev[256]={0};
        const char *rest = p + 25;
        const char *s1 = strchr(rest, '/');
        if (s1) {
            int clen = (int)(s1-rest); if (clen>255) clen=255;
            memcpy(customer, rest, clen); customer[clen]=0;
            const char *s2 = strchr(s1+1, '/');
            if (s2) {
                int plen = (int)(s2-s1-1); if (plen>255) plen=255;
                memcpy(pcb, s1+1, plen); pcb[plen]=0;
                strncpy(rev, s2+1, 255);
            }
        }
        V1BoardType bt;
        if (customer[0] && pcb[0] &&
            lrm_find_v1_board_type(db,customer,pcb,rev,&bt)==LRM_OK) {
            Json j; j_init(&j); json_v1_board_type(&j,&bt);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,404,"Board type not found");
        return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/board-types")==0) {
        V1BoardType bt={0};
        json_get_str(req->body,"customer",bt.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number_text",bt.pcb_number_text,MAX_TEXT_LEN);
        json_get_str(req->body,"revision",bt.revision,64);
        json_get_str(req->body,"platform",bt.platform,MAX_TEXT_LEN);
        bt.power_qty = (int32_t)json_get_int(req->body,"power_qty");
        bt.socket_rows = (int32_t)json_get_int(req->body,"socket_rows");
        bt.socket_cols = (int32_t)json_get_int(req->body,"socket_cols");
        json_get_str(req->body,"notes",bt.notes,MAX_TEXT_LEN);
        bt.is_default = (int32_t)json_get_int(req->body,"is_default");
        json_get_str(req->body,"devices",bt.devices,MAX_TEXT_LEN);
        int rc = lrm_create_v1_board_type(db,&bt,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_board_type(&j,&bt);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_CHECK?"Missing customer/pcb":"Failed");
        return;
    }
    if (strcmp(m,"PUT")==0 && sscanf(p,"/api/v1/board-types/%lld",(long long*)&id)==1) {
        V1BoardType bt={0};
        bt.board_type_id = id;
        json_get_str(req->body,"customer",bt.customer,MAX_TEXT_LEN);
        json_get_str(req->body,"pcb_number_text",bt.pcb_number_text,MAX_TEXT_LEN);
        json_get_str(req->body,"revision",bt.revision,64);
        json_get_str(req->body,"platform",bt.platform,MAX_TEXT_LEN);
        bt.power_qty = (int32_t)json_get_int(req->body,"power_qty");
        bt.socket_rows = (int32_t)json_get_int(req->body,"socket_rows");
        bt.socket_cols = (int32_t)json_get_int(req->body,"socket_cols");
        json_get_str(req->body,"notes",bt.notes,MAX_TEXT_LEN);
        bt.is_default = (int32_t)json_get_int(req->body,"is_default");
        json_get_str(req->body,"devices",bt.devices,MAX_TEXT_LEN);
        int rc = lrm_update_v1_board_type(db,&bt,actor);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_board_type(&j,&bt);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }
    if (strcmp(m,"DELETE")==0 && sscanf(p,"/api/v1/board-types/%lld",(long long*)&id)==1) {
        int rc = lrm_delete_v1_board_type(db,id,actor);
        if (rc==LRM_OK) send_ok(fd,"Board type deleted");
        else send_error(fd,404,"Not found");
        return;
    }

    /* ── V1 Board Logs ───────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/v1/board-logs/%lld",(long long*)&id)==1) {
        V1BoardLog logs[256]; uint32_t cnt=0;
        lrm_list_v1_board_logs(db,id,logs,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_v1_board_log(&j,&logs[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/board-logs")==0) {
        V1BoardLog log={0};
        log.board_id = json_get_int(req->body,"board_id");
        json_get_str(req->body,"timestamp",log.timestamp,32);
        json_get_str(req->body,"user",log.user,MAX_TEXT_LEN);
        json_get_str(req->body,"action",log.action,MAX_TEXT_LEN);
        json_get_str(req->body,"details",log.details,MAX_TEXT_LEN);
        log.from_location_id = json_get_int(req->body,"from_location_id");
        log.to_location_id = json_get_int(req->body,"to_location_id");
        int rc = lrm_create_v1_board_log(db,&log);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_board_log(&j,&log);
            send_json(fd,201,j.buf); free(j.buf);
        } else send_error(fd,400,rc==LRM_ERR_CHECK?"Missing board_id":"Failed");
        return;
    }

    /* ── V1 Socket Notes ─────────────────────────────── */
    if (strcmp(m,"GET")==0 && sscanf(p,"/api/v1/sockets/%lld",(long long*)&id)==1) {
        V1SocketNote notes[256]; uint32_t cnt=0;
        lrm_list_v1_sockets(db,id,notes,&cnt,256);
        Json j; j_init(&j); j_raw(&j,"[");
        for (uint32_t i=0;i<cnt;i++) {
            if (i>0) j_comma(&j);
            json_v1_socket_note(&j,&notes[i]);
        }
        j_raw(&j,"]"); send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && sscanf(p,"/api/v1/sockets/%lld",(long long*)&id)==1) {
        V1SocketNote sn={0};
        sn.board_id = id;
        sn.socket_number = (int32_t)json_get_int(req->body,"socket_number");
        json_get_str(req->body,"status",sn.status,64);
        json_get_str(req->body,"note",sn.note,MAX_TEXT_LEN);
        int rc = lrm_upsert_v1_socket(db,&sn);
        if (rc==LRM_OK) {
            Json j; j_init(&j); json_v1_socket_note(&j,&sn);
            send_json(fd,200,j.buf); free(j.buf);
        } else send_error(fd,400,"Failed");
        return;
    }

    /* ── Batch Import Endpoints ─────────────────────── */

    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/batch/locations")==0) {
        const char *arr = json_find_array(req->body,"items");
        if (!arr) { send_error(fd,400,"Missing items array"); return; }
        char obj[4096]; int created=0,errors=0;
        Json j; j_init(&j); j_raw(&j,"{\"ids\":[");
        while (json_next_object(&arr, obj, sizeof(obj))) {
            Location loc={0};
            json_get_str(obj,"name",loc.name,MAX_TEXT_LEN);
            loc.parent_id = json_get_int(obj,"parent_id");
            loc.system_id = json_get_int(obj,"system_id");
            loc.loc_type = LOC_STORAGE;
            loc.status = LSTAT_ACTIVE;
            if (lrm_create_location(db,&loc,actor)==LRM_OK) {
                if (created>0) j_raw(&j,",");
                char tmp[32]; snprintf(tmp,32,"%lld",(long long)loc.location_id);
                j_raw(&j,tmp); created++;
            } else errors++;
        }
        j_raw(&j,"],");
        j_int(&j,"created",created); j_comma(&j);
        j_int(&j,"errors",errors); j_raw(&j,"}");
        send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/batch/board-types")==0) {
        const char *arr = json_find_array(req->body,"items");
        if (!arr) { send_error(fd,400,"Missing items array"); return; }
        char obj[4096]; int created=0,errors=0;
        Json j; j_init(&j); j_raw(&j,"{\"ids\":[");
        while (json_next_object(&arr, obj, sizeof(obj))) {
            V1BoardType bt={0};
            json_get_str(obj,"customer",bt.customer,MAX_TEXT_LEN);
            json_get_str(obj,"pcb_number_text",bt.pcb_number_text,MAX_TEXT_LEN);
            json_get_str(obj,"revision",bt.revision,64);
            json_get_str(obj,"platform",bt.platform,MAX_TEXT_LEN);
            bt.power_qty = (int32_t)json_get_int(obj,"power_qty");
            bt.socket_rows = (int32_t)json_get_int(obj,"socket_rows");
            bt.socket_cols = (int32_t)json_get_int(obj,"socket_cols");
            json_get_str(obj,"notes",bt.notes,MAX_TEXT_LEN);
            bt.is_default = (int32_t)json_get_int(obj,"is_default");
            json_get_str(obj,"devices",bt.devices,MAX_TEXT_LEN);
            if (lrm_create_v1_board_type(db,&bt,actor)==LRM_OK) {
                if (created>0) j_raw(&j,",");
                char tmp[32]; snprintf(tmp,32,"%lld",(long long)bt.board_type_id);
                j_raw(&j,tmp); created++;
            } else errors++;
        }
        j_raw(&j,"],");
        j_int(&j,"created",created); j_comma(&j);
        j_int(&j,"errors",errors); j_raw(&j,"}");
        send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/batch/boards")==0) {
        const char *arr = json_find_array(req->body,"items");
        if (!arr) { send_error(fd,400,"Missing items array"); return; }
        char obj[4096]; int created=0,errors=0;
        Json j; j_init(&j); j_raw(&j,"{\"ids\":[");
        while (json_next_object(&arr, obj, sizeof(obj))) {
            V1Board b={0};
            json_get_str(obj,"customer",b.customer,MAX_TEXT_LEN);
            json_get_str(obj,"platform",b.platform,MAX_TEXT_LEN);
            json_get_str(obj,"pcb_number_text",b.pcb_number_text,MAX_TEXT_LEN);
            json_get_str(obj,"revision",b.revision,64);
            json_get_str(obj,"serial_no",b.serial_no,MAX_TEXT_LEN);
            b.power_qty = (int32_t)json_get_int(obj,"power_qty");
            json_get_str(obj,"status",b.status,64);
            b.location_id = json_get_int(obj,"location_id");
            b.socket_rows = (int32_t)json_get_int(obj,"socket_rows");
            b.socket_cols = (int32_t)json_get_int(obj,"socket_cols");
            json_get_str(obj,"notes",b.notes,MAX_TEXT_LEN);
            json_get_str(obj,"individual_notes",b.individual_notes,MAX_TEXT_LEN);
            json_get_str(obj,"date_created",b.date_created,32);
            json_get_str(obj,"last_used_date",b.last_used_date,32);
            b.sockets_working = (int32_t)json_get_int(obj,"sockets_working");
            b.sockets_bad = (int32_t)json_get_int(obj,"sockets_bad");
            b.sockets_not_installed = (int32_t)json_get_int(obj,"sockets_not_installed");
            if (lrm_create_v1_board(db,&b,actor)==LRM_OK) {
                if (created>0) j_raw(&j,",");
                char tmp[32]; snprintf(tmp,32,"%lld",(long long)b.board_id);
                j_raw(&j,tmp); created++;
            } else errors++;
        }
        j_raw(&j,"],");
        j_int(&j,"created",created); j_comma(&j);
        j_int(&j,"errors",errors); j_raw(&j,"}");
        send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/batch/board-logs")==0) {
        const char *arr = json_find_array(req->body,"items");
        if (!arr) { send_error(fd,400,"Missing items array"); return; }
        char obj[4096]; int created=0,errors=0;
        while (json_next_object(&arr, obj, sizeof(obj))) {
            V1BoardLog log={0};
            log.board_id = json_get_int(obj,"board_id");
            json_get_str(obj,"timestamp",log.timestamp,32);
            json_get_str(obj,"user",log.user,MAX_TEXT_LEN);
            json_get_str(obj,"action",log.action,MAX_TEXT_LEN);
            json_get_str(obj,"details",log.details,MAX_TEXT_LEN);
            log.from_location_id = json_get_int(obj,"from_location_id");
            log.to_location_id = json_get_int(obj,"to_location_id");
            if (lrm_create_v1_board_log(db,&log)==LRM_OK) created++;
            else errors++;
        }
        Json j; j_init(&j); j_raw(&j,"{");
        j_int(&j,"created",created); j_comma(&j);
        j_int(&j,"errors",errors); j_raw(&j,"}");
        send_json(fd,200,j.buf); free(j.buf); return;
    }
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/v1/batch/socket-notes")==0) {
        const char *arr = json_find_array(req->body,"items");
        if (!arr) { send_error(fd,400,"Missing items array"); return; }
        char obj[4096]; int created=0,errors=0;
        while (json_next_object(&arr, obj, sizeof(obj))) {
            V1SocketNote sn={0};
            sn.board_id = json_get_int(obj,"board_id");
            sn.socket_number = (int32_t)json_get_int(obj,"socket_number");
            json_get_str(obj,"status",sn.status,64);
            json_get_str(obj,"note",sn.note,MAX_TEXT_LEN);
            if (lrm_upsert_v1_socket(db,&sn)==LRM_OK) created++;
            else errors++;
        }
        Json j; j_init(&j); j_raw(&j,"{");
        j_int(&j,"created",created); j_comma(&j);
        j_int(&j,"errors",errors); j_raw(&j,"}");
        send_json(fd,200,j.buf); free(j.buf); return;
    }

    /* ── Import File (bridges import.c to HTTP) ─── */
    if (strcmp(m,"POST")==0 && strcmp(p,"/api/import/file")==0) {
        char filepath[512]={0};
        json_get_str(req->body,"filepath",filepath,512);
        int64_t system_id = json_get_int(req->body,"system_id");
        if (!filepath[0]) { send_error(fd,400,"Missing filepath"); return; }
        int rc = lrm_import_file(db, filepath, system_id, actor);
        if (rc==LRM_OK) send_ok(fd,"File imported");
        else send_error(fd,400,"Import failed");
        return;
    }

    /* ── 404 ───────────────────────────────────────── */
    send_error(fd, 404, "Not found");
}

/* ═══ Server Main Loop ══════════════════════════════════ */

static volatile int server_running = 1;
static void sigint_handler(int sig) { (void)sig; server_running = 0; }

int http_serve(HttpServer *srv, Database *db, int port) {
    srv->db = db;
    srv->port = port;
    sock_init();

    signal(SIGINT, sigint_handler);
#ifndef _WIN32
    signal(SIGPIPE, SIG_IGN);
#endif

    srv->server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (srv->server_fd < 0) { perror("socket"); return -1; }

    int opt = 1;
    setsockopt(srv->server_fd, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));

    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons(port);

    if (bind(srv->server_fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("bind"); close_socket(srv->server_fd); return -1;
    }
    if (listen(srv->server_fd, 16) < 0) {
        perror("listen"); close_socket(srv->server_fd); return -1;
    }

    printf("[http] Listening on http://0.0.0.0:%d\n", port);
    printf("[http] CORS enabled — any browser can connect\n");
    printf("[http] Press Ctrl+C to stop\n\n");

    srv->running = 1;
    while (server_running) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(srv->server_fd, (struct sockaddr*)&client_addr,
                               &client_len);
        if (client_fd < 0) continue;

        /* Rate limit before parsing body (exempt loopback) */
        uint32_t cip = client_addr.sin_addr.s_addr;
        if (cip != htonl(INADDR_LOOPBACK) && !rate_check(cip)) {
            send_error(client_fd, 429, "Too many requests");
            close_socket(client_fd);
            continue;
        }

        HttpReq req;
        if (parse_request(client_fd, &req) == 0) {
            printf("[http] %s %s\n", req.method, req.path);
            handle_request(db, client_fd, &req);
        } else {
            send_error(client_fd, 400, "Malformed request");
        }

        close_socket(client_fd);
    }

    close_socket(srv->server_fd);
    printf("[http] Server stopped\n");
    sock_cleanup();
    return 0;
}

void http_stop(HttpServer *srv) {
    server_running = 0;
    if (srv->server_fd > 0) close_socket(srv->server_fd);
}
