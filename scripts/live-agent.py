#!/usr/bin/env python3
"""Run the existing ignored Agent journey against a fixed, disposable backend/PG/step-ca.

Uses the backend's existing Environment and pinned CA fixtures, never production inputs.
"""
import argparse
import base64
import contextlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
import tempfile
import urllib.request

REVISION = '407b3febaf5192f397bedf5e0b64c57f75a1285f'
AGENT = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        return listener.getsockname()[1]


@contextlib.contextmanager
def process(command, cwd, log, env):
    with log.open('w') as stream:
        child = subprocess.Popen(list(map(str, command)), cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT)
        try:
            yield child
        finally:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill(); child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True)
    args = parser.parse_args()
    backend = args.backend.resolve()
    actual = subprocess.check_output(['/usr/bin/git', '-C', str(backend), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != REVISION:
        raise RuntimeError('Live backend must be the fixed Agent V1 deliverable revision')
    sys.path.insert(0, str(backend/'hack'))
    from build_run import require_lease
    from t2_environment import Environment, private
    from candidate_fixture import TENANTS
    from apple_fixtures import generate
    from apple_ca import running
    require_lease(backend)
    nginx = shutil.which('nginx')
    if not nginx:
        raise RuntimeError('The real management HTTPS gateway requires nginx on this host')
    curl = shutil.which('curl')
    if not curl:
        raise RuntimeError('The management HTTPS readiness probe requires curl on this host')
    environment = Environment(backend, group='agent-live')
    # Cleanup remains scoped to this worktree/group; logs and private inputs are retained as artifacts.
    try:
        receipt = environment.initialize()
        root = environment.root
        config_path = Path(receipt['configuration'])
        config = json.loads(config_path.read_text())
        fixture = Path(tempfile.mkdtemp(prefix='agent-pki-', dir=root))
        shutil.copyfile(root/'ca.crt', fixture/'ca.crt')
        environment.issue_leaf(fixture, ['localhost'])
        generate(fixture, fixture/'server.crt', fixture/'server.key')
        child_env = {**os.environ, 'MDM_AGENT_PKI_FIXTURE': '1'}
        with running(fixture, child_env):
            agent_pki = json.loads((fixture/'agent-pki.json').read_text())
            agent_port = free_port()
            agent_origin = f'https://localhost:{agent_port}'
            agent_pki['endpoint'].update(listen=f'127.0.0.1:{agent_port}', origin=agent_origin)
            config['agent_pki'] = agent_pki
            content = root/'agent-content'
            content.mkdir(exist_ok=True, mode=0o700)
            config['content'] = dict(directory=str(content), imports={}, max_artifact_bytes=33554432,
                max_temporary_bytes=67108864, max_uploads=4, transfer_seconds=60,
                retention_seconds=3600, max_bundle_bytes=67108864, max_bundle_entries=100, max_expansion_ratio=100)
            pem = root/'agent-task-signing.pem'
            subprocess.run(['openssl', 'genpkey', '-algorithm', 'ED25519', '-out', pem], check=True, capture_output=True)
            pem.chmod(0o600)
            keyfile = root/'agent-task-signing.pk8'
            subprocess.run(['openssl', 'pkcs8', '-topk8', '-nocrypt', '-in', pem, '-outform', 'DER', '-out', keyfile], check=True, capture_output=True)
            keyfile.chmod(0o600)
            public = subprocess.check_output(['openssl', 'pkey', '-in', pem, '-pubout', '-outform', 'DER'])
            if len(public) != 44 or public[:12] != bytes.fromhex('302a300506032b6570032100'):
                raise RuntimeError('Unexpected Ed25519 public key encoding')
            private_key = keyfile.read_bytes()
            if len(private_key) != 48 or private_key[:16] != bytes.fromhex('302e020100300506032b657004220420'):
                raise RuntimeError('Unexpected Ed25519 PKCS8 encoding')
            # RFC 5958 OneAsymmetricKey includes the public key required by ring 0.17.14.
            keyfile.write_bytes(bytes.fromhex('3051020101300506032b657004220420') + private_key[16:] + bytes.fromhex('812100') + public[12:])
            encoded = base64.urlsafe_b64encode(public[12:]).decode().rstrip('=')
            config['task_signing'] = dict(private_key_file=str(keyfile), key_id='agent-live', trusted_keys={'agent-live': encoded})
            private(config_path, config)
            live = private(root/'agent-live.json', dict(management_origin=receipt['origin'], agent_origin=agent_origin,
                tenant=TENANTS[0], management_ca_file=str(root/'ca.crt'), agent_ca_file=str(root/'ca.crt'),
                device_ca_file=agent_pki['issuer_certificate_file'], admin_password_file=str(root/'host/operator/account-password'),
                admin_login='admin', key_id='agent-live', public_key=encoded))
            build = subprocess.run(['cargo', 'build', '--locked', '-p', 'rss-mdm-app', '--bin', 'rss-mdm', '--message-format=json'], cwd=backend, text=True, capture_output=True, check=True)
            binaries = [value['executable'] for line in build.stdout.splitlines() if (value := json.loads(line)).get('reason')=='compiler-artifact' and value.get('executable') and value['target']['name']=='rss-mdm']
            if len(binaries)!=1:
                raise RuntimeError('Fixed backend binary unavailable')
            with process([binaries[0], 'serve', '--config', config_path], backend, root/'agent-live-backend.log', child_env) as server:
                end = time.monotonic()+40
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                while True:
                    if server.poll() is not None:
                        raise RuntimeError('Fixed backend exited; see private agent-live-backend.log')
                    try:
                        request = urllib.request.Request('http://'+config['listen']+'/readyz', headers={'Host': receipt['origin'].removeprefix('https://')})
                        with opener.open(request, timeout=1) as response:
                            if response.status==200: break
                    except OSError:
                        pass
                    if time.monotonic()>end: raise RuntimeError('Fixed backend readiness deadline')
                    time.sleep(.1)
                with process([nginx, '-c', receipt['gateway'], '-g', 'daemon off;'], backend, root/'agent-live-gateway.log', child_env) as gateway:
                    ready = time.monotonic()+15
                    while True:
                        if gateway.poll() is not None:
                            raise RuntimeError('Management gateway exited during readiness')
                        # Verify HTTPS with the explicit fixture CA. The producer's
                        # fixture lacks AKI, which Python 3.13+ strict mode requires.
                        probe = subprocess.run([curl, '--noproxy', '*', '--fail', '--silent',
                            '--show-error', '--max-time', '1', '--cacert', root/'ca.crt',
                            receipt['origin']+'/readyz'], capture_output=True, text=True)
                        if probe.returncode == 0: break
                        if time.monotonic()>ready: raise RuntimeError(f'Management gateway readiness deadline: {probe.stderr.strip()}')
                        time.sleep(.1)
                    env = dict(os.environ)
                    for key in ['_MDM_BUILD_LEASE', 'CARGO_TARGET_DIR']:
                        env.pop(key, None)
                    env['AGENT_LIVE_CONFIG'] = str(live)
                    result = subprocess.run([sys.executable, 'scripts/build-run.py', '--', 'cargo', 'test', '--locked', '-p', 'agent-service', '--lib', 'backend::live_mdm', '--', '--ignored'], cwd=AGENT, env=env)
                    if gateway.poll() is not None: raise RuntimeError('Management gateway exited; see private gateway log')
                    if result.returncode: raise RuntimeError('Existing Agent live journey failed')
    finally:
        environment.stop()


if __name__ == '__main__':
    main()
