#!/usr/bin/env python3
"""Install the system execution service or the current login process helper."""
import argparse
import os
import json
import re
import hashlib
from pathlib import Path
import plistlib
import stat
import subprocess


def registered(endpoint):
    command = ['/bin/launchctl', 'print', endpoint]
    result = subprocess.run(command, capture_output=True)
    if result.returncode == 0:
        return True
    if result.returncode == 113:  # launchctl service lookup: no such service in this domain.
        return False
    raise subprocess.CalledProcessError(result.returncode, command)


# Only current-format, natively verified candidates are accepted. No state is initialized here.
def candidate(binary, config):
    if binary is None or config is None or not binary.is_absolute() or not config.is_absolute():
        raise RuntimeError('refresh requires absolute current and candidate binary/config paths')
    # Never execute a proposed program before checking its administrator-owned image/config.
    for path in [binary, *binary.parents, config, *config.parents]:
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise RuntimeError('unprotected refresh candidate')
        listing = subprocess.run(['/bin/ls', '-lde', str(path)], check=True, capture_output=True, text=True).stdout
        if '+' in listing.split()[0]: raise RuntimeError('extended ACL on refresh candidate')
    document = json.loads(config.read_text())
    if document['service']['path'] != str(binary) or not binary.is_file() or hashlib.sha256(binary.read_bytes()).hexdigest() != document['service']['sha256']:
        raise RuntimeError('refresh candidate image mismatch')
    result = subprocess.run([str(binary), '--config', str(config), '--validate-installation'], check=True, capture_output=True, text=True)
    value = json.loads(result.stdout)
    if value['version'] != 2 or value['ipc_version'] != 6 or value['service']['path'] != str(binary):
        raise RuntimeError('candidate deployment/protocol mismatch')
    return value


def verify_registered(endpoint, program):
    current = subprocess.run(['/bin/launchctl', 'print', endpoint], check=True, capture_output=True, text=True)
    match = re.search(r'^\s*pid = ([0-9]+)\s*$', current.stdout, re.MULTILINE)
    if match is None: raise RuntimeError('current registered process identity is unavailable')
    pid = match[1]
    expected_uid = 0 if endpoint.startswith('system/') else os.geteuid()
    for field, expected in [('uid=', str(expected_uid)), ('comm=', program[0]), ('args=', ' '.join(program))]:
        actual = subprocess.run(['/bin/ps', '-p', pid, '-o', field], check=True, capture_output=True, text=True).stdout.strip()
        if actual != expected: raise RuntimeError('registered process does not match this installation')

def refresh(plist, label, domain, endpoint, program, config, next_binary, next_config):
    metadata = plist.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
        raise RuntimeError('unprotected launchd registration')
    with plist.open('rb') as stream:
        current = plistlib.load(stream)
    if current.get('Label') != label or current.get('ProgramArguments') != program or current.get('MachServices') != {label: True}:
        raise RuntimeError('refusing to refresh another installation')
    old = candidate(Path(program[0]), config)
    new = candidate(next_binary, next_config)
    immutable = ['origin', 'tenant', 'enrollment', 'registration_operation', 'state_root', 'helper_work_roots']
    if any(old[key] != new[key] for key in immutable) or any(old['execution'][key] != new['execution'][key] for key in ['work_root', 'material_root']):
        raise RuntimeError('refresh cannot replace identity, storage or unresolved task resources')
    if not registered(endpoint):
        raise RuntimeError('refresh requires the verified current registration to be loaded')
    verify_registered(endpoint, program)
    subprocess.run(['/bin/launchctl', 'bootout', endpoint], check=True)
    if registered(endpoint):
        raise RuntimeError('old registration stop was not confirmed')
    # Use an exclusive sibling and atomic replace; failed restart retains inspectable registration.
    publication = plist.with_suffix('.refresh')
    next_program = [str(next_binary), '--config', str(next_config)]
    if '--user-helper' in program:
        next_program.append('--user-helper')
    current['ProgramArguments'] = next_program
    try:
        fd = os.open(publication, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            plistlib.dump(current, stream); stream.flush(); os.fsync(stream.fileno())
        os.replace(publication, plist)
        subprocess.run(['/bin/launchctl', 'bootstrap', domain, str(plist)], check=True)
        if not registered(endpoint):
            raise RuntimeError('refreshed registration is not loaded')
        print(json.dumps({'phase': 'registered', 'readiness': 'requires-authenticated-user-check', 'scope': 'user' if '--user-helper' in next_program else 'system'}))
    finally:
        publication.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'refresh', 'remove', 'status'])
    parser.add_argument('--scope', choices=['system', 'user'], required=True)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--config', type=Path)
    parser.add_argument('--candidate-binary', type=Path)
    parser.add_argument('--candidate-config', type=Path)
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
    program = [str(binary)]
    if args.config is not None:
        if not args.config.is_absolute():
            raise RuntimeError('an absolute protected deployment path is required')
        for path in [args.config, *args.config.parents]:
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
                raise RuntimeError('unprotected deployment configuration')
        program += ['--config', str(args.config)]
    if not system:
        program += ['--user-helper']
    if args.action == 'refresh':
        refresh(plist, label, domain, endpoint, program, args.config, args.candidate_binary, args.candidate_config)
        return
    if args.action == 'install':
        if binary.is_symlink() or not binary.is_file():
            raise RuntimeError('a regular service binary is required')
        for path in [binary, *binary.parents]:
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or metadata.st_mode & 0o022 or metadata.st_uid not in (0, os.geteuid()):
                raise RuntimeError('unprotected executable path')
        if registered(endpoint):
            raise RuntimeError('service already registered')
        folder.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        folder.mkdir(mode=0o700, exist_ok=True)
        parent = folder.lstat()
        if stat.S_ISLNK(parent.st_mode) or parent.st_uid != os.geteuid() or parent.st_mode & 0o022:
            raise RuntimeError('unprotected launchd directory')
        created = False
        committed = False
        bootstrap_attempted = False
        try:
            descriptor = os.open(plist, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            created = True
            with os.fdopen(descriptor, 'wb') as stream:
                os.fchmod(stream.fileno(), 0o600)
                plistlib.dump({'Label': label, 'ProgramArguments': program,
                              'MachServices': {label: True}, 'RunAtLoad': True,
                              'KeepAlive': False, 'ExitTimeOut': 10,
                              'ProcessType': 'Background'}, stream)
                stream.flush()
                os.fsync(stream.fileno())
            metadata = plist.lstat()
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
                raise RuntimeError('unprotected launchd configuration')
            bootstrap_attempted = True
            subprocess.run(['/bin/launchctl', 'bootstrap', domain, str(plist)], check=True)
            committed = True
        finally:
            if created and not committed:
                # A lost/failed bootstrap receipt does not prove the job was never loaded.
                # If compensation fails, keep the plist so the operator can retry removal.
                if bootstrap_attempted:
                    subprocess.run(['/bin/launchctl', 'bootout', endpoint], check=True)
                plist.unlink(missing_ok=True)
    else:
        with plist.open('rb') as stream:
            current = plistlib.load(stream)
        if current.get('ProgramArguments') != program or current.get('Label') != label:
            raise RuntimeError('refusing to remove another installation')
        if registered(endpoint):
            subprocess.run(['/bin/launchctl', 'bootout', endpoint], check=True)
        plist.unlink()


if __name__ == '__main__':
    main()
