/* SoundTest: the touchHLE core's sound, with nothing that cannot be
 * distributed. An iPhone OS app that plays a square wave through OpenAL - a
 * 22050 Hz buffer, so OpenAL Soft resamples it to the core's 44.1 kHz - and
 * then a second, higher one on a second source, so two sources are mixed.
 * It runs on machine time (usleep), so the gate can say which frames must be
 * silent and which must not, and compare the sound between flavors.
 *
 * Built by waterbox/build-testapp.py with the same SDK and stubs as TestApp.
 * The OpenAL entry points are declared here rather than taken from headers. */
typedef unsigned int ALuint;
typedef int ALint, ALenum, ALsizei;
extern void *alcOpenDevice(const char *name);
extern void *alcCreateContext(void *device, const int *attrs);
extern char alcMakeContextCurrent(void *context);
extern void alGenBuffers(ALsizei n, ALuint *buffers);
extern void alBufferData(ALuint buffer, ALenum format, const void *data, ALsizei size, ALsizei freq);
extern void alGenSources(ALsizei n, ALuint *sources);
extern void alSourcei(ALuint source, ALenum param, ALint value);
extern void alSourcePlay(ALuint source);
extern ALenum alGetError(void);
extern int usleep(unsigned int usec);
extern int printf(const char *fmt, ...);

#define AL_FORMAT_MONO16 0x1101
#define AL_BUFFER 0x1009
#define AL_LOOPING 0x1007

static short low[22050], high[22050];

static ALuint play(short *wave, int half_period)
{
	ALuint buffer, source;
	/* a counter, not i / half_period: ARMv6 has no divide instruction, and
	 * the app should need nothing beyond the SDK's own libraries */
	short level = -6000;
	for (int i = 0, n = 0; i < 22050; i++, n++)
	{
		if (n == half_period) { n = 0; level = (short)-level; }
		wave[i] = level;
	}
	alGenBuffers(1, &buffer);
	alBufferData(buffer, AL_FORMAT_MONO16, wave, sizeof low, 22050);
	alGenSources(1, &source);
	alSourcei(source, AL_BUFFER, (ALint)buffer);
	alSourcei(source, AL_LOOPING, 1);
	alSourcePlay(source);
	return source;
}

int main(void)
{
	void *device = alcOpenDevice(0);
	void *context = alcCreateContext(device, 0);
	alcMakeContextCurrent(context);
	printf("SoundTest: silent for 1 s\n");
	usleep(1000000);
	play(low, 25); /* 441 Hz */
	printf("SoundTest: one tone at 1 s, error %d\n", alGetError());
	usleep(1000000);
	play(high, 17); /* ~649 Hz */
	printf("SoundTest: two tones at 2 s, error %d\n", alGetError());
	for (;;)
		usleep(100000);
}
