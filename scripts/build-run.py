#!/usr/bin/env python3
"""Lease one macOS CI run and its reusable Cargo target slot.

Adapted from rss-mdm hack/build_run.py at 97c11076ca38852dc915cd3f6cf6d1ef5a01003e.
This variant removes nested lease borrowing and direct-target mode, adds gate
descriptor handoff, and bounds allocator wait and slot count.
ref: CPython v3.11.13 Lib/subprocess.py (explicit pass_fds across Python children).
"""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import unicodedata

LEASE_ENV = '_AGENT_BUILD_LEASE'
LOCK_ROOT = Path.home() / '.cache/rss-mdm-agent-build-locks'
POOL_MARKER = '.agent-target-pool-v1'
LOCK_WAIT_SECONDS = 2
MAX_POOL_SLOTS = 32


def log(message):
    print(f'agent-build: {message}', file=sys.stderr, flush=True)


def directory(path):
    if path.is_symlink():
        raise ValueError(f'refusing symlink directory: {path}')
    path.mkdir(parents=True, exist_ok=True)
    return path.resolve()


def lock_file(path):
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError(f'not a regular lock file: {path}')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return fd
    except BlockingIOError:
        os.close(fd)
        return None
    except BaseException:
        os.close(fd)
        raise


def wait_lock_file(path):
    deadline = time.monotonic() + LOCK_WAIT_SECONDS
    while True:
        fd = lock_file(path)
        if fd is not None:
            return fd
        if time.monotonic() >= deadline:
            raise ValueError(f'allocator busy: {path}')
        time.sleep(.02)


def owned_directory(path, marker):
    root = directory(path)
    fd = wait_lock_file(root / '.init.lock')
    try:
        identity = root / marker
        if not identity.exists() and not identity.is_symlink():
            if any(p.name != '.init.lock' for p in root.iterdir()):
                raise ValueError(f'refusing unmarked nonempty directory: {root}')
            out = os.open(identity, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(out, 'w') as stream:
                stream.write(marker + '\n')
        source = os.open(identity, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(source) as stream:
            if stream.read() != marker + '\n':
                raise ValueError(f'invalid directory identity: {root}')
        return root
    finally:
        os.close(fd)


def lock_path(kind, path):
    # Conservatively coalesce case/Unicode aliases even when cargo clean removes the directory.
    # Case-sensitive filesystems may serialize distinct spellings; never permit an alias race.
    key = unicodedata.normalize('NFD', str(path.resolve())).casefold()
    digest = hashlib.sha256(os.fsencode(key)).hexdigest()
    return LOCK_ROOT / f'{kind}-{digest}.lock'


def take_lock(kind, path):
    fd = lock_file(lock_path(kind, path))
    if fd is None:
        raise ValueError(f'{kind} busy: {path}')
    return fd


def target_config(env):
    if 'CARGO_TARGET_DIR' in env or 'CARGO_BUILD_TARGET_DIR' in env:
        raise ValueError('managed CI owns Cargo target; remove external target overrides')
    raw = env.get('AGENT_TARGET_POOL_N', '4')
    if not re.fullmatch(r'[1-9][0-9]*', raw) or len(raw) > 2 or int(raw) > MAX_POOL_SLOTS:
        raise ValueError(f'AGENT_TARGET_POOL_N must be an integer from 1 to {MAX_POOL_SLOTS}')
    root = Path(env.get('AGENT_TARGET_POOL_ROOT', str(Path.home() / '.cache/rss-mdm-agent-cargo-target-pool')))
    return root.absolute(), int(raw)


def metadata(root, index):
    path = root / f'slot-{index}.json'
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd) as stream:
            value = json.load(stream)
        if (not isinstance(value, dict) or not isinstance(value.get('worktree'), str)
                or type(value.get('last_used')) not in (float, int)):
            raise ValueError(f'invalid slot metadata: {path}')
        return value
    except FileNotFoundError:
        return None


def write_metadata(root, index, worktree):
    path = root / f'.lease-{os.getpid()}.tmp'
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump({'worktree': str(worktree), 'last_used': time.time()}, stream)
        os.replace(path, root / f'slot-{index}.json')
    finally:
        path.unlink(missing_ok=True)


def acquire_slot(root, slots, worktree):
    root = owned_directory(root, POOL_MARKER)
    # Only metadata selection holds this lock; busy target slots still fail fast.
    global_fd = wait_lock_file(root / '.pool.lock')
    held = {}
    retired = []
    try:
        indices = set(range(slots))
        indices.update(int(p.name[5:]) for p in root.glob('slot-*') if re.fullmatch(r'slot-[0-9]+', p.name))
        candidates = []
        for index in sorted(indices):
            target = root / f'slot-{index}'
            if target.is_symlink():
                raise ValueError(f'refusing symlink slot: {target}')
            value = metadata(root, index)
            fd = lock_file(lock_path('target', target))
            if fd is None:
                continue
            held[index] = fd
            if index >= slots:
                retired.append(index)
                (root / f'slot-{index}.json').unlink(missing_ok=True)
                continue
            rank = (0 if value and value['worktree'] == str(worktree) else
                    1 if value is None else 2 if not Path(value['worktree']).exists() else 3)
            candidates.append((rank, value['last_used'] if value else 0, index))
        if not candidates:
            raise ValueError(f'pool full ({slots} slots): {root}')
        rank, _, index = min(candidates)
        target = root / f'slot-{index}'
        # Invalidate ownership before cleanup: interruption must force a fresh wipe next time.
        if rank != 0:
            (root / f'slot-{index}.json').unlink(missing_ok=True)
        for unused in set(held) - {index} - set(retired):
            os.close(held.pop(unused))
        os.close(global_fd)
        global_fd = None
        # Only target locks remain held during expensive filesystem work.
        for old in retired:
            obsolete = root / f'slot-{old}'
            if obsolete.exists():
                shutil.rmtree(obsolete)
        if rank != 0 and target.exists():
            shutil.rmtree(target)
        directory(target)
        write_metadata(root, index, worktree)
        return target, held.pop(index)
    finally:
        for fd in held.values():
            os.close(fd)
        if global_fd is not None:
            os.close(global_fd)


def compiler_environment(env):
    # A detached cache server can keep writing target after its client and lease disappear.
    # Managed runs use direct rustc; no compatibility fallback to the old sccache wrapper.
    wrappers = ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER',
                'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')
    if any(env.get(key) for key in wrappers):
        raise ValueError('custom rustc wrapper is incompatible with managed CI')
    env.update(RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='')


def run_child(argv, env, fds):
    process = None
    cancelled = []
    previous = {}
    def group_alive():
        try:
            os.killpg(process.pid, 0)
            return True
        except ProcessLookupError:
            return False

    def forward(sig, _frame):
        if not cancelled:
            cancelled.append((sig, time.monotonic()))
        if process is not None:
            try:
                # ci.mjs handles INT/TERM; HUP/QUIT also trigger its cleanup.
                delivered = signal.SIGTERM if sig in (signal.SIGHUP, signal.SIGQUIT) else sig
                os.killpg(process.pid, delivered)
            except ProcessLookupError:
                pass
    try:
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP, signal.SIGQUIT):
            previous[sig] = signal.signal(sig, forward)
        process = subprocess.Popen(argv, env=env, start_new_session=True, pass_fds=fds)
        if cancelled:
            forward(cancelled[0][0], None)
        while True:
            try:
                result = process.wait(timeout=.1)
                break
            except subprocess.TimeoutExpired:
                if cancelled and time.monotonic() - cancelled[0][1] >= 7:
                    forward(signal.SIGKILL, None)
        # The Node runner may exit while a gate is still alive. Keep both locks
        # until the managed process group has been stopped.
        leaked = group_alive()
        if leaked:
            if not cancelled:
                log('child exited with a live gate; stopping its process group')
                os.killpg(process.pid, signal.SIGTERM)
            end = cancelled[0][1] + 7 if cancelled else time.monotonic() + 7
            while group_alive() and time.monotonic() < end:
                time.sleep(.02)
            if group_alive():
                os.killpg(process.pid, signal.SIGKILL)
        if cancelled:
            return 128 + cancelled[0][0]
        if leaked and result == 0:
            return 2
        return result if result >= 0 else 128 - result
    finally:
        if process is not None and process.poll() is None:
            forward(signal.SIGKILL, None)
            process.wait()
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def main(argv):
    if len(argv) < 2 or argv[0] != '--':
        raise ValueError('usage: build-run.py -- COMMAND [ARG...]')
    if LEASE_ENV in os.environ:
        raise ValueError('nested build runner is not allowed; inherit the existing lease')
    env = os.environ.copy()
    identity = subprocess.run(['/usr/bin/git', 'rev-parse', '--show-toplevel'],
                              capture_output=True, text=True)
    if identity.returncode:
        raise ValueError('build runner requires a Git worktree')
    worktree = Path(identity.stdout.strip()).resolve()
    pool = target_config(env)
    compiler_environment(env)
    owned_directory(LOCK_ROOT, '.agent-build-locks-v1')
    work_fd = take_lock('worktree', worktree)
    target_fd = None
    try:
        target, target_fd = acquire_slot(*pool, worktree)
        env['CARGO_TARGET_DIR'] = str(target)
        fds = (work_fd, target_fd)
        env[LEASE_ENV] = json.dumps({
            'worktree': str(worktree), 'target': str(target), 'fds': fds,
            'locks': [str(lock_path('worktree', worktree)), str(lock_path('target', target))],
        })
        log(f'target={target} pool=on')
        return run_child(argv[1:], env, fds)
    finally:
        # No LOCK_UN: surviving children can still own the same open file descriptions.
        if target_fd is not None:
            os.close(target_fd)
        os.close(work_fd)


if __name__ == '__main__':
    try:
        sys.exit(main(sys.argv[1:]))
    except (ValueError, OSError) as error:
        log(str(error))
        sys.exit(2)
