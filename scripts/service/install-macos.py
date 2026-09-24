#!/usr/bin/env python3
"""Administrator-only lab install. Publish protected staging; fail closed after stopping."""
import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path
import plistlib
import pwd
import re
import shutil
import stat
import subprocess
import tempfile
import uuid

ROOT = Path('/Library/Application Support/RSS MDM Agent/service')
APP = ROOT.parent / 'desktop/RSS MDM Agent.app'
LABEL = 'com.rss-mdm.agent.status'
PLIST = Path('/Library/LaunchDaemons') / (LABEL + '.plist')
ACCOUNT = '_rssmdmstatus'


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def artifact(path, installed=None):
    details = subprocess.run(['/usr/bin/codesign', '-dv', '--verbose=4', str(path)],
                             check=True, capture_output=True, text=True).stderr
    match = re.search(r'^CDHash=([a-f0-9]{40})$', details, re.M)
    if not match:
        raise RuntimeError('fixed code-directory identity unavailable')
    return dict(path=str(installed or path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(), cdhash=match[1])


def stop():
    loaded = subprocess.run(['/bin/launchctl', 'print', 'system/' + LABEL], capture_output=True).returncode == 0
    if loaded:
        run('/bin/launchctl', 'bootout', 'system/' + LABEL)
    if subprocess.run(['/bin/launchctl', 'print', 'system/' + LABEL], capture_output=True).returncode == 0:
        raise RuntimeError('old service did not stop')


def protected(path):
    for entry in [path, *path.parents]:
        metadata = entry.lstat()
        if stat.S_ISLNK(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise RuntimeError('administrator-owned immutable installation required')
        library = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
        library.acl_get_file.argtypes = [ctypes.c_char_p, ctypes.c_int]
        library.acl_get_file.restype = ctypes.c_void_p
        library.acl_get_entry.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.POINTER(ctypes.c_void_p)]
        library.acl_free.argtypes = [ctypes.c_void_p]
        acl = library.acl_get_file(os.fsencode(entry), 0x100)
        if not acl:
            if ctypes.get_errno() == errno.ENOENT:
                entry.lstat()
                continue
            raise RuntimeError('installation ACL unavailable')
        try:
            value = ctypes.c_void_p()
            ctypes.set_errno(0)
            if library.acl_get_entry(acl, 0, ctypes.byref(value)) != -1 or ctypes.get_errno() != errno.EINVAL:
                raise RuntimeError('extended installation ACL rejected')
        finally:
            library.acl_free(acl)


def protect(root):
    for directory, dirs, files in os.walk(root, followlinks=False):
        for path in [Path(directory), *(Path(directory) / p for p in dirs + files)]:
            if path.is_symlink():
                os.chown(path, 0, 0, follow_symlinks=False)
            else:
                executable = path.is_dir() or bool(path.stat().st_mode & 0o111)
                os.chown(path, 0, 0)
                os.chmod(path, 0o755 if executable else 0o644)


def preflight(desktop, service):
    # Conflicts are rejected before touching the running service.
    if APP.exists() or APP.is_symlink():
        raise RuntimeError('remove the previous lab app explicitly; existing service was not stopped')
    if not desktop.is_dir() or not service.is_file():
        raise RuntimeError('complete fixed desktop and service artifacts required')
    with (desktop / 'Contents/Info.plist').open('rb') as file:
        executable = plistlib.load(file)['CFBundleExecutable']
    if not isinstance(executable, str) or executable in ('', '.', '..') or Path(executable).name != executable:
        raise RuntimeError('invalid app executable')
    parent = ROOT
    while not parent.exists() and not parent.is_symlink():
        parent = parent.parent
    protected(parent)
    return executable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--desktop', type=Path, required=True)
    parser.add_argument('--service', type=Path, required=True)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--allow-user', action='append', required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise RuntimeError('run this installer as administrator')
    if os.uname().machine != 'arm64' or tuple(map(int, run('/usr/bin/sw_vers', '-productVersion').strip().split('.')[:2])) < (26, 4):
        raise RuntimeError('requires macOS 26.4+ arm64')
    users = [str(pwd.getpwuid(int(uid)).pw_uid) for uid in args.allow_user]
    if '0' in users:
        raise RuntimeError('ordinary user required')
    executable = preflight(args.desktop, args.service)
    if not args.probe.is_file():
        raise RuntimeError('probe artifact required')
    try:
        account = pwd.getpwnam(ACCOUNT)
        if account.pw_uid == 0 or account.pw_shell != '/usr/bin/false' or account.pw_dir != '/var/empty':
            raise RuntimeError('existing service account is not the dedicated non-login identity')
    except KeyError:
        used = {p.pw_uid for p in pwd.getpwall()}
        uid = next(uid for uid in range(400, 500) if uid not in used)
        for key, value in [('UniqueID', str(uid)), ('PrimaryGroupID', '20'), ('UserShell', '/usr/bin/false'),
                           ('NFSHomeDirectory', '/var/empty'), ('IsHidden', '1'), ('RealName', 'RSS status service')]:
            run('/usr/bin/dscl', '.', '-create', '/Users/' + ACCOUNT, key, value)
        account = pwd.getpwnam(ACCOUNT)
    ROOT.mkdir(mode=0o755, parents=True, exist_ok=True)
    protected(ROOT)
    APP.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
    protected(APP.parent)
    old = ROOT / 'policy.json'
    installation = str(uuid.uuid4())
    if old.exists():
        protected(old)
        installation = json.loads(old.read_text())['installation']
    published = stopped = False
    # This container stays mode 0700 throughout staging, even when its children
    # receive their final readable modes. No untrusted process can hold a writer.
    with tempfile.TemporaryDirectory(prefix='.rss-service-stage-', dir=APP.parent) as temporary:
        stage = Path(temporary)
        staged_app = stage / APP.name
        staged_service = stage / 'rss-local-service'
        staged_probe = stage / 'rss-untrusted-service-probe'
        try:
            shutil.copytree(args.desktop, staged_app, symlinks=True)
            shutil.copy2(args.service, staged_service)
            shutil.copy2(args.probe, staged_probe)
            for path in (staged_app, staged_service, staged_probe):
                run('/bin/chmod', '-RN', str(path))
            for path in (staged_app, staged_service, staged_probe):
                run('/usr/bin/codesign', '--force', '--options', 'runtime', '--sign', '-', str(path))
                run('/usr/bin/codesign', '--verify', '--strict', str(path))
            protect(staged_app)
            os.chown(staged_service, 0, 0)
            os.chmod(staged_service, 0o755)
            os.chown(staged_probe, 0, 0)
            os.chmod(staged_probe, 0o755)
            service = ROOT / 'rss-local-service'
            probe = ROOT / 'rss-untrusted-service-probe'
            policy = dict(version=1, installation=installation, build=hashlib.sha256(staged_service.read_bytes()).hexdigest(),
                          platform='macos-arm64', service_subject=str(account.pw_uid), allowed_users=users,
                          client=artifact(staged_app / 'Contents/MacOS' / executable, APP / 'Contents/MacOS' / executable),
                          probe=artifact(staged_probe, probe),
                          service=artifact(staged_service, service))
            stop()
            stopped = True
            staged_app.rename(APP)
            published = True
            staged_service.replace(service)
            staged_probe.replace(probe)
            new_policy = ROOT / ('policy-' + str(uuid.uuid4()) + '.new')
            new_policy.write_text(json.dumps(policy, indent=2) + '\n')
            os.chown(new_policy, 0, 0)
            os.chmod(new_policy, 0o644)
            new_policy.replace(old)
            value = dict(Label=LABEL, ProgramArguments=[str(service)], UserName=ACCOUNT,
                         MachServices={LABEL: True}, RunAtLoad=True, KeepAlive=True, ProcessType='Background')
            with PLIST.open('wb') as file:
                plistlib.dump(value, file)
            os.chown(PLIST, 0, 0)
            os.chmod(PLIST, 0o644)
            run('/bin/launchctl', 'bootstrap', 'system', str(PLIST))
            response = json.loads(run('/usr/bin/sudo', '-u', '#' + users[0], '--', str(APP / 'Contents/MacOS' / executable), '--service-probe'))
            if response.get('phase') != 'connected':
                raise RuntimeError('ordinary-user service readiness failed')
        except Exception:
            if stopped:
                stop()
            if published:
                shutil.rmtree(APP)
            # Retain protected service evidence; never restore an old policy.
            raise
    print('Ordinary-user query passed; the complete platform security matrix remains required.')


if __name__ == '__main__':
    main()
