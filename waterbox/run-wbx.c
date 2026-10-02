/* run-wbx.c - runs core.wbx through the miniBox host the way the engine does,
 * taking run-native's options and printing run-native's lines, so the two
 * flavors diff directly.
 *
 * usage: run-wbx <core.wbx> <app.ipa> [run-native's options]
 *                [--rerecord] [--session] [--state-out PATH]
 *
 * The app is mounted as the engine mounts a game: under "rom", under its own
 * name ("/<name>.ipa"), with "rom.name" saying which; the settings are a flat
 * JSON object mounted as "settings".
 *
 * --rerecord  saves and loads the WHOLE machine before every frame; the
 *             output must not change.
 * --session   saves the machine half way, tears the host down, builds a new
 *             one from the same core and files, loads the state into it and
 *             finishes there: the reopened-project case, where anything of
 *             the machine kept outside the sandbox shows.
 * --savedata ZIP     start from this save data: mounted as a project mounts
 *                    it, named in the "savedata" slot of a "slots" map
 * --export-save ZIP  at the end, write what the app saved (stored, not
 *                    deflated), and print run-native's "save" lines
 */
#include "minibox.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { const uint8_t *p; size_t len, pos; } reader;
static intptr_t buf_read(uintptr_t ud, uint8_t *d, uintptr_t n)
{
	reader *r = (reader *)ud;
	size_t avail = r->len - r->pos;
	if (n > avail) n = avail;
	memcpy(d, r->p + r->pos, n);
	r->pos += n;
	return (intptr_t)n;
}
typedef struct { uint8_t *b; size_t len, cap, pos; } membuf;
static int32_t mem_write(uintptr_t ud, const uint8_t *d, uintptr_t n)
{
	membuf *m = (membuf *)ud;
	if (m->len + n > m->cap) { m->cap = (m->len + n) * 2 + 64; m->b = realloc(m->b, m->cap); }
	memcpy(m->b + m->len, d, n);
	m->len += n;
	return 0;
}
static intptr_t mem_read(uintptr_t ud, uint8_t *d, uintptr_t n)
{
	membuf *m = (membuf *)ud;
	uintptr_t avail = m->len - m->pos;
	if (n > avail) n = avail;
	memcpy(d, m->b + m->pos, n);
	m->pos += n;
	return (intptr_t)n;
}

static uint8_t *slurp(const char *path, size_t *len)
{
	FILE *f = fopen(path, "rb");
	if (!f) { perror(path); exit(1); }
	fseek(f, 0, SEEK_END);
	long n = ftell(f);
	fseek(f, 0, SEEK_SET);
	uint8_t *b = malloc(n > 0 ? (size_t)n : 1);
	if (fread(b, 1, (size_t)n, f) != (size_t)n) { perror(path); exit(1); }
	fclose(f);
	*len = (size_t)n;
	return b;
}

typedef int32_t (MB_GUEST_ABI *i32fn)(void);
typedef uintptr_t (MB_GUEST_ABI *ptrfn_i32)(int32_t);
typedef int64_t (MB_GUEST_ABI *i64fn_i32)(int32_t);
typedef uint64_t (MB_GUEST_ABI *u64fn)(void);
typedef uintptr_t (MB_GUEST_ABI *ptrfn)(void);
typedef void (MB_GUEST_ABI *framefn)(uint64_t);
typedef void (MB_GUEST_ABI *setfn)(int32_t, int32_t);

static mb_host *g_host;
static const char *g_wbx;
static uint8_t *g_ipa;
static size_t g_ipaLen;
static char g_alias[1024];
static char g_settings[8192];
static uint8_t *g_save;
static size_t g_saveLen;
static char g_saveName[1024];
static char g_slots[4096];

static i32fn g_Init, g_IsRunning, g_GetVideoWidth, g_GetVideoHeight;
static u64fn g_GetFrameCount, g_GetMachineTimeNs, g_GetExecutedTicks;
static ptrfn g_GetLoadError, g_GetVideoBgra, g_GetAudio;
static i32fn g_GetAudioSampleCount;
static framefn g_FrameAdvance;
static i32fn g_SaveCount;
static ptrfn_i32 g_SaveName, g_SaveBuffer;
static i64fn_i32 g_SaveSize;
static setfn g_SetAxis;

static void fail(const char *what, mb_return *r)
{
	if (r->error_message[0]) { fprintf(stderr, "%s: %s\n", what, r->error_message); exit(1); }
}

static uintptr_t proc(const char *n)
{
	mb_return r;
	wbx_get_proc_addr(g_host, n, &r);
	fail(n, &r);
	if (!r.data) { fprintf(stderr, "core.wbx exports no %s\n", n); exit(2); }
	return r.data;
}

static void mount(const char *name, const uint8_t *p, size_t len)
{
	reader rd = { p, len, 0 };
	mb_return r;
	wbx_mount_file(g_host, name, buf_read, (uintptr_t)&rd, false, &r);
	fail(name, &r);
}

/* the host, the core loaded and Init run on the engine's mounts, sealed */
static void build_host(void)
{
	size_t wlen;
	uint8_t *w = slurp(g_wbx, &wlen);
	/* matches waterbox.config memoryLayoutMiB */
	mb_memory_layout_template layout = { 256ull << 20, 16ull << 20, 16ull << 20, 256ull << 20, 6144ull << 20 };
	reader rd = { w, wlen, 0 };
	mb_return r;
	wbx_create_host(&layout, "core.wbx", buf_read, (uintptr_t)&rd, &r);
	free(w);
	fail("create", &r);
	g_host = (mb_host *)r.data;

	mount("rom", g_ipa, g_ipaLen);
	mount(g_alias, g_ipa, g_ipaLen);
	mount("rom.name", (const uint8_t *)g_alias, strlen(g_alias));
	if (g_save)
	{
		/* a project: each file under its own name, the slot map saying which */
		mount(g_alias + 1, g_ipa, g_ipaLen);
		mount(g_saveName, g_save, g_saveLen);
		mount("slots", (const uint8_t *)g_slots, strlen(g_slots));
	}
	mount("settings", (const uint8_t *)g_settings, strlen(g_settings));
	wbx_activate_host(g_host, &r);
	fail("activate", &r);

	g_Init = (i32fn)proc("Init");
	g_GetLoadError = (ptrfn)proc("GetLoadError");
	g_FrameAdvance = (framefn)proc("FrameAdvance");
	g_SetAxis = (setfn)proc("SetAxis");
	g_IsRunning = (i32fn)proc("IsRunning");
	g_GetFrameCount = (u64fn)proc("GetFrameCount");
	g_GetMachineTimeNs = (u64fn)proc("GetMachineTimeNs");
	g_GetExecutedTicks = (u64fn)proc("GetExecutedTicks");
	g_GetVideoBgra = (ptrfn)proc("GetVideoBgra");
	g_GetVideoWidth = (i32fn)proc("GetVideoWidth");
	g_GetVideoHeight = (i32fn)proc("GetVideoHeight");
	g_GetAudio = (ptrfn)proc("GetAudio");
	g_SaveCount = (i32fn)proc("GetSaveDataFileCount");
	g_SaveName = (ptrfn_i32)proc("GetSaveDataFileName");
	g_SaveSize = (i64fn_i32)proc("GetSaveDataFileSize");
	g_SaveBuffer = (ptrfn_i32)proc("GetSaveDataFileBuffer");
	g_GetAudioSampleCount = (i32fn)proc("GetAudioSampleCount");

	if (g_Init() != 1)
	{
		fprintf(stderr, "Init failed: %s\n", (const char *)g_GetLoadError());
		exit(1);
	}
	wbx_deactivate_host(g_host, &r);
	wbx_seal(g_host, &r);
	fail("seal", &r);
	wbx_activate_host(g_host, &r);
	fail("activate", &r);
}

static uint64_t fnv64(const uint8_t *p, size_t n)
{
	uint64_t h = 0xcbf29ce484222325ull;
	for (size_t i = 0; i < n; i++) { h ^= p[i]; h *= 0x100000001b3ull; }
	return h;
}

static void write_tga(const char *path, const uint8_t *bgra, int w, int h)
{
	uint8_t hdr[18] = { 0 };
	hdr[2] = 2;
	hdr[12] = (uint8_t)w; hdr[13] = (uint8_t)(w >> 8);
	hdr[14] = (uint8_t)h; hdr[15] = (uint8_t)(h >> 8);
	hdr[16] = 32;
	hdr[17] = 0x28;
	FILE *f = fopen(path, "wb");
	if (!f) { perror(path); exit(1); }
	fwrite(hdr, 1, sizeof hdr, f);
	fwrite(bgra, 1, (size_t)w * h * 4, f);
	fclose(f);
}

/* A zip of what the core lists, entries STORED (no compression): enough for
 * the core to read back, and nothing to get wrong. */
static uint32_t crc32(const uint8_t *p, size_t n)
{
	uint32_t c = 0xffffffffu;
	for (size_t i = 0; i < n; i++)
	{
		c ^= p[i];
		for (int k = 0; k < 8; k++) c = (c >> 1) ^ (0xedb88320u & (0u - (c & 1)));
	}
	return ~c;
}
static void put16(FILE *f, unsigned v) { fputc(v & 0xff, f); fputc((v >> 8) & 0xff, f); }
static void put32(FILE *f, uint32_t v) { put16(f, v & 0xffff); put16(f, v >> 16); }
static void export_save(const char *path)
{
	const int n = g_SaveCount();
	FILE *f = fopen(path, "wb");
	if (!f) { perror(path); exit(1); }
	uint32_t *offsets = calloc((size_t)n + 1, sizeof *offsets), *crcs = calloc((size_t)n + 1, sizeof *crcs);
	const unsigned date = ((2026 - 1980) << 9) | (10 << 5) | 2;
	for (int i = 0; i < n; i++)
	{
		const char *name = (const char *)g_SaveName(i);
		const uint32_t size = (uint32_t)g_SaveSize(i);
		const uint8_t *data = (const uint8_t *)g_SaveBuffer(i);
		printf("save %s %u %016llx\n", name, size, (unsigned long long)fnv64(data, size));
		offsets[i] = (uint32_t)ftell(f);
		crcs[i] = crc32(data, size);
		put32(f, 0x04034b50); put16(f, 20); put16(f, 0); put16(f, 0); put16(f, 0); put16(f, date);
		put32(f, crcs[i]); put32(f, size); put32(f, size); put16(f, (unsigned)strlen(name)); put16(f, 0);
		fwrite(name, 1, strlen(name), f);
		fwrite(data, 1, size, f);
	}
	const uint32_t cd = (uint32_t)ftell(f);
	for (int i = 0; i < n; i++)
	{
		const char *name = (const char *)g_SaveName(i);
		const uint32_t size = (uint32_t)g_SaveSize(i);
		put32(f, 0x02014b50); put16(f, 20); put16(f, 20); put16(f, 0); put16(f, 0); put16(f, 0); put16(f, date);
		put32(f, crcs[i]); put32(f, size); put32(f, size); put16(f, (unsigned)strlen(name));
		put16(f, 0); put16(f, 0); put16(f, 0); put16(f, 0); put32(f, 0); put32(f, offsets[i]);
		fwrite(name, 1, strlen(name), f);
	}
	const uint32_t cdLen = (uint32_t)ftell(f) - cd;
	put32(f, 0x06054b50); put16(f, 0); put16(f, 0); put16(f, (unsigned)n); put16(f, (unsigned)n);
	put32(f, cdLen); put32(f, cd); put16(f, 0);
	fclose(f);
	free(offsets);
	free(crcs);
	fprintf(stderr, "run-wbx: exported %d save files to %s\n", n, path);
}

/* the input schedule, as run-native takes it */
enum { TOUCH, RELEASE, TILT };
typedef struct { long frame; int kind, finger, x, y; } change;
typedef struct { long frame; const char *path; } shot;

int main(int argc, char **argv)
{
	if (argc < 3)
	{
		fprintf(stderr, "usage: run-wbx <core.wbx> <app.ipa> [options]\n");
		return 2;
	}
	g_wbx = argv[1];
	const char *app = argv[2];
	g_ipa = slurp(app, &g_ipaLen);
	const char *base = strrchr(app, '/');
	snprintf(g_alias, sizeof g_alias, "/%s", base ? base + 1 : app);

	long frames = 600, digestEvery = 0;
	int rerecord = 0, session = 0;
	const char *stateOut = NULL;
	const char *exportSave = NULL;
	change changes[1024];
	int nchanges = 0;
	shot shots[256];
	int nshots = 0;
	size_t sl = 0;
	g_settings[sl++] = '{';
	for (int i = 3; i < argc; i++)
	{
		const char *a = argv[i];
		const char *v = i + 1 < argc ? argv[i + 1] : NULL;
		if (!strcmp(a, "--rerecord")) { rerecord = 1; continue; }
		if (!strcmp(a, "--session")) { session = 1; continue; }
		if (!v) { fprintf(stderr, "%s needs a value\n", a); return 2; }
		i++;
		if (!strcmp(a, "--frames")) frames = atol(v);
		else if (!strcmp(a, "--digest-every")) digestEvery = atol(v);
		else if (!strcmp(a, "--state-out")) stateOut = v;
		else if (!strcmp(a, "--export-save")) exportSave = v;
		else if (!strcmp(a, "--savedata"))
		{
			g_save = slurp(v, &g_saveLen);
			const char *sb = strrchr(v, '/');
			snprintf(g_saveName, sizeof g_saveName, "%s", sb ? sb + 1 : v);
		}
		else if (!strcmp(a, "--setting"))
		{
			const char *eq = strchr(v, '=');
			if (!eq) { fprintf(stderr, "--setting KEY=VALUE\n"); return 2; }
			sl += (size_t)snprintf(g_settings + sl, sizeof g_settings - sl, "%s\"%.*s\":\"%s\"",
				sl > 1 ? "," : "", (int)(eq - v), v, eq + 1);
		}
		else if (!strcmp(a, "--touch"))
		{
			change c = { 0, TOUCH, 0, 0, 0 };
			if (sscanf(v, "%ld:%d:%d:%d", &c.frame, &c.finger, &c.x, &c.y) != 4) { fprintf(stderr, "--touch F:FINGER:X:Y\n"); return 2; }
			c.finger--;
			changes[nchanges++] = c;
		}
		else if (!strcmp(a, "--release"))
		{
			change c = { 0, RELEASE, 0, 0, 0 };
			if (sscanf(v, "%ld:%d", &c.frame, &c.finger) != 2) { fprintf(stderr, "--release F:FINGER\n"); return 2; }
			c.finger--;
			changes[nchanges++] = c;
		}
		else if (!strcmp(a, "--tilt"))
		{
			change c = { 0, TILT, 0, 0, 0 };
			if (sscanf(v, "%ld:%d:%d", &c.frame, &c.x, &c.y) != 3) { fprintf(stderr, "--tilt F:X:Y\n"); return 2; }
			changes[nchanges++] = c;
		}
		else if (!strcmp(a, "--screenshot"))
		{
			const char *eq = strchr(v, '=');
			if (!eq) { fprintf(stderr, "--screenshot F=PATH\n"); return 2; }
			shots[nshots].frame = atol(v);
			shots[nshots++].path = eq + 1;
		}
		else { fprintf(stderr, "unknown option %s\n", a); return 2; }
	}
	g_settings[sl++] = '}';
	g_settings[sl] = 0;
	snprintf(g_slots, sizeof g_slots, "{\"game\":[\"%s\"],\"savedata\":[\"%s\"]}", g_alias + 1, g_saveName);

	build_host();

	membuf state = { 0 };
	mb_return r;
	uint64_t buttons = 0;
	uint64_t audioHash = 0xcbf29ce484222325ull;
	int peak = 0;
	for (long frame = 0; frame < frames; frame++)
	{
		if (session && frame == frames / 2)
		{
			state.len = 0;
			wbx_save_state(g_host, mem_write, (uintptr_t)&state, &r);
			fail("save_state", &r);
			wbx_deactivate_host(g_host, &r);
			wbx_destroy_host(g_host, &r);
			build_host();
			state.pos = 0;
			wbx_load_state(g_host, mem_read, (uintptr_t)&state, &r);
			fail("load_state (session)", &r);
		}
		else if (rerecord)
		{
			state.len = 0;
			wbx_save_state(g_host, mem_write, (uintptr_t)&state, &r);
			fail("save_state", &r);
			state.pos = 0;
			wbx_load_state(g_host, mem_read, (uintptr_t)&state, &r);
			fail("load_state", &r);
		}
		for (int i = 0; i < nchanges; i++)
		{
			const change *c = &changes[i];
			if (c->frame != frame) continue;
			if (c->kind == TOUCH)
			{
				buttons |= 1ull << c->finger;
				g_SetAxis(c->finger * 2, c->x);
				g_SetAxis(c->finger * 2 + 1, c->y);
			}
			else if (c->kind == RELEASE)
				buttons &= ~(1ull << c->finger);
			else
			{
				g_SetAxis(4, c->x);
				g_SetAxis(5, c->y);
			}
		}
		g_FrameAdvance(buttons);
		{
			const int16_t *a = (const int16_t *)g_GetAudio();
			const int n = g_GetAudioSampleCount() * 2;
			for (int i = 0; i < n; i++)
			{
				const uint16_t v = (uint16_t)a[i];
				audioHash ^= v & 0xff; audioHash *= 0x100000001b3ull;
				audioHash ^= v >> 8; audioHash *= 0x100000001b3ull;
				const int m = a[i] < 0 ? -a[i] : a[i];
				if (m > peak) peak = m;
			}
		}
		const int w = g_GetVideoWidth(), h = g_GetVideoHeight();
		const uint8_t *px = (const uint8_t *)g_GetVideoBgra();
		for (int i = 0; i < nshots; i++)
			if (shots[i].frame == frame) write_tga(shots[i].path, px, w, h);
		if (digestEvery > 0 && (frame + 1) % digestEvery == 0)
		{
			printf("frame %ld ticks=%llu time_ns=%llu video=%016llx %dx%d peak=%d\n", frame + 1,
				(unsigned long long)g_GetExecutedTicks(), (unsigned long long)g_GetMachineTimeNs(),
				(unsigned long long)fnv64(px, (size_t)w * h * 4), w, h, peak);
			peak = 0;
		}
		if (!g_IsRunning()) break;
	}
	if (exportSave) export_save(exportSave);
	const int w = g_GetVideoWidth(), h = g_GetVideoHeight();
	const uint8_t *px = (const uint8_t *)g_GetVideoBgra();
	printf("frames=%llu ticks=%llu time_ns=%llu video=%016llx %dx%d audio=%016llx running=%d\n",
		(unsigned long long)g_GetFrameCount(), (unsigned long long)g_GetExecutedTicks(),
		(unsigned long long)g_GetMachineTimeNs(),
		(unsigned long long)fnv64(px, (size_t)w * h * 4), w, h,
		(unsigned long long)audioHash, g_IsRunning());
	if (stateOut)
	{
		state.len = 0;
		wbx_save_state(g_host, mem_write, (uintptr_t)&state, &r);
		fail("save_state", &r);
		FILE *f = fopen(stateOut, "wb");
		if (!f) { perror(stateOut); return 1; }
		fwrite(state.b, 1, state.len, f);
		fclose(f);
	}
	if (rerecord || session || stateOut)
		fprintf(stderr, "stateBytes=%zu\n", state.len);
	wbx_deactivate_host(g_host, &r);
	wbx_destroy_host(g_host, &r);
	return 0;
}
