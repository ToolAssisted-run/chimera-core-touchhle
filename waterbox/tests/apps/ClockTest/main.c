/* ClockTest: the touchHLE core's clocks, from inside an app. Once a second
 * of machine time it reports:
 *  - time(): the date, which starts at the rtc_start setting and runs with
 *    the machine;
 *  - how many times it could read mach_absolute_time() in a busy loop until
 *    a quarter of a second had passed on it: machine time is bought with the
 *    app's own instructions, so this count follows the cpu_mhz setting and
 *    nothing on the host.
 * The gate stalls the host and changes cpu_mhz and rtc_start against it.
 *
 * Built by waterbox/build-testapp.py with TestApp's SDK and stubs. */
typedef unsigned long long uint64_t;
typedef struct { unsigned int numer, denom; } mach_timebase_info_data_t;
extern uint64_t mach_absolute_time(void);
extern int mach_timebase_info(mach_timebase_info_data_t *info);
extern long time(long *t);
extern int printf(const char *fmt, ...);
extern int usleep(unsigned int usec);

int main(void)
{
	mach_timebase_info_data_t tb;
	mach_timebase_info(&tb);
	for (int second = 0;; second++)
	{
		/* a quarter of a second, in mach_absolute_time units, without a
		 * divide (ARMv6 has none): count up in nanoseconds instead */
		uint64_t start = mach_absolute_time(), now = start;
		unsigned reads = 0;
		for (;;)
		{
			now = mach_absolute_time();
			reads++;
			uint64_t ticks = now - start, ns = 0;
			for (unsigned k = 0; k < tb.numer; k++) ns += ticks;
			if (ns >= 250000000ull * tb.denom) break;
		}
		printf("ClockTest: second %d time %ld reads %u\n", second, time(0), reads);
		usleep(1000000);
	}
}
