/* SaveTest: save data in and out of the touchHLE core. An iPhone OS app that
 * keeps a counter in Documents/counter.txt: it reads the counter it finds
 * (0 when there is none), then once a second of machine time writes the next
 * value and says so. Exporting its save data and starting again from that
 * export must carry on counting where the first run stopped.
 *
 * Built by waterbox/build-testapp.py with TestApp's SDK and stubs. */
typedef struct FILE FILE;
extern FILE *fopen(const char *path, const char *mode);
extern int fclose(FILE *f);
extern int fscanf(FILE *f, const char *fmt, ...);
extern int fprintf(FILE *f, const char *fmt, ...);
extern int printf(const char *fmt, ...);
extern int snprintf(char *s, unsigned long n, const char *fmt, ...);
extern char *getenv(const char *name);
extern int usleep(unsigned int usec);

int main(void)
{
	char path[1024];
	snprintf(path, sizeof path, "%s/Documents/counter.txt", getenv("HOME"));
	int counter = 0;
	FILE *f = fopen(path, "r");
	if (f)
	{
		if (fscanf(f, "%d", &counter) != 1)
			counter = -1000;
		fclose(f);
	}
	printf("SaveTest: found %d\n", counter);
	for (;;)
	{
		usleep(1000000);
		counter++;
		f = fopen(path, "w");
		if (!f)
		{
			printf("SaveTest: cannot write %s\n", path);
			continue;
		}
		fprintf(f, "%d\n", counter);
		fclose(f);
		printf("SaveTest: wrote %d\n", counter);
	}
}
