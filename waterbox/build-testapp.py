#!/usr/bin/env python3
"""Builds touchHLE's own TestApp (extern/touchHLE/tests/TestApp_source) into
build/testapp/TestApp.app - the free, buildable iPhone OS app this core's gate
runs when no game is at hand.

It is what upstream's tests/integration.rs does, done outside the submodule so
the checkout stays clean:
  1. touchHLE dumps the symbols it implements (--dump=symbols);
  2. each library and framework in that list becomes a stub dylib to link
     against, frameworks with the SDK's headers beside them;
  3. TestApp is compiled for ARMv6 + ARMv7 and linked against the stubs, the
     bundled libstdc++ and the SDK.

Needs: a clang that targets arm-apple-ios (clang 12 is what upstream tests;
newer ones work), touchHLE's common-3.0.sdk (its ld and lipo), and a native
touchHLE binary for the dump.

usage: build-testapp.py --touchhle <binary> --sdk <common-3.0.sdk> [--clang <clang>] [--out <dir>]
"""
import argparse
import os
import shutil
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
upstream = os.path.join(root, 'extern', 'touchHLE')
tests = os.path.join(upstream, 'tests')

ap = argparse.ArgumentParser()
ap.add_argument('--touchhle', required=True, help='a native touchHLE binary, for --dump=symbols')
ap.add_argument('--sdk', required=True, help="touchHLE's common-3.0.sdk directory")
ap.add_argument('--clang', default=shutil.which('clang') or '/usr/lib/llvm-20/bin/clang')
ap.add_argument('--out', default=os.path.join(root, 'build', 'testapp'))
args = ap.parse_args()

sdk = os.path.abspath(args.sdk)
out = os.path.abspath(args.out)
stubs = os.path.join(out, 'stubs')
stubs_src = os.path.join(stubs, 'src')
stubs_lib = os.path.join(stubs, 'lib')
stubs_fw = os.path.join(stubs, 'Frameworks')
app = os.path.join(out, 'TestApp.app')
objs = os.path.join(out, 'obj')

for d in (stubs, app, objs):
    shutil.rmtree(d, ignore_errors=True)
os.makedirs(stubs_src)
os.makedirs(objs)
# the bundle's resources (Info.plist, fonts, test files) come from upstream
shutil.copytree(os.path.join(tests, 'TestApp.app'), app)


def clang(output, sources, extra):
    cmd = [args.clang, '--target=arm-apple-ios', '-miphoneos-version-min=2.0',
           '-arch', 'armv6', '-arch', 'armv7', '-fno-stack-protector', '-DPRODUCT_iPhone',
           '-nodefaultlibs', '-B' + os.path.join(sdk, 'usr', 'bin'), '--sysroot=' + sdk]
    cmd += extra + list(sources) + ['-o', output]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(' '.join(cmd) + '\n' + r.stdout + r.stderr)
        sys.exit('build-testapp: building %s failed' % output)


# 1. the symbols touchHLE implements
symbols = os.path.join(stubs, 'SYMBOLS.txt')
subprocess.run([os.path.abspath(args.touchhle), '--dump=symbols', '--dump-file=' + symbols, '--headless'],
               check=True, capture_output=True, cwd=out)  # its log file lands there

# 2. one stub source per library: a "// /path" comment starts a library, and
#    further comments before any body are other names for it
files = []
current = None
in_body = False
for line in open(symbols):
    line = line.rstrip('\n')
    if line.startswith('// '):
        path = line[3:]
        if in_body or current is None:
            name = path.rsplit('/', 1)[1]
            src = os.path.join(stubs_src, name + '.m')
            if current:
                current.close()
            current = open(src, 'w')
            files.append((path, src))
            in_body = False
    elif current is not None:
        in_body = True
        current.write(line + '\n')
if current:
    current.close()

link_extra = []
for path, src in files:
    if path.startswith('/.touchHLE'):
        continue                      # the app picker's own library
    name = path.rsplit('/', 1)[1]
    base = ['-mlinker-version=253', '-fno-builtin', '-nostdlib', '-Wl,-install_name,' + path,
            '-Wno-objc-root-class', '-Wl,-dylib']
    if path.startswith('/System/Library/Frameworks/'):
        fw_path = path[len('/System/Library/Frameworks/'):]
        link_extra += ['-framework', name]
        dest = os.path.join(stubs_fw, fw_path)
        compile_args = base + ['-L' + stubs_lib, '-lobjc.A']
        # only the first framework search path is used, so the stub carries
        # the SDK's headers itself
        headers = os.path.join(sdk, path.rsplit('/', 1)[0].lstrip('/'), 'Headers')
        if os.path.isdir(headers):
            shutil.copytree(headers, os.path.join(os.path.dirname(dest), 'Headers'), dirs_exist_ok=True)
    else:
        assert name.startswith('lib') and name.endswith('.dylib'), name
        link_extra.append('-l' + name[3:-len('.dylib')])
        dest = os.path.join(stubs_lib, name)
        compile_args = base
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    clang(dest, [src], compile_args)

# 3. TestApp itself
# clang 16 made an undeclared function an error; upstream's clang 12 warned,
# and a few GUI tests call CoreGraphics functions the SDK headers lack
extra = ['-mlinker-version=253', '-Wno-expansion-to-defined', '-Wno-literal-range',
         '-Wno-error=implicit-function-declaration',
         '-L' + os.path.join(upstream, 'touchHLE_dylibs'), '-L' + stubs_lib, '-F' + stubs_fw,
         '-ObjC', '-fno-objc-exceptions', '-fno-objc-arc', '-fno-objc-arc-exceptions']
srcdir = os.path.join(tests, 'TestApp_source')
sources = sorted(os.path.join(srcdir, f) for f in os.listdir(srcdir) if f.rsplit('.', 1)[-1] in ('m', 'c', 'cpp'))
cpp_objs = []
for s in [s for s in sources if s.endswith('.cpp')]:
    # -ObjC would make clang read C++ as Objective-C
    o = os.path.join(objs, os.path.basename(s)[:-4] + '.o')
    clang(o, [s], [a for a in extra if a not in ('-ObjC', '-fno-objc-exceptions', '-fno-objc-arc',
                                                  '-fno-objc-arc-exceptions')] + ['-c'])
    cpp_objs.append(o)
clang(os.path.join(app, 'TestApp'), [s for s in sources if not s.endswith('.cpp')] + cpp_objs,
      extra + ['-lstdc++.6.0.9'] + link_extra)
print('built:', os.path.join(app, 'TestApp'))

# 4. as an .ipa, the form the core takes a game in: Payload/<Name>.app/...,
#    every entry dated the same, so the file is a function of its contents
import zipfile


def make_ipa(name, app_dir):
    ipa = os.path.join(out, name + '.ipa')
    with zipfile.ZipFile(ipa, 'w', zipfile.ZIP_DEFLATED) as z:
        for dirpath, dirnames, filenames in os.walk(app_dir):
            dirnames.sort()
            for f in sorted(filenames):
                full = os.path.join(dirpath, f)
                info = zipfile.ZipInfo('Payload/%s.app/' % name + os.path.relpath(full, app_dir),
                                       (2026, 10, 2, 0, 0, 0))
                info.external_attr = 0o100755 << 16 if f == name else 0o100644 << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, open(full, 'rb').read())
    print('built:', ipa)


make_ipa('TestApp', app)

# 5. this repository's own test apps (waterbox/tests/apps/<Name>/: C sources
#    and an Info.plist), against the same stubs
apps = os.path.join(here, 'tests', 'apps')
for name in sorted(os.listdir(apps)) if os.path.isdir(apps) else []:
    src = os.path.join(apps, name)
    app_dir = os.path.join(out, name + '.app')
    shutil.rmtree(app_dir, ignore_errors=True)
    os.makedirs(app_dir)
    shutil.copy(os.path.join(src, 'Info.plist'), app_dir)
    c_sources = sorted(os.path.join(src, f) for f in os.listdir(src) if f.endswith('.c'))
    clang(os.path.join(app_dir, name), c_sources,
          ['-mlinker-version=253', '-L' + stubs_lib, '-F' + stubs_fw] + link_extra)
    make_ipa(name, app_dir)
