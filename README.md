# chimera-core-touchhle

[touchHLE](https://github.com/touchHLE/touchHLE)'s iPhone OS 2.x-4.0 app
emulator as a [Chimera](https://github.com/ToolAssisted-run/chimera) core,
running in miniBox's sandbox. A project is a decrypted `.ipa`; nothing else is
needed, because touchHLE reimplements iPhone OS and carries the free libraries
apps link against.

- `extern/touchHLE` - upstream, pinned (clone with `--recursive`)
- `patches/` - the series that makes touchHLE a machine run a frame at a time
- `waterbox/` - the core (`guest/`), the native reference (`run-native/`), the
  builds and the gate
- `docs/PLAN.md` - the design and the reasoning behind every decision

Status: in development (issue chimera#177). Not published.
