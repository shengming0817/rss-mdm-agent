#!/usr/bin/env python3
"""Destructive local SIT reset, followed by installation of a verified PKG. No server data is deleted."""
# ref: Apple Security SecItem.h (scoped SecItemDelete); launchctl(1); installer(8).
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import shutil
import signal
import subprocess
import sys
import time

BASE = Path('/Library/Application Support/RSS MDM Agent')
LABEL = 'com.rss-mdm.agent.execution'


def run(args, missing=False):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True, timeout=300)
    if result.returncode and not missing:
        raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr.strip()}')
    return result


def delete_credentials(keychain, entries):
    """Delete exact attributes without reading secrets or authorizing another executable."""
    security = C.CDLL('/System/Library/Frameworks/Security.framework/Security')
    cf = C.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
    ptr = C.c_void_p
    cf.CFStringCreateWithCString.argtypes = [ptr, C.c_char_p, C.c_uint32]
    cf.CFStringCreateWithCString.restype = ptr
    cf.CFDataCreate.argtypes = [ptr, ptr, C.c_long]
    cf.CFDataCreate.restype = ptr
    cf.CFArrayCreate.argtypes = [ptr, C.POINTER(ptr), C.c_long, ptr]
    cf.CFArrayCreate.restype = ptr
    cf.CFDictionaryCreate.argtypes = [ptr, C.POINTER(ptr), C.POINTER(ptr), C.c_long, ptr, ptr]
    cf.CFDictionaryCreate.restype = ptr
    cf.CFRelease.argtypes = [ptr]
    security.SecKeychainOpen.argtypes = [C.c_char_p, C.POINTER(ptr)]
    security.SecKeychainSetUserInteractionAllowed.argtypes = [C.c_bool]
    security.SecKeychainItemDelete.argtypes = [ptr]
    security.SecItemCopyMatching.argtypes = [ptr, C.POINTER(ptr)]
    kc = ptr()
    if security.SecKeychainOpen(os.fsencode(keychain), C.byref(kc)):
        raise RuntimeError('Cannot open the selected Keychain')
    if security.SecKeychainSetUserInteractionAllowed(False):
        raise RuntimeError('Cannot disable interactive credential prompts')
    constant = lambda name: ptr.in_dll(security, name).value
    try:
        for service, account, key in entries:
            owned = []
            def string(value):
                obj = cf.CFStringCreateWithCString(None, value.encode(), 0x08000100)
                owned.append(obj)
                return obj
            search = cf.CFArrayCreate(None, (ptr * 1)(kc), 1, None)
            owned.append(search)
            fields = {
                constant('kSecClass'): constant('kSecClassKey' if key else 'kSecClassGenericPassword'),
                constant('kSecMatchSearchList'): search,
            }
            if key:
                data = account.encode()
                tag = cf.CFDataCreate(None, C.cast(C.c_char_p(data), ptr), len(data))
                owned.append(tag)
                fields[constant('kSecAttrApplicationTag')] = tag
                fields[constant('kSecAttrKeyClass')] = constant('kSecAttrKeyClassPrivate')
            else:
                fields[constant('kSecAttrService')] = string(service)
                fields[constant('kSecAttrAccount')] = string(account)
            fields[constant('kSecReturnRef')] = ptr.in_dll(cf, 'kCFBooleanTrue').value
            query = cf.CFDictionaryCreate(None, (ptr * len(fields))(*fields),
                                          (ptr * len(fields))(*fields.values()), len(fields), None, None)
            owned.append(query)
            try:
                found = ptr()
                status = security.SecItemCopyMatching(query, C.byref(found))
                if status == -25300:
                    continue
                if status != 0 or not found:
                    raise RuntimeError(f'Exact credential lookup failed ({status}); no interactive retry')
                try:
                    status = security.SecKeychainItemDelete(found)
                finally:
                    cf.CFRelease(found)
                if status not in (0, -25300):
                    raise RuntimeError(f'Exact credential deletion failed ({status}); no interactive retry')
                found = ptr()
                status = security.SecItemCopyMatching(query, C.byref(found))
                if found:
                    cf.CFRelease(found)
                if status != -25300:
                    raise RuntimeError(f'Credential absence could not be confirmed ({status})')
            finally:
                for obj in reversed(owned):
                    cf.CFRelease(obj)
    finally:
        cf.CFRelease(kc)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path)
    parser.add_argument('--sha256')
    parser.add_argument('--tenant')
    parser.add_argument('--plan', action='store_true', help='Show reset scope without changing anything')
    parser.add_argument('--user-credential', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('Run on the macOS SIT development machine')
    if args.user_credential:
        delete_credentials(Path.home() / 'Library/Keychains/login.keychain-db',
                           [('RSS MDM Agent', 'connection-master-key', False)])
        return
    if os.geteuid() != 0:
        parser.error('Run with sudo; the script does not store administrator passwords')
    if not args.package or not args.package.is_absolute() or not re.fullmatch('[0-9a-f]{64}', args.sha256 or ''):
        parser.error('Provide an absolute --package and its --sha256 before deleting local state')
    if hashlib.sha256(args.package.read_bytes()).hexdigest() != args.sha256:
        raise RuntimeError('Package digest mismatch; nothing was changed')
    config = json.loads((BASE / 'execution.json').read_text())
    if config['tenant'] != args.tenant or Path(config['state_root']) != BASE / 'state':
        raise RuntimeError('Installed tenant or local state root does not match this SIT reset')
    manifest = json.loads((Path(config['service']['path']).parent / 'candidate.json').read_text())
    if manifest['signing_mode'] != 'development':
        raise RuntimeError('This script only resets a development installation')
    namespace = hashlib.sha256(json.dumps([config['origin'], config['tenant'], config['installation_id']],
                                          separators=(',', ':')).encode()).hexdigest()
    secrets = BASE / 'state/secrets'
    if (secrets / 'credential-owner').read_text() != namespace:
        raise RuntimeError('Cannot establish ownership of the installed credentials')
    entries = []
    for path in secrets.glob('*.credential.json'):
        raw = path.read_bytes()
        if path.name != hashlib.sha256(raw).hexdigest() + '.credential.json':
            raise RuntimeError('Credential catalog digest mismatch')
        reference, key = json.loads(raw)
        if not isinstance(key, bool) or not re.fullmatch('[A-Za-z0-9._-]{1,128}', reference):
            raise RuntimeError('Invalid credential catalog entry')
        entries.append(('RSS MDM Agent device', namespace + '-' + reference, key))
    uid = os.stat('/dev/console').st_uid
    if uid == 0 or config['clients']['subjects'] != [str(uid)]:
        raise RuntimeError('The current console user must be the sole installed SIT desktop user')
    user = pwd.getpwuid(uid)
    desktop = Path(user.pw_dir) / 'Library/Application Support/com.rss.mdmagent'
    helper = Path(user.pw_dir) / 'Library/Application Support/RSS MDM Agent/helper'
    args.package = args.package.resolve(strict=True)
    if any(args.package.is_relative_to(path.resolve()) for path in [BASE, desktop, helper]):
        raise RuntimeError('The installation package must be outside every directory being cleared')
    app = Path('/Applications/RSS MDM Agent.app')
    if not app.is_symlink() or not app.resolve().is_relative_to(BASE / 'versions'):
        raise RuntimeError('The desktop entry is not owned by this installation')
    for path in [BASE, desktop, helper]:
        if path.is_symlink():
            raise RuntimeError('Refusing a redirected product directory')
    print(json.dumps({'tenant': args.tenant, 'deleteDeviceCredentials': len(entries),
                      'deleteLocalProductState': True, 'deleteUserAgentCredential': True,
                      'archive': False, 'deleteServerData': False, 'planOnly': args.plan}), flush=True)
    if args.plan:
        return
    # launchd is stopped before deleting credentials or files. No application UI automation is used.
    run(['/bin/launchctl', 'bootout', f'gui/{uid}/{LABEL}.user'], missing=True)
    run(['/bin/launchctl', 'bootout', f'system/{LABEL}'], missing=True)
    for job in [f'gui/{uid}/{LABEL}.user', f'system/{LABEL}']:
        for attempt in range(20):
            if run(['/bin/launchctl', 'print', job], missing=True).returncode != 0:
                break
            time.sleep(0.25)
        else:
            raise RuntimeError('Agent launch job did not stop; local state was not deleted')
    processes = run(['/bin/ps', '-axo', 'pid=,comm=']).stdout
    selected = []
    for line in processes.splitlines():
        pid, command = line.strip().split(None, 1)
        if command.startswith(str(BASE / 'versions') + '/'):
            selected.append(int(pid))
            try:
                os.kill(int(pid), signal.SIGTERM)
            except ProcessLookupError:
                pass
    time.sleep(1)
    for pid in selected:
        current = run(['/bin/ps', '-p', str(pid), '-o', 'comm='], missing=True).stdout.strip()
        if current.startswith(str(BASE / 'versions') + '/'):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    delete_credentials('/Library/Keychains/System.keychain', entries)
    run(['/usr/bin/sudo', '-u', user.pw_name, '/usr/bin/python3', Path(__file__).resolve(), '--user-credential'])
    for path in [Path('/Library/LaunchDaemons') / (LABEL + '.plist'),
                 Path('/Library/LaunchAgents') / (LABEL + '.user.plist'), app]:
        path.unlink(missing_ok=True)
    for path in [BASE, desktop, helper]:
        if path.exists():
            shutil.rmtree(path)
    run(['/usr/sbin/pkgutil', '--forget', 'com.rss-mdm.agent'], missing=True)
    run(['/usr/sbin/installer', '-pkg', args.package, '-target', '/'])
    fresh = json.loads((BASE / 'execution.json').read_text())
    if fresh['tenant'] != args.tenant or fresh['installation_id'] == config['installation_id']:
        raise RuntimeError('The installed package did not create the expected fresh SIT installation')
    status = run([fresh['service']['path'], '--installation', 'status']).stdout.strip()
    print(json.dumps({'freshInstallation': True, 'installationId': fresh['installation_id'],
                      'registration': 'pending', 'nativeStatus': status}), flush=True)


if __name__ == '__main__':
    main()
