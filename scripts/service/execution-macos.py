#!/usr/bin/env python3
"""Local launchd mechanism setup. Unbound services cannot execute submitted work."""
import argparse
import os
from pathlib import Path
import plistlib
import stat
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'remove', 'status'])
    parser.add_argument('--scope', choices=['system', 'user'], required=True)
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    system = args.scope == 'system'
    if system and os.geteuid() != 0:
        raise RuntimeError('system scope requires administrator execution')
    if not system and os.geteuid() == 0:
        raise RuntimeError('user helper must be installed by its actual user')
    label = 'com.rss-mdm.agent.execution' + ('' if system else '.user')
    domain = 'system' if system else f'gui/{os.geteuid()}'
    folder = Path('/Library/LaunchDaemons') if system else Path.home() / 'Library/LaunchAgents'
    plist = folder / (label + '.plist')
    endpoint = domain + '/' + label
    if args.action == 'status':
        subprocess.run(['/bin/launchctl', 'print', endpoint], check=True)
        return
    binary = args.binary
    if binary is None or not binary.is_absolute():
        raise RuntimeError('an absolute installation path is required')
    if args.action == 'install':
        if binary.is_symlink() or not binary.is_file():
            raise RuntimeError('a regular service binary is required')
        for path in [binary, *binary.parents]:
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or metadata.st_mode & 0o022 or metadata.st_uid not in (0, os.geteuid()):
                raise RuntimeError('unprotected executable path')
        if subprocess.run(['/bin/launchctl', 'print', endpoint], capture_output=True).returncode == 0:
            raise RuntimeError('service already registered')
        folder.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        folder.mkdir(mode=0o700, exist_ok=True)
        parent = folder.lstat()
        if stat.S_ISLNK(parent.st_mode) or parent.st_uid != os.geteuid() or parent.st_mode & 0o022:
            raise RuntimeError('unprotected launchd directory')
        created = False
        committed = False
        try:
            descriptor = os.open(plist, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            created = True
            with os.fdopen(descriptor, 'wb') as stream:
                os.fchmod(stream.fileno(), 0o600)
                plistlib.dump({'Label': label, 'ProgramArguments': [str(binary)],
                              'MachServices': {label: True}, 'RunAtLoad': True,
                              'KeepAlive': False, 'ExitTimeOut': 5,
                              'ProcessType': 'Background'}, stream)
                stream.flush()
                os.fsync(stream.fileno())
            metadata = plist.lstat()
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
                raise RuntimeError('unprotected launchd configuration')
            subprocess.run(['/bin/launchctl', 'bootstrap', domain, str(plist)], check=True)
            committed = True
        finally:
            if created and not committed:
                plist.unlink(missing_ok=True)
    else:
        with plist.open('rb') as stream:
            current = plistlib.load(stream)
        if current.get('ProgramArguments') != [str(binary)] or current.get('Label') != label:
            raise RuntimeError('refusing to remove another installation')
        if subprocess.run(['/bin/launchctl', 'print', endpoint], capture_output=True).returncode == 0:
            subprocess.run(['/bin/launchctl', 'bootout', endpoint], check=True)
        plist.unlink()


if __name__ == '__main__':
    main()
