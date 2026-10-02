// Guest-only overrides for libc calls whose syscalls the miniBox surface
// rejects (by design: no host filesystem). One static link means defining
// these here shadows musl's versions. Everything reports a read-only
// filesystem: touchHLE keeps the app's files in the machine's own memory
// (patch 0003), so nothing it does should reach these. Taken from
// chimera-core-azahar, which proved the dynarmic stubs below.
// SPDX-License-Identifier: MIT
#include <cerrno>
#include <cstring>
#include <pthread.h>
#include <signal.h>
#include <sched.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

extern "C" {

int mkdir(const char*, mode_t)
{
  errno = EROFS;
  return -1;
}

int rmdir(const char*)
{
  errno = EROFS;
  return -1;
}

int unlink(const char*)
{
  errno = EROFS;
  return -1;
}

int rename(const char*, const char*)
{
  errno = EROFS;
  return -1;
}

int chmod(const char*, mode_t)
{
  errno = EROFS;
  return -1;
}

// std::thread::hardware_concurrency goes through here; the box is one CPU
// and saying so keeps every pool deterministic.
int sched_getaffinity(pid_t, size_t cpusetsize, cpu_set_t* mask)
{
  if (!mask || cpusetsize < sizeof(unsigned long))
  {
    errno = EINVAL;
    return -1;
  }
  memset(mask, 0, cpusetsize);
  CPU_SET(0, mask);
  return 0;
}

// musl defines all four affinity functions in one object; shadowing one
// means providing all of them.
int sched_setaffinity(pid_t, size_t, const cpu_set_t*)
{
  return 0;
}

int pthread_setaffinity_np(pthread_t, size_t, const cpu_set_t*)
{
  return 0;
}

int pthread_getaffinity_np(pthread_t, size_t cpusetsize, cpu_set_t* mask)
{
  return sched_getaffinity(0, cpusetsize, mask);
}

// dynarmic installs a SIGSEGV handler for its "fastmem" when a JIT is made.
// touchHLE never gives it a fastmem arena (it hands dynarmic a page table), so
// the handler would never run - and a sandboxed guest has no signals to
// install one for. Both calls succeed and change nothing.
int sigaction(int, const struct sigaction*, struct sigaction* old)
{
  if (old)
  {
    memset(old, 0, sizeof *old);
    old->sa_handler = SIG_DFL;
  }
  return 0;
}

// Nothing in the guest has a signal to handle; installing nothing is the same
// outcome as installing something that never runs.
sighandler_t signal(int, sighandler_t)
{
  return SIG_DFL;
}

int sigaltstack(const stack_t*, stack_t* old)
{
  if (old)
  {
    memset(old, 0, sizeof *old);
    old->ss_flags = SS_DISABLE;
  }
  return 0;
}

char* getcwd(char* buf, size_t size)
{
  // one flat namespace; "/" is as true as anything
  if (!buf || size < 2)
  {
    errno = ERANGE;
    return nullptr;
  }
  buf[0] = '/';
  buf[1] = '\0';
  return buf;
}

}  // extern "C"
