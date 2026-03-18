/*
 * ir.c — Pattern IR lifecycle: init, free, add signals/vectors, encode
 */

#include "pc.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* ═══════════════════════════════════════════════════════════════
 * CHARACTER ↔ PINSTATE CONVERSION
 * ═══════════════════════════════════════════════════════════════ */

PinState pc_char_to_state(char c)
{
    switch (c) {
    case '0':           return PS_DRIVE_0;
    case '1':           return PS_DRIVE_1;
    case 'L': case 'l': return PS_EXPECT_L;
    case 'H': case 'h': return PS_EXPECT_H;
    case 'X': case 'x':
    case '.':           return PS_DONT_CARE;
    case 'Z': case 'z': return PS_HIGH_Z;
    case 'P': case 'p': return PS_PULSE;
    case 'N': case 'n': return PS_NEG_PULSE;
    case 'U': case 'u': return PS_RISING;
    case 'D': case 'd': return PS_FALLING;
    case 'T': case 't': return PS_TERMINATE;
    case 'C': case 'c': return PS_CLOCK;
    default:            return PS_DONT_CARE;
    }
}

char pc_state_to_char(PinState s)
{
    static const char map[PS__COUNT] = {
        '0', '1', 'L', 'H', 'X', 'Z', 'P', 'N', 'U', 'D', 'T', 'C'
    };
    if (s >= 0 && s < PS__COUNT) return map[s];
    return '?';
}

/* ═══════════════════════════════════════════════════════════════
 * PATTERN LIFECYCLE
 * ═══════════════════════════════════════════════════════════════ */

void pc_pattern_init(PcPattern *p, const char *name)
{
    memset(p, 0, sizeof(*p));
    if (name) {
        strncpy(p->name, name, PC_MAX_NAME - 1);
        p->name[PC_MAX_NAME - 1] = '\0';
    }
}

void pc_pattern_free(PcPattern *p)
{
    if (p->vectors) {
        free(p->vectors);
        p->vectors = NULL;
    }
    p->num_vectors = 0;
    p->cap_vectors = 0;
}

int pc_pattern_add_signal(PcPattern *p, const char *name)
{
    if (p->num_signals >= PC_MAX_SIG) return -1;

    PcSignal *s = &p->signals[p->num_signals];
    strncpy(s->name, name, PC_MAX_NAME - 1);
    s->name[PC_MAX_NAME - 1] = '\0';
    s->channel = -1;
    s->pin_type = 0;

    return p->num_signals++;
}

int pc_pattern_add_vector(PcPattern *p, const PcVector *v)
{
    if (p->num_vectors >= p->cap_vectors) {
        int new_cap = p->cap_vectors ? p->cap_vectors * 2 : 64;
        PcVector *nv = (PcVector *)realloc(p->vectors,
                                            (size_t)new_cap * sizeof(PcVector));
        if (!nv) return PC_ERR_ALLOC;
        p->vectors = nv;
        p->cap_vectors = new_cap;
    }
    p->vectors[p->num_vectors++] = *v;
    return PC_OK;
}

/* ═══════════════════════════════════════════════════════════════
 * VECTOR ENCODING — PcVector → PcHexVector (40 bytes)
 * ═══════════════════════════════════════════════════════════════ */

void pc_encode_vector(const PcVector *v, PcHexVector *out)
{
    out->value_lo = 0;
    out->value_hi = 0;
    out->oen_lo   = 0;
    out->oen_hi   = 0;
    out->repeat   = v->repeat;

    for (int ch = 0; ch < PC_MAX_CH; ch++) {
        PinState s = (PinState)v->states[ch];
        if (s >= PS__COUNT) s = PS_DONT_CARE;

        uint8_t val = STATE_VALUE[s];
        uint8_t oen = STATE_OEN[s];

        if (ch < 64) {
            uint64_t bit = (uint64_t)1 << ch;
            if (val) out->value_lo |= bit;
            if (oen) out->oen_lo   |= bit;
        } else {
            uint64_t bit = (uint64_t)1 << (ch - 64);
            if (val) out->value_hi |= bit;
            if (oen) out->oen_hi   |= bit;
        }
    }
}

/* ═══════════════════════════════════════════════════════════════
 * IDENTITY PIN MAP
 * ═══════════════════════════════════════════════════════════════ */

void pc_apply_identity_map(PcPattern *p)
{
    for (int i = 0; i < p->num_signals && i < PC_MAX_CH; i++)
        p->signals[i].channel = i;
    p->mapped = 1;
}
