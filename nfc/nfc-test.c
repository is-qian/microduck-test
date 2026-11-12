/* NFC ESL tag test tool: poll Type A/V tags, ISO-DEP APDU exchange (FMSC ESL) */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <time.h>
#include "rfal_nfc.h"
#include "rfal_platform.h"
#include "rfal_rf.h"
#include "rfal_chip.h"
#include "rfal_nfcv.h"
#include "rfal_nfca.h"
#include "rfal_t2t.h"
#include "st25r200_com.h"
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"

void rfal_platform_write_reg(uint8_t addr, uint8_t val);

#define ESL_W 240
#define ESL_H 240
#define ESL_PLANES 4

static double now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1e6;
}

/* 6-color palette: RGB + 4-bit code from tag TLV E0 */
static const struct { uint8_t r, g, b, code; } palette[] = {
    {   0,   0,   0, 0x00 },   /* black  */
    { 255, 255, 255, 0x01 },   /* white  */
    { 255, 255,   0, 0x02 },   /* yellow */
    { 255,   0,   0, 0x03 },   /* red    */
    {   0,   0, 255, 0x05 },   /* blue   */
    {   0, 255,   0, 0x06 },   /* green  */
};

static void print_hex(const char *tag, const uint8_t *d, uint32_t n)
{
    printf("%s ", tag);
    for (uint32_t i = 0; i < n; i++)
        printf("%02X", d[i]);
    printf("\n");
    fflush(stdout);
}

static int hex2bin(const char *s, uint8_t *out)
{
    int n = 0;
    while (s[0] && s[1]) {
        unsigned v;
        if (sscanf(s, "%2x", &v) != 1)
            return -1;
        out[n++] = (uint8_t)v;
        s += 2;
    }
    return n;
}

/* run discovery until a device is activated or timeout; returns 0 if activated */
static int activate(uint32_t timeout_ms)
{
    rfalNfcDiscoverParam dp;
    rfalNfcDefaultDiscParams(&dp);
    dp.techs2Find = RFAL_NFC_POLL_TECH_A | RFAL_NFC_POLL_TECH_B | RFAL_NFC_POLL_TECH_V;
    dp.totalDuration = 500;
    dp.devLimit = 1;
    dp.maxBR = RFAL_BR_106;
    dp.isoDepFS = RFAL_ISODEP_FSXI_256;

    rfalNfcDiscover(&dp);
    uint32_t t0 = rfal_platform_systick();
    rfalNfcState last = RFAL_NFC_STATE_NOTINIT;
    while (rfal_platform_systick() - t0 < timeout_ms) {
        rfalNfcWorker();
        rfalNfcState st = rfalNfcGetState();
        if (st != last) { printf("[state %d->%d]\n", last, st); last = st; }
        if (st == RFAL_NFC_STATE_ACTIVATED)
            return 0;
        if (st == RFAL_NFC_STATE_IDLE) {
            printf("[restart discovery]\n");
            rfalNfcDeactivate(RFAL_NFC_DEACTIVATE_IDLE);
            rfalNfcDiscover(&dp);
        }
    }
    return -1;
}

static void print_device(void)
{
    rfalNfcDevice *dev = NULL;
    if (rfalNfcGetActiveDevice(&dev) != RFAL_ERR_NONE || dev == NULL) {
        printf("no active device\n");
        return;
    }
    const char *tech = "unknown";
    switch (dev->type) {
    case RFAL_NFC_LISTEN_TYPE_NFCA: tech = "NFC-A"; break;
    case RFAL_NFC_LISTEN_TYPE_NFCB: tech = "NFC-B"; break;
    case RFAL_NFC_LISTEN_TYPE_NFCV: tech = "NFC-V"; break;
    case RFAL_NFC_LISTEN_TYPE_NFCF: tech = "NFC-F"; break;
    default: break;
    }
    printf("type=%s(%d) interface=%s nfcid=", tech, dev->type,
           dev->rfInterface == RFAL_NFC_INTERFACE_ISODEP ? "ISODEP" : "RF");
    for (int i = 0; i < dev->nfcidLen; i++)
        printf("%02X", dev->nfcid[i]);
    printf("\n");
    /* this polled RFAL stores LISTEN_TYPE_* even for polled devices */
    if (dev->type == RFAL_NFC_LISTEN_TYPE_NFCA) {
        printf("ATQA=%02X%02X SAK=%02X\n",
               dev->dev.nfca.sensRes.platformInfo,
               dev->dev.nfca.sensRes.anticollisionInfo,
               dev->dev.nfca.selRes.sak);
    }
    if (dev->type == RFAL_NFC_LISTEN_TYPE_NFCB) {
        printf("ATQBlen=%u FSCI=%u ISO-DEP=%s\n",
               dev->dev.nfcb.sensbResLen,
               (unsigned)rfalNfcbGetFSCI(&dev->dev.nfcb.sensbRes),
               rfalNfcbIsIsoDepSupported(&dev->dev.nfcb) ? "yes" : "no");
    }
    if (dev->rfInterface == RFAL_NFC_INTERFACE_ISODEP) {
        printf("ATSLen=%u FWI=%u FSxI=%u FSx=%u HB=",
               dev->proto.isoDep.activation.A.Listener.ATSLen,
               dev->proto.isoDep.info.FWI,
               dev->proto.isoDep.info.FSxI,
               dev->proto.isoDep.info.FSx);
        uint8_t hbLen = dev->proto.isoDep.activation.A.Listener.ATSLen;
        for (uint8_t i = 0; i < hbLen && i < 15; i++)
            printf("%02X", dev->proto.isoDep.activation.A.Listener.ATS.HB[i]);
        printf("\n");
    }
    fflush(stdout);
}

static int send_apdu(const uint8_t *tx, uint16_t tx_len)
{
    uint8_t *rx = NULL;
    uint16_t *rx_len = NULL;
    print_hex(">>", tx, tx_len);
    ReturnCode err = rfalNfcDataExchangeStart((uint8_t *)tx, tx_len, &rx, &rx_len, 0);
    if (err != RFAL_ERR_NONE) {
        printf("start failed: %d\n", err);
        return -1;
    }
    while ((err = rfalNfcDataExchangeGetStatus()) == RFAL_ERR_BUSY)
        rfalNfcWorker();
    if (err != RFAL_ERR_NONE) {
        printf("exchange error: %d\n", err);
        return -1;
    }
    print_hex("<<", rx, *rx_len);
    return 0;
}

/* send APDU, return response length or -1; response copied to resp (with SW12) */
static int send_apdu_quiet(const uint8_t *tx, uint16_t tx_len, uint8_t *resp, uint16_t *resp_len)
{
    uint8_t *rx = NULL;
    uint16_t *rx_len = NULL;
    ReturnCode err = rfalNfcDataExchangeStart((uint8_t *)tx, tx_len, &rx, &rx_len, 0);
    if (err != RFAL_ERR_NONE)
        return -1;
    while ((err = rfalNfcDataExchangeGetStatus()) == RFAL_ERR_BUSY)
        rfalNfcWorker();
    if (err != RFAL_ERR_NONE) {
        printf("apdu %02X%02X exchange error: %d\n", tx[0], tx[1], err);
        return -1;
    }
    if (resp && resp_len) {
        memcpy(resp, rx, *rx_len);
        *resp_len = *rx_len;
    }
    return (int)*rx_len;
}

static int sw_ok(const uint8_t *resp, int len)
{
    return len >= 2 && resp[len - 2] == 0x90 && resp[len - 1] == 0x00;
}

static int esl_refresh(uint8_t p1);

/* tag answers refresh cmds slowly (screen power-up), stretch ISO-DEP FWT to ~5 s */
static void stretch_fwt(void)
{
    rfalNfcDevice *dev = NULL;
    if (rfalNfcGetActiveDevice(&dev) == RFAL_ERR_NONE && dev &&
        dev->rfInterface == RFAL_NFC_INTERFACE_ISODEP) {
        dev->proto.isoDep.info.FWT = rfalIsoDepFWI2FWT(14);
        dev->proto.isoDep.info.dFWT = rfalIsoDepFWI2FWT(14);
    }
}

/* load PNG/JPG/BMP/PPM etc., must be exactly ESL_W x ESL_H; returns 0 ok */
static int load_image(const char *path, uint8_t *rgb /* w*h*3 */)
{
    int w, h, ch;
    uint8_t *img = stbi_load(path, &w, &h, &ch, 3);
    if (!img) {
        printf("cannot load image: %s\n", stbi_failure_reason());
        return -1;
    }
    if (w != ESL_W || h != ESL_H) {
        printf("image must be %dx%d (got %dx%d)\n", ESL_W, ESL_H, w, h);
        stbi_image_free(img);
        return -1;
    }
    memcpy(rgb, img, (size_t)w * h * 3);
    stbi_image_free(img);
    return 0;
}

static int nearest_color(uint8_t r, uint8_t g, uint8_t b)
{
    int best = 0;
    int bestd = 1 << 30;
    for (unsigned i = 0; i < sizeof(palette) / sizeof(palette[0]); i++) {
        int dr = r - palette[i].r, dg = g - palette[i].g, db = b - palette[i].b;
        int d = dr * dr + dg * dg + db * db;
        if (d < bestd) { bestd = d; best = i; }
    }
    return best;
}

/* separable box blur, radius from NFC_BLUR (default 0 = off).
   Needed for camera-captured images: pixel-grid/moire noise otherwise gets
   quantized as content and washes regions out to gray-white. */
static void box_blur(uint8_t *rgb, int radius)
{
    if (radius <= 0) return;
    static uint8_t tmp[ESL_W * ESL_H * 3];
    int w = 2 * radius + 1;
    for (int y = 0; y < ESL_H; y++) {           /* horizontal */
        for (int x = 0; x < ESL_W; x++) {
            int acc[3] = {0, 0, 0};
            for (int k = -radius; k <= radius; k++) {
                int nx = x + k < 0 ? 0 : x + k >= ESL_W ? ESL_W - 1 : x + k;
                const uint8_t *px = &rgb[(y * ESL_W + nx) * 3];
                acc[0] += px[0]; acc[1] += px[1]; acc[2] += px[2];
            }
            uint8_t *o = &tmp[(y * ESL_W + x) * 3];
            o[0] = acc[0] / w; o[1] = acc[1] / w; o[2] = acc[2] / w;
        }
    }
    for (int y = 0; y < ESL_H; y++) {           /* vertical */
        for (int x = 0; x < ESL_W; x++) {
            int acc[3] = {0, 0, 0};
            for (int k = -radius; k <= radius; k++) {
                int ny = y + k < 0 ? 0 : y + k >= ESL_H ? ESL_H - 1 : y + k;
                const uint8_t *px = &tmp[(ny * ESL_W + x) * 3];
                acc[0] += px[0]; acc[1] += px[1]; acc[2] += px[2];
            }
            uint8_t *o = &rgb[(y * ESL_W + x) * 3];
            o[0] = acc[0] / w; o[1] = acc[1] / w; o[2] = acc[2] / w;
        }
    }
}

/* NFC_DITHER=none|fs selects vendor SDK modes; default atkinson (iOS default).
   fs: Android BMPConverterUtil, weight 2/16 to (x+1,y) (x-1,y+1) (x,y+1) (x+1,y+1)
   atkinson: iOS ImageConvertUtils, weight 1/8 to 6 neighbors; smoother on
   mid-tones (fs shows vertical banding there) */
static void quantize_image(uint8_t *rgb /* modified */, uint8_t *idx_out)
{
    const char *alg = getenv("NFC_DITHER");
    int fs = alg && !strcmp(alg, "fs");
    int dither = !(alg && !strcmp(alg, "none"));
    int atkinson = !fs;
    static const int8_t fs_nb[4][2] = { {1, 0}, {-1, 1}, {0, 1}, {1, 1} };
    static const int8_t at_nb[6][2] = { {1, 0}, {2, 0}, {-1, 1}, {0, 1}, {1, 1}, {0, 2} };
    const int8_t (*nb)[2] = atkinson ? at_nb : fs_nb;
    int nnb = atkinson ? 6 : 4;
    for (int y = 0; y < ESL_H; y++) {
        for (int x = 0; x < ESL_W; x++) {
            uint8_t *px = &rgb[(y * ESL_W + x) * 3];
            int best = nearest_color(px[0], px[1], px[2]);
            idx_out[y * ESL_W + x] = (uint8_t)best;
            if (!dither) continue;
            for (int c = 0; c < 3; c++) {
                int err = px[c] - (c == 0 ? palette[best].r : c == 1 ? palette[best].g : palette[best].b);
                int share = atkinson ? (err + (err >= 0 ? 4 : -4)) / 8
                                     : (err * 2) >> 4;
                for (int k = 0; k < nnb; k++) {
                    int nx = x + nb[k][0], ny = y + nb[k][1];
                    if (nx < 0 || nx >= ESL_W || ny >= ESL_H) continue;
                    if (!atkinson && k == 1 && x - 1 <= 0) continue;   /* SDK boundary quirk */
                    uint8_t *np = &rgb[(ny * ESL_W + nx) * 3 + c];
                    int v = *np + share;
                    *np = (uint8_t)(v < 0 ? 0 : v > 255 ? 255 : v);
                }
            }
        }
    }
}

/* build 4bpp stream (2 px/byte, high nibble first); tag TLV: hf=1 vf=1 (flip 0x87) */
static void build_4bpp(const uint8_t *idx, uint8_t *out /* w*h/2 */)
{
    int nibble_hi = 1;
    uint8_t byte = 0;
    uint8_t *p = out;
    for (int orgY = 0; orgY < ESL_H; orgY++) {
        for (int orgX = 0; orgX < ESL_W; orgX++) {
            int x = orgX;                 /* flipHorizontal=1 -> no mirror */
            int y = ESL_H - 1 - orgY;     /* flipVertical=1 -> vertical flip */
            uint8_t code = palette[idx[y * ESL_W + x]].code & 0x0F;
            if (nibble_hi) {
                byte = code << 4;
                nibble_hi = 0;
            } else {
                *p++ = byte | code;
                nibble_hi = 1;
            }
        }
    }
}

/* default factory pattern: 6 vertical color bars + black frame (checks panel edges) */
static void gen_test_pattern(uint8_t *rgb)
{
    const int bar = ESL_W / 6;
    for (int y = 0; y < ESL_H; y++) {
        for (int x = 0; x < ESL_W; x++) {
            const uint8_t *c;
            if (x < 4 || y < 4 || x >= ESL_W - 4 || y >= ESL_H - 4)
                c = &palette[0].r;                    /* black frame */
            else
                c = &palette[(x / bar) % 6].r;        /* palette order stripes */
            uint8_t *px = &rgb[(y * ESL_W + x) * 3];
            px[0] = c[0]; px[1] = c[1]; px[2] = c[2];
        }
    }
}

/* select NDEF application on the ESL tag, with retries */
static int select_ndef(uint8_t *resp, uint16_t *resp_len)
{
    uint8_t sel[] = { 0x00, 0xA4, 0x04, 0x00, 0x07, 0xD2, 0x76, 0x00, 0x00, 0x85, 0x01, 0x01 };
    int n = -1;
    for (int retry = 0; retry < 10 && (n < 0 || !sw_ok(resp, n)); retry++) {
        if (retry) rfal_platform_delay(50);
        n = send_apdu_quiet(sel, sizeof(sel), resp, resp_len);
    }
    return (n >= 0 && sw_ok(resp, n)) ? 0 : -1;
}

static int esl_write_image(const char *path)
{
    static uint8_t rgb[ESL_W * ESL_H * 3];
    static uint8_t stream[ESL_W * ESL_H / 2];
    uint8_t resp[300];
    uint16_t resp_len;

    if (path) {
        if (load_image(path, rgb) != 0)
            return -1;
    } else {
        printf("no image given, using built-in test pattern (6-color bars)\n");
        gen_test_pattern(rgb);
    }
    const char *bl = getenv("NFC_BLUR");
    if (bl) box_blur(rgb, atoi(bl));
    static uint8_t qidx[ESL_W * ESL_H];
    quantize_image(rgb, qidx);
    build_4bpp(qidx, stream);

    if (select_ndef(resp, &resp_len) != 0) {
        printf("select NDEF failed\n");
        return -1;
    }

    stretch_fwt();

    /* optional: let the tag reservoir recharge before the NVM write burst
       (helps when writing right after a refresh) */
    const char *pc = getenv("NFC_PRECHARGE_MS");
    if (pc) rfal_platform_delay((uint32_t)atoi(pc));

    /* 4 chunks -> image index 0..3, 250-byte packets */
    int chunk = (ESL_W * ESL_H / 2) / ESL_PLANES;   /* 7200 */
    const char *d = getenv("NFC_D2_DELAY_MS");
    uint32_t pace = d ? (uint32_t)atoi(d) : 0;
    double t_all = now_ms();
    for (int idx = 0; idx < ESL_PLANES; idx++) {
        const uint8_t *data = stream + idx * chunk;
        int left = chunk;
        int pkt = 0;
        double t0 = now_ms();
        while (left > 0) {
            int plen = left > 250 ? 250 : left;
            uint8_t apdu[5 + 250];
            apdu[0] = 0xF0; apdu[1] = 0xD2;
            apdu[2] = (uint8_t)idx;
            apdu[3] = (uint8_t)pkt;
            apdu[4] = (uint8_t)plen;
            memcpy(apdu + 5, data + pkt * 250, plen);
            /* transient errors: retry; persistent: re-activate (field stays
               on) and resume from the same packet */
            int ok = 0;
            for (int attempt = 0; attempt < 40 && !ok; attempt++) {
                double tp = now_ms();
                int n = send_apdu_quiet(apdu, 5 + plen, resp, &resp_len);
                double pdt = now_ms() - tp;
                if (pdt > 150) {
                    printf("slow pkt: idx=%d pkt=%d t=%.0fms%s\n",
                           idx, pkt, pdt, (n >= 0 && sw_ok(resp, n)) ? " ok" : "");
                    /* tag energy-starved: back off the write pace */
                    if (pace > 0 && pace < 50) { pace *= 2; printf("pacing up to %ums\n", pace); }
                }
                ok = (n >= 0 && sw_ok(resp, n));
                if (!ok && attempt % 5 == 4) {
                    printf("re-activating tag (idx=%d pkt=%d)...\n", idx, pkt);
                    rfalNfcDeactivate(RFAL_NFC_DEACTIVATE_SLEEP);
                    rfal_platform_delay(50);
                    uint32_t dl = rfal_platform_timer_create(60000);
                    while (activate(10000) != 0) {
                        if (rfal_platform_timer_is_expired(dl)) {
                            printf("re-activation timeout\n");
                            return -1;
                        }
                    }
                    if (select_ndef(resp, &resp_len) != 0) {
                        printf("re-select NDEF failed\n");
                        return -1;
                    }
                    stretch_fwt();
                }
            }
            if (!ok) {
                printf("D2 failed idx=%d pkt=%d\n", idx, pkt);
                return -1;
            }
            left -= plen;
            pkt++;
            /* optional pacing: let the tag cap recharge between NVM writes;
               without it the tag may stall ~10s (2x FWT) mid-transfer */
            if (pace) rfal_platform_delay(pace);
        }
        double dt = now_ms() - t0;
        printf("image %d written (%d bytes, %d pkts, %.2fs, %.1f kB/s)\n",
               idx, chunk, pkt, dt / 1000.0, chunk / dt);
    }
    double dt_all = now_ms() - t_all;
    printf("transfer total: %d bytes in %.2fs = %.1f kB/s (%.0f kbps air payload)\n",
           ESL_W * ESL_H / 2, dt_all / 1000.0, (ESL_W * ESL_H / 2) / dt_all,
           (ESL_W * ESL_H / 2) * 8.0 / dt_all);

    const char *p1 = getenv("NFC_REFRESH_P1");
    return esl_refresh(p1 ? (uint8_t)strtoul(p1, NULL, 16) : 0x85);
}

/* redraw with immediate return, keep field on and poll busy until idle */
static int esl_refresh(uint8_t p1)
{
    uint8_t resp[300];
    uint16_t resp_len;

    stretch_fwt();
    /* Android SDK passive flow: P2 bit7=1 (immediate return), then poll DE.
       A bare 2-byte 9000 means wait-mode refresh already finished. */
    uint8_t d4[] = { 0xF0, 0xD4, p1, 0x80, 0x00 };
    int n = send_apdu_quiet(d4, sizeof(d4), resp, &resp_len);
    printf("D4 resp:");
    for (int i = 0; i < n; i++) printf(" %02X", resp[i]);
    printf(n < 0 ? " (no response, poll busy instead)\n" : "\n");
    /* Android polls DE even after a bare 9000 (it only means "started") */
    if (n > 0 && !(n == 2 && sw_ok(resp, n)) && !(n == 3 && resp[0] <= 0x01)) {
        printf("D4 rejected by tag\n");
        return -1;
    }

    printf("refreshing, polling busy status (keep tag on antenna)...\n");
    uint8_t de[] = { 0xF0, 0xDE, 0x00, 0x00, 0x01 };
    int errs = 0;
    uint32_t deadline = rfal_platform_timer_create(300000);
    for (int i = 0; !rfal_platform_timer_is_expired(deadline); i++) {
        n = send_apdu_quiet(de, sizeof(de), resp, &resp_len);
        if (n >= 3 && resp[0] == 0x00 && sw_ok(resp, n)) {
            printf("refresh done (%d polls)\n", i + 1);
            return 0;
        }
        if (n < 0) {
            if (++errs > 20) {
                printf("DE exchange keeps failing\n");
                return -1;
            }
            /* tolerate a few garbled frames (brown-out), then re-activate */
            if (errs <= 5) {
                rfal_platform_delay(300);
                continue;
            }
            printf("re-activating tag...\n");
            rfalNfcDeactivate(RFAL_NFC_DEACTIVATE_SLEEP);   /* keep the field on */
            rfal_platform_delay(50);
            while (activate(10000) != 0) {
                if (rfal_platform_timer_is_expired(deadline))
                    goto out;
            }
            errs = 0;
            stretch_fwt();
            continue;
        }
        errs = 0;
        rfal_platform_delay(500);
    }
out:
    printf("refresh timeout\n");
    return -1;
}

/* select the one NFC-A tag in the field (T2T commands need it selected) */
static int t2_select(void)
{
    rfalNfcaSensRes sens;
    rfalNfcaListenDevice dev;
    uint8_t devcnt = 0;
    ReturnCode err = RFAL_ERR_TIMEOUT;
    for (int i = 0; i < 50; i++) {
        err = rfalNfcaPollerTechnologyDetection(RFAL_COMPLIANCE_MODE_NFC, &sens);
        if (err != RFAL_ERR_NONE) {
            rfal_platform_delay(20);
            continue;
        }
        err = rfalNfcaPollerFullCollisionResolution(RFAL_COMPLIANCE_MODE_NFC, 1, &dev, &devcnt);
        if (err == RFAL_ERR_NONE && devcnt == 1)
            break;
        rfal_platform_delay(20);
    }
    if (err != RFAL_ERR_NONE || devcnt != 1) {
        printf("no NFC-A tag found\n");
        return -1;
    }
    printf("UID=");
    for (int i = 0; i < dev.nfcId1Len; i++)
        printf("%02X", dev.nfcId1[i]);
    printf(" SAK=%02X\n", dev.selRes.sak);
    return 0;
}

static int t2_write_page(uint8_t page, const uint8_t *data)
{
    ReturnCode err = RFAL_ERR_TIMEOUT;
    for (int i = 0; i < 3 && err != RFAL_ERR_NONE; i++) {
        err = rfalT2TPollerWrite(page, data);
        if (err != RFAL_ERR_NONE)
            rfal_platform_delay(10);
    }
    return err == RFAL_ERR_NONE ? 0 : -1;
}

static int t2_init(void)
{
    if (rfal_platform_init() != 0)
        return -1;
    if (rfalNfcInitialize() != RFAL_ERR_NONE)
        return -1;
    rfalNfcaPollerInitialize();
    const char *ant = getenv("NFC_ANT");
    if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1 (J30)\n"); }
    if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2 (J31)\n"); }
    rfalFieldOnAndStartGT();
    rfal_platform_delay(5);
    return 0;
}

/* one-record NDEF text message as a TLV (03 len D1 01 pl T ... FE) — nfcd's pad-pairing format */
static size_t build_ndef_text(uint8_t *out, const char *text)
{
    size_t tl = strlen(text);
    uint8_t payload[64];
    size_t pl = 0;
    payload[pl++] = 0x02;   /* UTF-8, 2-byte language code */
    payload[pl++] = 'e';
    payload[pl++] = 'n';
    memcpy(payload + pl, text, tl);
    pl += tl;
    out[0] = 0x03;                  /* NDEF TLV */
    out[1] = (uint8_t)(pl + 4);     /* record length */
    out[2] = 0xD1;                  /* MB|ME|SR, TNF well-known */
    out[3] = 0x01;
    out[4] = (uint8_t)pl;
    out[5] = 'T';
    memcpy(out + 6, payload, pl);
    size_t n = 6 + pl;
    out[n++] = 0xFE;                /* terminator */
    return n;
}

static int t5_select(rfalNfcvInventoryRes *inv)
{
    ReturnCode err = RFAL_ERR_TIMEOUT;
    for (int i = 0; i < 50 && err != RFAL_ERR_NONE; i++) {
        err = rfalNfcvPollerInventory(RFAL_NFCV_NUM_SLOTS_1, 0, NULL, inv, NULL);
        if (err != RFAL_ERR_NONE)
            rfal_platform_delay(20);
    }
    if (err != RFAL_ERR_NONE) {
        printf("no NFC-V tag found\n");
        return -1;
    }
    printf("UID=");
    for (int i = RFAL_NFCV_UID_LEN - 1; i >= 0; i--)
        printf("%02X", inv->UID[i]);
    printf("\n");
    return 0;
}

static int t5_write(const uint8_t *uid, uint8_t blk, const uint8_t *data, uint8_t n)
{
    ReturnCode err = RFAL_ERR_TIMEOUT;
    for (int i = 0; i < 3 && err != RFAL_ERR_NONE; i++) {
        err = rfalNfcvPollerWriteSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, uid, blk, data, n);
        if (err != RFAL_ERR_NONE)
            rfal_platform_delay(10);
    }
    return err == RFAL_ERR_NONE ? 0 : -1;
}

/* block count from Get System Info; 0 if unreadable */
static unsigned t5_blocks(const rfalNfcvInventoryRes *inv)
{
    uint8_t buf[64];
    uint16_t len = 0;
    if (rfalNfcvPollerGetSystemInformation(RFAL_NFCV_REQ_FLAG_DEFAULT, inv->UID,
                                           buf, sizeof(buf), &len) != RFAL_ERR_NONE || len < 4)
        return 0;
    /* response: flags, DSFID, AFI, (blocks-1)[, block size]; blocks-1 may be 2-byte big-endian */
    unsigned blocks = (buf[3] == 0 && len >= 5) ? (unsigned)buf[4] + 1 : (unsigned)buf[3] + 1;
    return blocks <= 1024 ? blocks : 0;
}

int main(int argc, char **argv)
{
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 2) {
        printf("usage: %s poll [timeout_ms] | apdu HEX [HEX ...] | esl-info | write [image] | refresh [P1hex] | busywait [secs] | field | carrier | regs | nfcv read/write | t2 ... | t5 read/ndef <text>\n", argv[0]);
        return 2;
    }

    if (strcmp(argv[1], "t2") == 0) {
        /* Type 2 tag (NTAG) page access:
           t2 read <page> | t2 write <page> <hex8> | t2 ndef <text>
           ndef writes a one-record NDEF text message at page 4 (nfcd's pad-pairing format) */
        if (argc < 4) {
            printf("usage: %s t2 read <page> | t2 write <page> <hex8> | t2 ndef <text>\n", argv[0]);
            return 2;
        }
        if (t2_init() != 0)
            return 1;
        if (t2_select() != 0)
            return 1;

        if (strcmp(argv[2], "ndef") == 0) {
            const char *text = argv[3];
            if (strlen(text) == 0 || strlen(text) > 48) {
                printf("text too long (max 48)\n");
                return 2;
            }
            uint8_t mem[64];
            size_t n = build_ndef_text(mem, text);
            while (n % 4)
                mem[n++] = 0x00;
            for (size_t off = 0; off < n; off += 4) {
                uint8_t page = 4 + (uint8_t)(off / 4);
                if (t2_write_page(page, mem + off) != 0) {
                    printf("write page %u failed\n", page);
                    return 1;
                }
            }
            uint8_t chk[16];
            uint16_t clen = 0;
            if (rfalT2TPollerRead(4, chk, sizeof(chk), &clen) != RFAL_ERR_NONE) {
                printf("readback failed\n");
                return 1;
            }
            printf("wrote %u bytes at page 4; first 16 read back: ", (unsigned)n);
            print_hex("", chk, clen);
            return 0;
        }

        unsigned page = (unsigned)atoi(argv[3]);
        if (page > 255) {
            printf("bad page number\n");
            return 2;
        }
        if (strcmp(argv[2], "read") == 0) {
            uint8_t buf[16];
            uint16_t len = 0;
            if (rfalT2TPollerRead((uint8_t)page, buf, sizeof(buf), &len) != RFAL_ERR_NONE) {
                printf("read page %u failed\n", page);
                return 1;
            }
            print_hex("read:", buf, len);
            return 0;
        }
        if (strcmp(argv[2], "write") == 0) {
            if (argc < 5) {
                printf("usage: %s t2 write <page> <hex8>\n", argv[0]);
                return 2;
            }
            uint8_t data[4];
            if (hex2bin(argv[4], data) != 4) {
                printf("hex8 must be exactly 4 bytes (8 hex chars)\n");
                return 2;
            }
            if (t2_write_page((uint8_t)page, data) != 0) {
                printf("write page %u failed\n", page);
                return 1;
            }
            uint8_t chk[16];
            uint16_t clen = 0;
            if (rfalT2TPollerRead((uint8_t)(page & ~3u), chk, sizeof(chk), &clen) != RFAL_ERR_NONE ||
                clen < 4 || memcmp(chk + (page % 4) * 4, data, 4) != 0) {
                printf("verify failed\n");
                return 1;
            }
            printf("page %u written and verified\n", page);
            return 0;
        }
        printf("usage: %s t2 read <page> | t2 write <page> <hex8> | t2 ndef <text>\n", argv[0]);

        return 2;
    }

    if (strcmp(argv[1], "carrier") == 0) {
        /* unmodulated 13.56 MHz CW carrier, for RSE/EMC measurements */
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2\n"); }
        rfalSetMode(RFAL_MODE_POLL_NFCA, RFAL_BR_106, RFAL_BR_106);
        rfalFieldOnAndStartGT();
        printf("carrier on (unmodulated 13.56 MHz, Ctrl-C to stop)\n");
        for (;;)
            rfal_platform_delay(1000);
    }

    if (strcmp(argv[1], "field") == 0) {
        /* field strength probe: measure amplitude/IQ once per second */
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2\n"); }
        rfalSetMode(RFAL_MODE_POLL_NFCA, RFAL_BR_106, RFAL_BR_106);
        rfalFieldOnAndStartGT();
        printf("field on; amp I Q (move tag to find best coupling, Ctrl-C to stop)\n");
        for (;;) {
            uint8_t amp = 0;
            int8_t ii = 0, qq = 0;
            rfalChipMeasureAmplitude(&amp);
            rfalChipMeasureIQ(&ii, &qq);
            printf("amp=%3d I=%4d Q=%4d\n", amp, ii, qq);
            rfal_platform_delay(1000);
        }
    }

    if (strcmp(argv[1], "regs") == 0) {
        /* dump key registers after init + field on (TX power debugging) */
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1 (J30)\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2 (J31)\n"); }
        rfalSetMode(RFAL_MODE_POLL_NFCA, RFAL_BR_106, RFAL_BR_106);
        rfalFieldOnAndStartGT();
        printf("reg dump (field on):\n");
        for (uint8_t a = 0x00; a <= 0x11; a++) {
            uint8_t v = 0;
            if (st25r200ReadRegister(a, &v) == RFAL_ERR_NONE)
                printf("  [0x%02X] = 0x%02X\n", a, v);
        }
        return 0;
    }

    if (strcmp(argv[1], "vregs") == 0) {
        /* dump registers after NFC-V init (driver-parity debugging) */
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        rfalNfcvPollerInitialize();
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1 (J30)\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2 (J31)\n"); }
        rfalFieldOnAndStartGT();
        rfal_platform_delay(5);
        for (uint8_t a = 0x00; a <= 0x3f; a++) {
            uint8_t v = 0;
            if (st25r200ReadRegister(a, &v) == RFAL_ERR_NONE)
                printf("  [%02X] = %02X\n", a, v);
        }
        return 0;
    }

    if (strcmp(argv[1], "nfcv") == 0) {
        /* ISO15693 block read/write test:
           nfcv read <blk> | nfcv write <blk> [hexdata]
           write without hexdata rewrites the same data (non-destructive) */
        if (argc < 4 || (strcmp(argv[2], "read") != 0 && strcmp(argv[2], "write") != 0)) {
            printf("usage: %s nfcv read <blk> | nfcv write <blk> [hexdata]\n", argv[0]);
            return 2;
        }
        unsigned blk = (unsigned)atoi(argv[3]);
        if (blk > 255) {
            printf("bad block number\n");
            return 2;
        }
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        rfalNfcvPollerInitialize();
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1 (J30)\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2 (J31)\n"); }
        rfalFieldOnAndStartGT();
        rfal_platform_delay(5);

        rfalNfcvInventoryRes inv;
        ReturnCode verr = RFAL_ERR_TIMEOUT;
        for (int i = 0; i < 50 && verr != RFAL_ERR_NONE; i++) {
            verr = rfalNfcvPollerInventory(RFAL_NFCV_NUM_SLOTS_1, 0, NULL, &inv, NULL);
            if (verr != RFAL_ERR_NONE)
                rfal_platform_delay(20);
        }
        if (verr != RFAL_ERR_NONE) {
            printf("no NFC-V tag found\n");
            return 1;
        }
        printf("UID=");
        for (int i = RFAL_NFCV_UID_LEN - 1; i >= 0; i--)
            printf("%02X", inv.UID[i]);
        printf("\n");

        uint8_t buf[64];
        uint16_t len = 0;
        verr = rfalNfcvPollerReadSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                             (uint8_t)blk, buf, sizeof(buf), &len);
        if (verr != RFAL_ERR_NONE) {
            printf("read blk %u failed: %d\n", blk, verr);
            return 1;
        }
        if (len < 1) {
            printf("empty read response\n");
            return 1;
        }
        uint8_t blk_len = (uint8_t)(len - 1);   /* strip response flags byte */
        print_hex("read:", buf + 1, blk_len);

        if (strcmp(argv[2], "read") == 0)
            return 0;

        uint8_t wdata[32];
        int wlen;
        if (argc > 4) {
            wlen = hex2bin(argv[4], wdata);
            if (wlen <= 0) {
                printf("bad hexdata\n");
                return 2;
            }
            if (wlen < blk_len) {
                memset(wdata + wlen, 0, blk_len - wlen);
                printf("note: padded to %u bytes with 00\n", blk_len);
                wlen = blk_len;
            } else if (wlen > blk_len) {
                printf("note: truncated to %u bytes\n", blk_len);
                wlen = blk_len;
            }
        } else {
            wlen = blk_len;
            memcpy(wdata, buf + 1, blk_len);
        }
        verr = rfalNfcvPollerWriteSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                              (uint8_t)blk, wdata, (uint8_t)wlen);
        if (verr != RFAL_ERR_NONE) {
            printf("write blk %u failed: %d\n", blk, verr);
            return 1;
        }
        rfal_platform_delay(5);
        memset(buf, 0, sizeof(buf));
        len = 0;
        verr = rfalNfcvPollerReadSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                             (uint8_t)blk, buf, sizeof(buf), &len);
        if (verr != RFAL_ERR_NONE) {
            printf("read-back failed: %d\n", verr);
            return 1;
        }
        print_hex("readback:", buf, len);
        if (len == (uint16_t)(wlen + 1) && memcmp(buf + 1, wdata, wlen) == 0) {
            printf("WRITE PASS\n");
            return 0;
        }
        printf("WRITE FAIL (mismatch)\n");
        return 1;
    }

    if (strcmp(argv[1], "t5") == 0) {
        /* Type 5 (ISO 15693) tag NDEF access:
           t5 read — dump the container and TLV area, decode the text record
           t5 ndef <text> — keeps a valid CC (writes one on a blank tag), then
           the one-record NDEF text TLV from byte 4, nfcd's pad-pairing format */
        int writing = argc > 3 && strcmp(argv[2], "ndef") == 0;
        if (!writing && (argc != 3 || strcmp(argv[2], "read") != 0)) {
            printf("usage: %s t5 read | t5 ndef <text>\n", argv[0]);
            return 2;
        }
        const char *text = writing ? argv[3] : "";
        if (writing && (strlen(text) == 0 || strlen(text) > 48)) {
            printf("text too long (max 48)\n");
            return 2;
        }
        if (rfal_platform_init() != 0)
            return 1;
        if (rfalNfcInitialize() != RFAL_ERR_NONE)
            return 1;
        rfalNfcvPollerInitialize();
        const char *ant = getenv("NFC_ANT");
        if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1 (J30)\n"); }
        if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2 (J31)\n"); }
        rfalFieldOnAndStartGT();
        rfal_platform_delay(5);

        rfalNfcvInventoryRes inv;
        if (t5_select(&inv) != 0)
            return 1;

        uint8_t buf[64];
        uint16_t len = 0;
        if (rfalNfcvPollerReadSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                          0, buf, sizeof(buf), &len) != RFAL_ERR_NONE || len < 1) {
            printf("read blk 0 failed\n");
            return 1;
        }
        uint8_t blk_len = (uint8_t)(len - 1);   /* strip response flags byte */

        if (!writing) {
            /* read through the TLV terminator (16-block cap), dump, decode the record */
            uint8_t img[128];
            unsigned n = 0;
            for (unsigned b = 0; b < 16 && n < sizeof(img); b++) {
                uint16_t rlen = 0;
                if (rfalNfcvPollerReadSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                                  (uint8_t)b, buf, sizeof(buf), &rlen) != RFAL_ERR_NONE ||
                    rlen < 1)
                    break;
                unsigned bl = rlen - 1;
                if (n + bl > sizeof(img))
                    bl = sizeof(img) - n;
                memcpy(img + n, buf + 1, bl);
                n += bl;
                unsigned cc = (img[0] == 0xE2) ? 8 : 4;
                for (unsigned i = cc; i < n; i++)
                    if (img[i] == 0xFE)
                        goto dumped;
            }
        dumped:
            print_hex("memory:", img, n);
            unsigned cc = (n > 0 && img[0] == 0xE2) ? 8 : 4;
            /* one NDEF text record, the layout `t5 ndef` writes (and phones copy) */
            if (n >= cc + 7 && img[cc] == 0x03 && img[cc + 2] == 0xD1 &&
                img[cc + 3] == 0x01 && img[cc + 5] == 'T' && cc + 6 + img[cc + 4] <= n) {
                unsigned pl = img[cc + 4], lang = img[cc + 6] & 0x3F;
                if (pl > 1 + lang) {
                    unsigned tl = pl - 1 - lang;
                    char txt[64];
                    if (tl > sizeof(txt) - 1)
                        tl = sizeof(txt) - 1;
                    memcpy(txt, img + cc + 7 + lang, tl);
                    txt[tl] = 0;
                    printf("text: %s\n", txt);
                }
            } else {
                printf("no NDEF text record\n");
            }
            return 0;
        }

        /* CC: keep a valid one; MLEN per NFC Forum T5T = tag area in 8-byte units */
        uint8_t img[128];
        uint8_t cc_len;
        if (buf[1] == 0xE1 || buf[1] == 0xE2) {
            cc_len = (buf[1] == 0xE1) ? 4 : 8;
            memcpy(img, buf + 1, cc_len);
            printf("existing CC kept: ");
            print_hex("", img, cc_len);
        } else {
            unsigned blocks = t5_blocks(&inv);
            if (blocks == 0)
                blocks = 32;
            unsigned mem = blocks * blk_len;
            if (mem <= 2044) {
                cc_len = 4;
                img[0] = 0xE1;
                img[1] = 0x40;
                img[2] = 0x00;
                img[3] = (uint8_t)((mem - 4) / 8);
            } else {
                cc_len = 8;
                unsigned mlen = (mem - 8) / 8;
                img[0] = 0xE2;
                img[1] = 0x40;
                memset(img + 2, 0, 4);
                img[6] = (uint8_t)(mlen >> 8);
                img[7] = (uint8_t)mlen;
            }
            printf("blank tag: CC written for %u blocks x %u bytes\n", blocks, blk_len);
        }

        size_t n = cc_len + build_ndef_text(img + cc_len, text);
        while (n % blk_len)
            img[n++] = 0x00;

        unsigned nblk = (unsigned)(n / blk_len);
        for (unsigned b = 0; b < nblk; b++) {
            if (t5_write(inv.UID, (uint8_t)b, img + b * blk_len, blk_len) != 0) {
                printf("write blk %u failed\n", b);
                return 1;
            }
        }
        for (unsigned b = 0; b < nblk; b++) {
            len = 0;
            if (rfalNfcvPollerReadSingleBlock(RFAL_NFCV_REQ_FLAG_DEFAULT, inv.UID,
                                              (uint8_t)b, buf, sizeof(buf), &len) != RFAL_ERR_NONE ||
                (uint8_t)(len - 1) != blk_len || memcmp(buf + 1, img + b * blk_len, blk_len) != 0) {
                printf("verify blk %u failed\n", b);
                return 1;
            }
        }
        printf("wrote %u bytes in %u blocks: ", (unsigned)n, nblk);
        print_hex("", img, n > 24 ? 24 : n);
        printf("WRITE PASS\n");
        return 0;
    }

    if (rfal_platform_init() != 0)
        return 1;
    ReturnCode err = rfalNfcInitialize();
    if (err != RFAL_ERR_NONE) {
        printf("rfalNfcInitialize failed: %d\n", err);
        return 1;
    }
    printf("RFAL init ok\n");

    /* NFC_ANT=1|2: single-ended drive on RFO1/RFO2 (GENERAL: single + rfo2) */
    const char *ant = getenv("NFC_ANT");
    if (ant && ant[0] == '1') { rfal_platform_write_reg(0x01, 0x0C | 0x20); printf("antenna: single RFO1\n"); }
    if (ant && ant[0] == '2') { rfal_platform_write_reg(0x01, 0x0C | 0x20 | 0x10); printf("antenna: single RFO2\n"); }

    uint32_t timeout = (argc > 2 && strcmp(argv[1], "poll") == 0)
                       ? (uint32_t)atoi(argv[2]) : 10000;
    printf("waiting for tag (%u ms)...\n", timeout);
    if (activate(timeout) != 0) {
        printf("no tag found\n");
        return 1;
    }
    print_device();

    int rc = 0;
    if (strcmp(argv[1], "poll") == 0) {
        /* done */
    } else if (strcmp(argv[1], "write") == 0) {
        /* no image arg -> built-in 6-color bar test pattern */
        rc = esl_write_image(argc > 2 ? argv[2] : NULL) == 0 ? 0 : 1;
    } else if (strcmp(argv[1], "refresh") == 0) {
        unsigned p1 = 0x81;
        if (argc > 2)
            sscanf(argv[2], "%x", &p1);
        rc = esl_refresh((uint8_t)p1) == 0 ? 0 : 1;
    } else if (strcmp(argv[1], "busywait") == 0) {
        /* hold the field and poll busy status until idle (resume an
           interrupted refresh without re-triggering D4) */
        unsigned secs = 180;
        if (argc > 2)
            secs = (unsigned)atoi(argv[2]);
        uint8_t de[] = { 0xF0, 0xDE, 0x00, 0x00, 0x01 };
        uint8_t resp[300];
        uint16_t resp_len;
        stretch_fwt();
        uint32_t deadline = rfal_platform_timer_create(secs * 1000);
        int last = -1;
        while (!rfal_platform_timer_is_expired(deadline)) {
            int n = send_apdu_quiet(de, sizeof(de), resp, &resp_len);
            int busy = (n >= 3 && resp[0] == 0x01) ? 1 : 0;
            if (n < 0) {
                printf("DE exchange failed, re-activating...\n");
                rfalNfcDeactivate(RFAL_NFC_DEACTIVATE_SLEEP);   /* keep the field on */
                rfal_platform_delay(50);
                while (activate(10000) != 0) {
                    if (rfal_platform_timer_is_expired(deadline))
                        goto bwout;
                }
                stretch_fwt();
                continue;
            }
            if (busy != last) {
                printf("busy=%d\n", busy);
                last = busy;
            }
            if (!busy) {
                printf("refresh idle/done\n");
                rc = 0;
                break;
            }
            rfal_platform_delay(300);
        }
bwout:
        if (last == 1)
            { printf("still busy after %us\n", secs); rc = 1; }
    } else if (strcmp(argv[1], "esl-info") == 0) {
        static const char *seq[] = {
            "00A4040007D2760000850101",   /* select NDEF app */
            "F0D801FE050000000000",       /* read OTP/UID */
            "00D1000000",                 /* get config info (TLV) */
            "F0D8000005000000000E",       /* read config area */
        };
        for (size_t i = 0; i < sizeof(seq) / sizeof(seq[0]); i++) {
            uint8_t buf[300];
            int n = hex2bin(seq[i], buf);
            if (n < 0 || send_apdu(buf, (uint16_t)n) != 0) { rc = 1; break; }
        }
    } else if (strcmp(argv[1], "apdu") == 0) {
        stretch_fwt();
        for (int i = 2; i < argc; i++) {
            uint8_t buf[300];
            int n = hex2bin(argv[i], buf);
            if (n < 0) { printf("bad hex: %s\n", argv[i]); rc = 2; break; }
            if (send_apdu(buf, (uint16_t)n) != 0) { rc = 1; break; }
        }
    } else {
        printf("unknown mode\n");
        rc = 2;
    }

    rfalNfcDeactivate(RFAL_NFC_DEACTIVATE_IDLE);
    return rc;
}
