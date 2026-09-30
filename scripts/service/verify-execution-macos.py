#!/usr/bin/env python3
"""Exercise the installed production service with a controlled signed HTTPS backend.

Build agent-service and its controlled-backend example first. Only registration created by this
run is removed; credentials, execution journal and materials remain for inspection. Native macOS
administrator authorization is required. This is an explicit acceptance entry, not a CI fallback.
"""
import argparse
import hashlib
import http.client
import http.server
import json
import os
from pathlib import Path
import re
import shlex
import ssl
import subprocess
import threading
import time
import uuid


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)


def administrator(script):
    command = '/usr/bin/python3 ' + shlex.quote(str(script))
    return run('/usr/bin/osascript', '-e',
               'do shell script ' + json.dumps(command) + ' with administrator privileges')


def acknowledged_result(status, attempt):
    matches = [(operation, result) for operation, result in status['results'].items()
               if result.get('attemptId') == attempt and operation in status['acknowledged']]
    if not matches:
        return None
    if len(matches) != 1:
        raise AssertionError('more than one logical result for the original attempt')
    return matches[0][1]['event']


def validate_script(event, fixture):
    assert event['kind'] == 'result'
    assert event['exitCode'] == 0
    assert event['output'] == {'fixture': fixture}
    assert event['quality'] in ('complete', 'partial')
    assert event['diagnostics']['failure'] is None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.uname().sysname != 'Darwin' or os.geteuid() == 0:
        raise RuntimeError('run this harness from the actual macOS user login')
    for domain in ['system/com.rss-mdm.agent.execution', f'gui/{os.geteuid()}/com.rss-mdm.agent.execution.user']:
        result = subprocess.run(['/bin/launchctl', 'print', domain], capture_output=True)
        if result.returncode != 113:
            raise RuntimeError('existing or unqueryable execution registration; refusing replacement')
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    lab = args.output.resolve()
    backend = subprocess.Popen([str(args.backend.resolve())], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=(lab / 'backend.log').open('w'), text=True)
    info = json.loads(backend.stdout.readline())
    def command(kind, **values):
        backend.stdin.write(json.dumps(dict(kind=kind, **values)) + '\n')
        backend.stdin.flush()
        return json.loads(backend.stdout.readline())
    from urllib.parse import urlparse
    target = urlparse(info['origin'])
    class Proxy(http.server.BaseHTTPRequestHandler):
        def forward(self):
            connection = http.client.HTTPConnection(target.hostname, target.port, timeout=10)
            try:
                connection.request(self.command, self.path,
                                   self.rfile.read(int(self.headers.get('Content-Length', '0'))),
                                   {k: v for k, v in self.headers.items() if k.lower() not in ('host', 'connection')})
                reply = connection.getresponse()
                body = reply.read()
                self.send_response(reply.status)
                for key, value in reply.getheaders():
                    if key.lower() not in ('connection', 'transfer-encoding', 'content-length'):
                        self.send_header(key, value)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            finally:
                connection.close()
        do_POST = forward
        do_GET = forward
        def log_message(self, *_):
            pass
    run('/usr/bin/openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
        '-subj', '/CN=localhost', '-keyout', str(lab / 'tls.key'), '-out', str(lab / 'tls.pem'),
        '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1')
    os.chmod(lab / 'tls.key', 0o600)
    proxy = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Proxy)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(lab / 'tls.pem', lab / 'tls.key')
    proxy.socket = context.wrap_socket(proxy.socket, server_side=True)
    threading.Thread(target=proxy.serve_forever, daemon=True).start()
    tag = uuid.uuid4().hex
    protected = Path('/Library/Application Support/RSS MDM Agent') / ('verification-' + tag)
    binary, config = protected / 'rss-execution-service', protected / 'execution.json'
    helper_root = lab / 'helper'
    helper_root.mkdir(mode=0o700)
    installer = Path(__file__).with_name('execution-macos.py').resolve()
    deployment = dict(version=1, origin=f'https://localhost:{proxy.server_port}/', tenant=info['tenant'],
                      signing_keys={'test': info['key']}, ca_file=str(protected / 'ca.pem'),
                      enrollment=str(uuid.uuid4()), registration_operation=str(uuid.uuid4()),
                      state_root=str(protected / 'state'),
                      helper_work_roots={str(os.geteuid()): str(helper_root)},
                      execution=dict(work_root=str(protected / 'work'), material_root=str(protected / 'materials'),
                                     interpreters=[], managers=[], processes=8))
    setup = lab / 'install.py'
    setup.write_text('''import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
root=Path(%r)
root.parent.mkdir(mode=0o755,parents=True,exist_ok=True)
root.mkdir(mode=0o755)
binary=root/'rss-execution-service'
shutil.copyfile(%r,binary); os.chmod(binary,0o755)
subprocess.run(['/usr/bin/codesign','--force','--sign','-','--options','runtime',str(binary)],check=True)
def artifact(path):
    identity=subprocess.run(['/usr/bin/codesign','-d','--verbose=4',str(path)],capture_output=True,text=True,check=True).stderr
    return dict(path=str(path),sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest(),cdhash=re.search(r'CDHash=([0-9a-f]+)',identity)[1])
config=%r
config['service']=artifact(binary)
config['clients']=dict(images=[config['service']],subjects=[%r],interactive=True)
config['execution']['interpreters']=[dict(profile='posix_sh',image=artifact('/bin/sh'))]
config['execution']['managers']=[dict(executor='package_installer',image=artifact('/usr/sbin/installer'))]
shutil.copyfile(%r,root/'ca.pem'); os.chmod(root/'ca.pem',0o644)
path=root/'execution.json';path.write_text(json.dumps(config));os.chmod(path,0o644)
subprocess.run([str(binary),'--config',str(path),'--initialize'],input='AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',text=True,check=True)
subprocess.run(['/usr/bin/python3',%r,'install','--scope','system','--binary',str(binary),'--config',str(path)],check=True)
''' % (str(protected), str(args.binary.resolve()), deployment, str(os.geteuid()), str(lab / 'tls.pem'), str(installer)))
    cleanup = lab / 'remove.py'
    cleanup.write_text('import subprocess\nsubprocess.run(%r,check=True)\n' %
                       ['/usr/bin/python3', str(installer), 'remove', '--scope', 'system', '--binary', str(binary), '--config', str(config)])
    receipt = dict(platform=os.uname().sysname, architecture=os.uname().machine,
                   journal=str(protected / 'state/execution.sqlite'), scenarios={})
    installed = helper = False
    try:
        first = command('script', body='printf \'{"fixture":"system"}\\n\'\n')
        command('result_failure')
        administrator(setup)
        installed = True
        receipt['artifact'] = json.loads(config.read_text())['service']
        receipt['sourceHead'] = run('/usr/bin/git', 'rev-parse', 'HEAD').stdout.strip()
        run('/usr/bin/python3', str(installer), 'install', '--scope', 'user', '--binary', str(binary), '--config', str(config))
        helper = True
        def query():
            reply = json.loads(run(str(binary), '--config', str(config), '--query').stdout)
            assert reply['kind'] == 'tasks' and isinstance(reply['value']['items'], list)
            return reply
        def completed(task, seconds=30):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                status = command('status')
                if acknowledged_result(status, task) is not None:
                    return status
                time.sleep(.2)
            raise RuntimeError('backend result deadline exceeded')
        receipt['scenarios']['authenticated_ipc'] = query()
        system = completed(first['attempt'])
        validate_script(acknowledged_result(system, first['attempt']), 'system')
        assert system['resultCalls'] >= 2, 'the injected 503 must be followed by an acknowledged retry'
        receipt['scenarios']['system_script'] = system
        second = command('script', user=True, body='printf \'{"fixture":"user"}\\n\'\n')
        user = completed(second['attempt'])
        validate_script(acknowledged_result(user, second['attempt']), 'user')
        receipt['scenarios']['user_script'] = user
        payload = lab / 'payload'; payload.mkdir()
        (payload / 'fixed.txt').write_text('controlled package payload\n')
        pkg = lab / 'fixed.pkg'
        package_receipt = 'org.rss.verification.' + tag
        run('/usr/bin/pkgbuild', '--root', str(payload), '--identifier', package_receipt,
            '--version', '1.0', '--install-location', str(protected / 'package-payload'), str(pkg))
        third = command('package', path=str(pkg), receipt=package_receipt)
        package = completed(third['attempt'], 60)
        event = acknowledged_result(package, third['attempt'])
        assert event['kind'] == 'software_result' and event['installerExitCode'] == 0
        assert event['diagnostics']['failure'] is None
        assert (protected / 'package-payload/fixed.txt').read_text() == 'controlled package payload\n'
        import plistlib
        installed_receipt = plistlib.loads(run('/usr/sbin/pkgutil', '--pkg-info-plist', package_receipt).stdout.encode())
        assert installed_receipt['pkg-version'] == '1.0'
        # macOS process groups cannot establish global quiescence. Keep Unknown while
        # independently requiring the actual installer exit, receipt and payload facts.
        assert event['detection'] in ('present', 'unknown')
        receipt['scenarios']['package_exit_and_independent_effect'] = package
        final = query()
        for attempt in [first['attempt'], second['attempt'], third['attempt']]:
            records = [r for r in final['value']['items'] if r['action']['initiator'].get('attempt') == attempt]
            assert len(records) == 1 and records[0]['status']['attempts'] == 1
        receipt['scenarios']['final_ipc'] = final
        marker = protected / 'cancel-started'
        cancel_tail = protected / 'cancel-tail'
        fourth = command('script', body=f"umask 022; printf x > {shlex.quote(str(marker))}; printf '{{\"fixture\":\"cancel\"}}\\n'; /bin/sleep 20; printf x > {shlex.quote(str(cancel_tail))}\n")
        deadline = time.monotonic() + 15
        while not marker.exists():
            assert time.monotonic() < deadline, 'cancellation fixture did not start'
            time.sleep(.2)
        cancel_started = time.monotonic()
        command('cancel')
        deadline = time.monotonic() + 15
        while True:
            current = query()
            records = [r for r in current['value']['items'] if r['action']['initiator'].get('attempt') == fourth['attempt']]
            if records and records[0]['status']['cancelRequested']:
                assert records[0]['status']['attempts'] == 1
                receipt['scenarios']['cancel'] = records[0]
                break
            assert time.monotonic() < deadline, 'cancellation was not recorded'
            time.sleep(.2)
        receipt['scenarios']['cancel_delivery'] = completed(fourth['attempt'], 30)
        cancelled = [r for r in query()['value']['items'] if r['action']['initiator'].get('attempt') == fourth['attempt']][0]
        assert cancelled['status']['process']['finished'] is True
        assert cancelled['status']['process']['end'] == 'cancelled'
        time.sleep(max(0, cancel_started + 21 - time.monotonic()))
        assert not cancel_tail.exists(), 'the cancelled shell reached its tail effect'
        receipt['scenarios']['cancel'] = cancelled
        # Kill the sole service while its original process remains active, then reopen exactly
        # the same journal. The marker must not be appended a second time after recovery.
        counter = protected / 'restart-count'
        process_file = protected / 'restart-pid'
        finish = protected / 'restart-finish'
        fifth = command('script', timeout=120, body=f"umask 022; printf '%s' \"$$\" > {shlex.quote(str(process_file))}; printf x >> {shlex.quote(str(counter))}; printf '{{\"fixture\":\"restart\"}}\\n'; i=0; while [ ! -e {shlex.quote(str(finish))} ] && [ \"$i\" -lt 90 ]; do /bin/sleep 1; i=$((i+1)); done\n")
        deadline = time.monotonic() + 15
        while not counter.exists():
            assert time.monotonic() < deadline, 'restart fixture did not start'
            time.sleep(.2)
        restart = lab / 'restart.py'
        pid = int(process_file.read_text())
        original_process = run('/bin/ps', '-p', str(pid), '-o', 'lstart=,uid=,comm=').stdout.strip()
        assert original_process
        deadline_utc = time.time() + 45
        restart.write_text('import os,plistlib,subprocess,time\nfrom pathlib import Path\np=plistlib.loads(Path("/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist").read_bytes())\nassert p["ProgramArguments"]==%r\nassert time.time()<%r, "authorization exceeded the active fixture window"\nactual=subprocess.run(["/bin/ps","-p",%r,"-o","lstart=,uid=,comm="],capture_output=True,text=True,check=True).stdout.strip()\nassert actual==%r, "original process is no longer active"\nos.kill(%r,0)\nsubprocess.run(["/bin/launchctl","kickstart","-k","system/com.rss-mdm.agent.execution"],check=True)\n' % ([str(binary), '--config', str(config)], deadline_utc, str(pid), original_process, pid))
        # Release only this controlled fixture during cleanup; no restored PID is killed.
        cleanup.write_text('from pathlib import Path\nPath(%r).write_text("finished")\n' % str(finish) + cleanup.read_text())
        administrator(restart)
        time.sleep(3)
        reopened = query()
        records = [r for r in reopened['value']['items'] if r['action']['initiator'].get('attempt') == fifth['attempt']]
        assert len(records) == 1 and records[0]['status']['attempts'] == 1
        assert counter.read_text() == 'x'
        assert records[0]['status']['phase'] == 'outcomeUnknown'
        receipt['scenarios']['restart_no_redispatch'] = records[0]

        receipt['status'] = 'passed'
    except BaseException as error:
        receipt['status'] = 'failed'
        receipt['error'] = str(error)
        if isinstance(error, subprocess.CalledProcessError):
            (lab / 'command-error.txt').write_text((error.stdout or '') + (error.stderr or ''))
        raise
    finally:
        cleanup_errors = []
        if helper:
            try:
                run('/usr/bin/python3', str(installer), 'remove', '--scope', 'user', '--binary', str(binary), '--config', str(config))
            except BaseException as error:
                cleanup_errors.append(str(error))
        if installed:
            try:
                administrator(cleanup)
            except BaseException as error:
                cleanup_errors.append(str(error))
        proxy.shutdown()
        backend.stdin.close()
        try:
            backend.wait(timeout=10)
        except BaseException as error:
            cleanup_errors.append(str(error))
        if cleanup_errors:
            receipt['status'] = 'failed'
            receipt['cleanupErrors'] = cleanup_errors
        (lab / 'receipt.json').write_text(json.dumps(receipt, indent=2))
        if cleanup_errors:
            raise RuntimeError('acceptance cleanup incomplete')



if __name__ == '__main__':
    main()
