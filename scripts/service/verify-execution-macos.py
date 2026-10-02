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
import sys
import shutil
import tempfile
import sqlite3
import select
import inspect
import socket
import ctypes
import struct
import io
import contextlib
import stat


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)


def copy_candidate(source, destination, expected):
    data = Path(source).read_bytes()
    assert hashlib.sha256(data).hexdigest() == expected, 'source candidate changed'
    Path(destination).write_bytes(data)
    os.chmod(destination, 0o755)


def frozen_installer(source, arguments):
    return ['/usr/bin/python3', '-I', '-c', source, *arguments]


class AuthorizationCancelled(RuntimeError):
    pass


def read_authorization_password(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(descriptor, 'rb') as stream:
        metadata = os.fstat(stream.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
            raise RuntimeError('authorization password file must be private and owned by the login user')
        data = stream.read(4097)
    if len(data)>4096:
        raise RuntimeError('authorization password file exceeds its bound')
    value = data.decode('utf-8').rstrip('\r\n')
    if not value or '\r' in value or '\n' in value or '\0' in value:
        raise RuntimeError('authorization password file is empty or invalid')
    return value


def peer_identity(connection):
    # ref: macOS SDK sys/un.h LOCAL_PEERPID; getpeereid(3), actual kernel credentials.
    uid, gid = ctypes.c_uint(), ctypes.c_uint()
    library = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    if library.getpeereid(connection.fileno(), ctypes.byref(uid), ctypes.byref(gid)) != 0:
        raise RuntimeError('administrator channel peer unavailable')
    pid = struct.unpack('i', connection.getsockopt(0, 2, 4))[0]
    return pid, uid.value


def authorized_steps(programs, endpoint, expected_pid, expected_uid, deadline):
    # This test owner has only frozen setup/initialize/restart/cleanup operations.
    connection = socket.socket(socket.AF_UNIX)
    cleaned = False
    started = False
    output = io.StringIO()
    def perform(operation):
        nonlocal output
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            exec(compile(programs[operation], '<fixed acceptance operation>', 'exec'), {'__name__':'__main__'})
        raw = output.getvalue().strip()
        return json.loads(raw) if raw else None
    def reply(identity, operation):
        nonlocal cleaned
        try:
            if operation == 'cleanup': cleaned = True
            value = perform(operation)
            response = {'id':identity, 'ok':True, 'value':value}
        except BaseException as error:
            raw = output.getvalue().strip()
            diagnostic = json.loads(raw) if raw.startswith('{') else None
            response = {'id':identity, 'ok':False, 'error':type(error).__name__, 'value':diagnostic}
        connection.sendall((json.dumps(response)+'\n').encode())
        return response['ok']
    try:
        connection.connect(endpoint)
        if peer_identity(connection) != (expected_pid, expected_uid):
            raise RuntimeError('administrator channel owner mismatch')
        connection.settimeout(60)
        with connection.makefile('rb') as reader:
            begin = reader.readline(4097)
            if time.clock_gettime(time.CLOCK_MONOTONIC) >= deadline or len(begin)>4096 or not begin or json.loads(begin) != {'id':0,'operation':'setup'}:
                return
            started = True
            connection.settimeout(1200)
            if not reply(0, 'setup'): return
            sequence = 0
            while True:
                line = reader.readline(4097)
                if not line: break
                message = json.loads(line)
                if len(line)>4096 or set(message)!={'id','operation'} or type(message['id']) is not int or message['id']!=sequence+1 or message['operation'] not in ('initialize','restart','cleanup'):
                    connection.sendall((json.dumps({'id':message.get('id'),'ok':False,'error':'invalid operation'})+'\n').encode())
                    break
                sequence = message['id']
                reply(sequence, message['operation'])
                if cleaned: break
    finally:
        if started and not cleaned:
            try: perform('cleanup')
            except BaseException:
                raise RuntimeError('fixed administrator cleanup incomplete') from None
        connection.close()


class AdministratorSession:
    def __init__(self, setup, initialize, restart, cleanup, lab, password_file=None):
        password = read_authorization_password(password_file) if password_file else None
        self.directory = Path(tempfile.mkdtemp(prefix='rss-admin-', dir='/private/tmp'))
        self.endpoint = self.directory/'control'
        self.listener = socket.socket(socket.AF_UNIX)
        self.listener.bind(str(self.endpoint)); self.listener.listen(1); self.listener.settimeout(1)
        self.connection = self.reader = self.process = None
        self.sequence = 0
        self.lab = lab
        # System Python 3.9's macOS monotonic() has a per-process epoch.
        self.deadline = time.clock_gettime(time.CLOCK_MONOTONIC)+120
        programs = {'setup':setup.read_text(),'initialize':initialize.read_text(),'restart':restart.read_text(),'cleanup':cleanup.read_text()}
        source = 'import socket,ctypes,struct,io,contextlib,json,time\n' + inspect.getsource(peer_identity) + inspect.getsource(authorized_steps)
        source += 'authorized_steps('+repr(programs)+','+repr(str(self.endpoint))+','+str(os.getpid())+','+str(os.geteuid())+','+repr(self.deadline)+')\n'
        command = 'cd /private/tmp && /usr/bin/python3 -I -c ' + shlex.quote(source)
        self.log = (lab/'administrator-session.log').open('w')
        script = 'do shell script '+json.dumps(command)
        if password is not None:
            script += ' password '+json.dumps(password,ensure_ascii=False)
        script += ' with administrator privileges\n'
        # ref: AppleScript Language Guide, do shell script password parameter. Source travels
        # over stdin; neither the password nor the frozen operations enter process arguments.
        self.process = subprocess.Popen(['/usr/bin/osascript'],stdin=subprocess.PIPE,stdout=self.log,stderr=self.log)
        self.process.stdin.write(script.encode('utf-8'));self.process.stdin.close()
    def start(self):
        while self.connection is None:
            try: self.connection,_ = self.listener.accept()
            except socket.timeout:
                if self.process.poll() is not None:
                    self.log.flush()
                    if '(-128)' in (self.lab/'administrator-session.log').read_text():
                        raise AuthorizationCancelled('native administrator authorization cancelled')
                    raise RuntimeError('native administrator authorization failed; see administrator-session.log')
                if time.clock_gettime(time.CLOCK_MONOTONIC) >= self.deadline:
                    self.process.terminate()
                    raise RuntimeError('native administrator authorization deadline exceeded')
        if time.clock_gettime(time.CLOCK_MONOTONIC) >= self.deadline:
            self.connection.close();self.connection=None
            raise RuntimeError('native administrator authorization deadline exceeded')
        if peer_identity(self.connection)[1] != 0:
            self.connection.close();self.connection=None
            raise RuntimeError('administrator channel is not system-owned')
        self.connection.settimeout(60);self.reader=self.connection.makefile('rb')
        self.connection.sendall(b'{"id":0,"operation":"setup"}\n')
        return self.receive(0)
    def receive(self, identity):
        message=json.loads(self.reader.readline(8*1024*1024+1))
        if message.get('id')!=identity or message.get('ok') is not True:
            if message.get('value'):
                (self.lab/'administrator-operation-diagnostic.json').write_text(json.dumps(message['value'],indent=2))
            raise RuntimeError('fixed administrator operation failed: '+str(message.get('error','invalid acknowledgement')))
        return message['value']
    def command(self, operation):
        self.sequence+=1
        self.connection.sendall((json.dumps({'id':self.sequence,'operation':operation})+'\n').encode())
        return self.receive(self.sequence)
    def close(self):
        if self.reader:self.reader.close()
        if self.connection:self.connection.close()
        self.listener.close()
        try:self.process.wait(timeout=15)
        except subprocess.TimeoutExpired:raise RuntimeError('administrator cleanup process exit unconfirmed')
        finally:
            self.log.close(); self.endpoint.unlink(missing_ok=True); self.directory.rmdir()


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


def validate_desktop_completion(records, backend, effect, request):
    matches = [row for row in records if row['status']['operationRequestId'] == request]
    assert len(matches) == 1, 'original request missing or duplicated'
    record = matches[0]
    assert record['status']['attempts'] == 1 and backend['startRequests'] == 1, 'original attempt redispatched'
    process = record['status']['process']
    assert process and process['finished'] is True, 'native process has not finished'
    initiator = record['action']['initiator']
    assert initiator['kind'] == 'backend', 'unexpected execution owner'
    event = acknowledged_result(backend, initiator['attempt'])
    assert event is not None, 'original result not acknowledged'
    if event['kind'] == 'cancelled':
        assert process['end'] == 'cancelled', 'cancellation lacks process termination'
        cancelled = True
    else:
        assert event['kind'] == 'software_result' and len(event['steps']) == 1
        step = event['steps'][0]
        cancelled = step['process'] == {'kind':'failed','failure':'cancelled'}
        if cancelled:
            assert process['end'] == 'cancelled' and step['diagnostics']['failure'] == 'cancelled'
        else:
            assert step['process'] == {'kind':'exited','code':0}, 'installer failed or did not run'
            assert process['end'] == 'exited' and process['exitCode'] == 0
            assert step['diagnostics']['failure'] is None
            assert step['after']['kind'] in ('present','unknown')
    present = all(effect[key] is True for key in ('receiptPresent','payloadPresent','payloadMatches'))
    absent = all(effect[key] is False for key in ('receiptPresent','payloadPresent','payloadMatches'))
    assert present or (cancelled and absent), 'independent device effect conflicts with terminal result'
    return {'request':request, 'record':record, 'backend':backend, 'effect':effect}


def journal_proof(path):
    db=sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True)
    try:
        captures = {row[0]:json.loads(row[1]) for row in db.execute('SELECT attempt_id,body FROM process_evidence')}
        rows=[]
        for request, encoded in db.execute('SELECT request_id,snapshot FROM executions'):
            snapshot=json.loads(encoded)
            capture=captures.get((snapshot.get('attempt') or {}).get('id'))
            process=None if capture is None else {key:capture[key] for key in ('attemptId','finished','end','exitCode','contentDigest')}
            rows.append({'request':request,'snapshot':snapshot,'process':process})
        return {'journal':str(path),'records':rows}
    finally: db.close()


def validate_journal_completion(proof, completion):
    rows=[row for row in proof['records'] if row['request']==completion['request']]
    assert len(rows)==1 and rows[0]['snapshot']['attempts']==1
    process=rows[0]['process']
    assert process and process['finished'], 'final journal lacks terminal process proof'
    assert process['attemptId']==completion['record']['status']['attemptId'], 'journal attempt changed'
    assert process['end']==completion['record']['status']['process']['end'], 'journal process result changed'
    assert process['exitCode']==completion['record']['status']['process']['exitCode'], 'journal exit result changed'


def validate_desktop_finish(finish, evidence):
    assert finish and finish.get('status') == 'passed', 'parent did not explicitly pass'
    assert evidence and finish.get('request') == evidence['request'], 'completion evidence missing or mismatched'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--desktop', type=Path)
    parser.add_argument('--authorization-password-file', type=Path)
    args = parser.parse_args()
    if os.uname().sysname != 'Darwin' or os.geteuid() == 0:
        raise RuntimeError('run this harness from the actual macOS user login')
    for domain in ['system/com.rss-mdm.agent.execution', f'gui/{os.geteuid()}/com.rss-mdm.agent.execution.user']:
        result = subprocess.run(['/bin/launchctl', 'print', domain], capture_output=True)
        if result.returncode != 113:
            raise RuntimeError('existing or unqueryable execution registration; refusing replacement')
    default_config = Path('/Library/Application Support/RSS MDM Agent/execution.json')
    if args.desktop and (default_config.exists() or default_config.is_symlink()):
        raise RuntimeError('default deployment exists; refusing to replace another installation')
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    lab = args.output.resolve()
    inputs = Path(tempfile.mkdtemp(prefix='rss-native-service-input-')).resolve()
    receipt_inputs = inputs
    # Copy only this fixed candidate; no writable worktree image is trusted by the service.
    for field, leaf in [('binary', 'rss-execution-service'), ('desktop', 'rss-mdm-desktop')]:
        value = getattr(args, field)
        if value is not None:
            destination = inputs / leaf
            shutil.copyfile(value.resolve(), destination); destination.chmod(0o700)
            setattr(args, field, destination)

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
        '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1',
        '-addext', 'extendedKeyUsage=serverAuth',
        '-addext', 'basicConstraints=critical,CA:FALSE',
        '-addext', 'keyUsage=critical,digitalSignature,keyEncipherment')
    os.chmod(lab / 'tls.key', 0o600)
    proxy = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Proxy)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(lab / 'tls.pem', lab / 'tls.key')
    proxy.socket = context.wrap_socket(proxy.socket, server_side=True)
    threading.Thread(target=proxy.serve_forever, daemon=True).start()
    tag = uuid.uuid4().hex
    protected = Path('/Library/Application Support/RSS MDM Agent') / ('verification-' + tag)
    binary, config = protected / 'rss-execution-service', protected / 'execution.json'
    helper_root = inputs / 'helper'
    helper_root.mkdir(mode=0o700)
    installer = inputs / 'execution-macos.py'
    installer_source = Path(__file__).with_name('execution-macos.py').read_text()
    installer.write_text(installer_source)
    installer.chmod(0o600)
    shutil.copyfile(lab / 'tls.pem', inputs / 'tls.pem')
    deployment = dict(version=2, ipc_version=6, origin=f'https://localhost:{proxy.server_port}/', tenant=info['tenant'],
                      signing_keys={'test': info['key']}, ca_file=str(protected / 'ca.pem'),
                      enrollment=str(uuid.uuid4()), registration_operation=str(uuid.uuid4()),
                      state_root=str(protected / 'state'),
                      helper_work_roots={str(os.geteuid()): str(helper_root)},
                      execution=dict(work_root=str(protected / 'work'), material_root=str(protected / 'materials'),
                                     interpreters=[], managers=[], processes=8))
    setup = lab / 'install.py'
    setup.write_text(inspect.getsource(copy_candidate) + '''import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
root=Path(%r)
root.parent.mkdir(mode=0o755,parents=True,exist_ok=True)
root.mkdir(mode=0o755)
binary=root/'rss-execution-service'
copy_candidate(%r,binary,%r)
subprocess.run(['/usr/bin/codesign','--force','--sign','-','--options','runtime',str(binary)],check=True)
def artifact(path):
    identity=subprocess.run(['/usr/bin/codesign','-d','--verbose=4',str(path)],capture_output=True,text=True,check=True).stderr
    return dict(path=str(path),sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest(),cdhash=re.search(r'CDHash=([0-9a-f]+)',identity)[1])
config=%r
config['service']=artifact(binary)
config['clients']=dict(images=[config['service']],subjects=[%r],interactive=True)
config['execution']['interpreters']=[dict(profile='posix_sh',image=artifact('/bin/sh'))]
config['execution']['managers']=[dict(executor='package_installer',image=artifact('/usr/sbin/installer'))]
(root/'ca.pem').write_bytes(%r); os.chmod(root/'ca.pem',0o644)
path=root/'execution.json';path.write_text(json.dumps(config));os.chmod(path,0o644)
subprocess.run([str(binary),'--config',str(path),'--initialize'],input='AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',text=True,check=True)
subprocess.run(%r,check=True)
''' % (str(protected), str(args.binary.resolve()), hashlib.sha256(args.binary.read_bytes()).hexdigest(), deployment, str(os.geteuid()), (lab / 'tls.pem').read_bytes(), frozen_installer(installer_source, ['install','--scope','system','--binary',str(binary),'--config',str(config)])))
    if args.desktop:
        # Extend this same installation owner, retaining its protected binary/config checks.
        contents = setup.read_text()
        insertion = """desktop=root/'rss-mdm-desktop'
copy_candidate(%r,desktop,%r)
subprocess.run(['/usr/bin/codesign','--force','--sign','-','--options','runtime',str(desktop)],check=True)
config['clients']['images'].append(artifact(desktop))
""" % (str(args.desktop.resolve()), hashlib.sha256(args.desktop.read_bytes()).hexdigest())
        contents = contents.replace("path=root/'execution.json'", insertion + "path=root/'execution.json'")
        # Default pin creation is exclusive. The desktop never selects an arbitrary config.
        contents += """default=Path(%r)
fd=os.open(default,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644)
with os.fdopen(fd,'w') as stream: json.dump(config,stream); stream.flush(); os.fsync(stream.fileno())
""" % str(default_config)
        initializer = "subprocess.run([str(binary),'--config',str(path),'--initialize'],input='AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',text=True,check=True)\n"
        contents = contents.replace(initializer, '')
        setup.write_text(contents)
    cleanup = lab / 'remove.py'
    cleanup.write_text(inspect.getsource(journal_proof) + """import json,sqlite3,subprocess
from pathlib import Path
expected=%r
plist=Path('/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist')
if plist.exists(): subprocess.run(expected,check=True)
journal=Path(%r)
if journal.exists(): print(json.dumps(journal_proof(journal)))
""" % (frozen_installer(installer_source, ['remove', '--scope', 'system', '--binary', str(binary), '--config', str(config)]), str(protected / 'state/execution.sqlite')))
    if args.desktop:
        cleanup.write_text(cleanup.read_text() + """default=Path(%r)
if default.exists():
    actual=json.loads(default.read_text())
    assert actual['service']['path']==%r and actual['state_root']==%r, 'refusing to remove another deployment pin'
    default.unlink()
""" % (str(default_config), str(binary), str(protected / 'state')))
    initialize = lab/'initialize.py'
    initialize.write_text("""import json,plistlib,sqlite3,subprocess
from pathlib import Path
p=plistlib.loads(Path('/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist').read_bytes())
assert p['ProgramArguments']==%r, 'initialization registration owner mismatch'
try:
    subprocess.run(%r,input='AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',text=True,check=True,capture_output=True)
except subprocess.CalledProcessError as error:
    root=Path(%r); diagnostic={'phase':'initialize','error':error.stderr.strip(),'entries':[],'databases':{}}
    for leaf in ['','identity-binding','execution-initialized','communication','communication/communication.sqlite','execution.sqlite','secrets']:
        path=root/leaf
        try:
            metadata=path.stat();diagnostic['entries'].append({'leaf':leaf,'uid':metadata.st_uid,'mode':oct(metadata.st_mode & 0o777),'directory':path.is_dir()})
        except FileNotFoundError: pass
    for leaf in ['communication/communication.sqlite','execution.sqlite']:
        path=root/leaf
        if path.exists():
            db=sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True)
            diagnostic['databases'][leaf]={'version':db.execute('PRAGMA user_version').fetchone()[0],'tables':[row[0] for row in db.execute("SELECT name FROM sqlite_schema WHERE type='table'")]}
            if leaf.startswith('communication'): diagnostic['databases'][leaf]['stateKeys']=[row[0] for row in db.execute('SELECT key FROM state')]
            db.close()
    print(json.dumps(diagnostic));raise
subprocess.run(['/bin/launchctl','kickstart','-k','system/com.rss-mdm.agent.execution'],check=True)
""" % ([str(binary),'--config',str(config)],[str(binary),'--config',str(config),'--initialize'],str(protected/'state')))
    restart = lab/'restart.py'
    restart.write_text("""import os,plistlib,subprocess
from pathlib import Path
p=plistlib.loads(Path('/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist').read_bytes())
assert p['ProgramArguments']==%r, 'restart registration owner mismatch'
pid=int(Path(%r).read_text());os.kill(pid,0)
subprocess.run(['/bin/launchctl','kickstart','-k','system/com.rss-mdm.agent.execution'],check=True)
""" % ([str(binary),'--config',str(config)],str(protected/'restart-pid')))
    cleanup.write_text("from pathlib import Path\nif Path(%r).exists(): Path(%r).write_text('finished')\n" % (str(protected/'restart-pid'),str(protected/'restart-finish')) + cleanup.read_text())
    receipt = dict(platform=os.uname().sysname, architecture=os.uname().machine,
                   journal=str(protected / 'state/execution.sqlite'), scenarios={}, sourceHead=run('/usr/bin/git', 'rev-parse', 'HEAD').stdout.strip())
    installed = helper = False
    administrator_session = None
    try:
        first = None if args.desktop else command('script', body='printf \'{"fixture":"system"}\\n\'\n')
        if not args.desktop: command('result_failure')
        administrator_session = AdministratorSession(setup,initialize,restart,cleanup,lab,args.authorization_password_file)
        administrator_session.start()
        installed = True
        if args.desktop and select.select([sys.stdin], [], [], 0)[0]:
            raise RuntimeError('native acceptance parent ended before readiness; refusing initialization')
        receipt['artifact'] = json.loads(config.read_text())['service']
        if args.desktop:
            deadline = time.monotonic() + 15
            while True:
                probe = subprocess.run([str(binary), '--config', str(config), '--service-status'], capture_output=True, text=True)
                if probe.returncode == 0: break
                if time.monotonic() >= deadline: raise RuntimeError('unregistered service diagnostic deadline exceeded')
                time.sleep(.2)
            status = json.loads(probe.stdout)
            assert status['reply']['value']['readiness']['phase'] == 'registrationRequired'
            denied = json.loads(run(str(binary), '--config', str(config), '--query').stdout)
            assert denied['reply']['kind'] == 'rejected'
            assert not (protected / 'state').exists(), 'diagnostic startup created device state'
            receipt['scenarios']['unregistered_status_without_authority'] = status
            administrator_session.command('initialize')

        helper = True
        run('/usr/bin/python3', str(installer), 'install', '--scope', 'user', '--binary', str(binary), '--config', str(config))
        helper = True
        def query():
            reply = json.loads(run(str(binary), '--config', str(config), '--query').stdout)['reply']
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
        if args.desktop:
            payload = lab / 'payload'; payload.mkdir()
            (payload / 'fixed.txt').write_text('controlled package payload\n')
            pkg = lab / 'fixed.pkg'
            package_receipt = 'org.rss.verification.' + tag
            run('/usr/bin/pkgbuild', '--root', str(payload), '--identifier', package_receipt,
                '--version', '1.0', '--install-location', str(protected / 'package-payload'), str(pkg))
            offered = command('package', path=str(pkg), receipt=package_receipt, user=True)
            print(json.dumps({'kind': 'desktopService', 'desktop': str(protected / 'rss-mdm-desktop'),
                'service': receipt['artifact'], 'config': str(default_config), 'task': offered, 'journalOwner': 'agent-service'}), flush=True)
            def effect():
                detected = subprocess.run(['/usr/sbin/pkgutil', '--pkg-info-plist', package_receipt], capture_output=True, text=True)
                target_file = protected / 'package-payload/fixed.txt'
                return {'receiptPresent': detected.returncode == 0, 'payloadPresent': target_file.exists(),
                    'payloadMatches': target_file.exists() and target_file.read_text() == 'controlled package payload\n'}
            finish = completion = None
            for line in sys.stdin:
                request = json.loads(line)
                method = request['method']
                if method == 'query': value = query()
                elif method == 'status': value = command('status')
                elif method == 'effect': value = effect()
                elif method == 'completion':
                    deadline = time.monotonic() + 60
                    while True:
                        current, remote = query(), command('status')
                        rows = [row for row in current['value']['items'] if row['status']['operationRequestId'] == request['request']]
                        if rows and rows[0]['status']['process'] and rows[0]['status']['process']['finished'] and acknowledged_result(remote, rows[0]['action']['initiator']['attempt']) is not None:
                            break
                        assert time.monotonic() < deadline, 'terminal process/result deadline exceeded'
                        time.sleep(.2)
                    completion = validate_desktop_completion(current['value']['items'], remote, effect(), request['request'])
                    value = completion
                elif method == 'finish':
                    finish = request
                    value = {}
                else: raise RuntimeError('unknown native harness control')
                print(json.dumps({'id': request['id'], 'value': value}), flush=True)
                if method == 'finish': break
            validate_desktop_finish(finish, completion)
            receipt['scenarios']['completion'] = completion
            receipt['scenarios']['native_final'] = query()
            receipt['scenarios']['backend_final'] = command('status')
            receipt['status'] = 'passed'
            return
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
        assert event['kind'] == 'software_result' and len(event['steps']) == 1
        step = event['steps'][0]
        assert step['process'] == {'kind':'exited','code':0}
        assert step['diagnostics']['failure'] is None
        assert (protected / 'package-payload/fixed.txt').read_text() == 'controlled package payload\n'
        import plistlib
        installed_receipt = plistlib.loads(run('/usr/sbin/pkgutil', '--pkg-info-plist', package_receipt).stdout.encode())
        assert installed_receipt['pkg-version'] == '1.0'
        # macOS process groups cannot establish global quiescence. Keep Unknown while
        # independently requiring the actual installer exit, receipt and payload facts.
        assert step['after']['kind'] in ('present', 'unknown')
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
        administrator_session.command('restart')
        time.sleep(3)
        reopened = query()
        records = [r for r in reopened['value']['items'] if r['action']['initiator'].get('attempt') == fifth['attempt']]
        assert len(records) == 1 and records[0]['status']['attempts'] == 1
        assert counter.read_text() == 'x'
        assert records[0]['status']['phase'] == 'outcomeUnknown'
        receipt['scenarios']['restart_no_redispatch'] = records[0]

        receipt['status'] = 'passed'
    except BaseException as error:
        receipt['status'] = 'cancelled' if isinstance(error, AuthorizationCancelled) else 'failed'
        receipt['error'] = str(error)
        if isinstance(error, subprocess.CalledProcessError):
            (lab / 'command-error.txt').write_text((error.stdout or '') + (error.stderr or ''))
        raise
    finally:
        if protected.exists(): installed = True
        cleanup_errors = []
        if helper:
            try:
                run('/usr/bin/python3', str(installer), 'remove', '--scope', 'user', '--binary', str(binary), '--config', str(config))
            except BaseException as error:
                cleanup_errors.append(str(error))
        if administrator_session and administrator_session.reader:
            try:
                proof = administrator_session.command('cleanup')
                if proof: receipt['journalProof'] = proof
                if args.desktop and receipt['status'] == 'passed':
                    validate_journal_completion(receipt['journalProof'], completion)
            except BaseException as error:
                cleanup_errors.append(str(error))
        if administrator_session:
            try: administrator_session.close()
            except BaseException as error: cleanup_errors.append(str(error))
        proxy.shutdown()
        backend.stdin.close()
        try:
            backend.wait(timeout=10)
        except BaseException as error:
            cleanup_errors.append(str(error))
        if cleanup_errors:
            receipt['status'] = 'failed'
            receipt['cleanupErrors'] = cleanup_errors
        receipt['installationCreated'] = installed
        receipt['cleanup'] = 'incomplete' if cleanup_errors else 'complete'
        receipt['inputStaging'] = str(receipt_inputs)
        (lab / 'receipt.json').write_text(json.dumps(receipt, indent=2))
        if not cleanup_errors:
            # The original helper work root can hold unresolved recovery evidence.
            for leaf in ['rss-execution-service','rss-mdm-desktop','execution-macos.py','tls.pem']:
                (inputs / leaf).unlink(missing_ok=True)
        if cleanup_errors:
            raise RuntimeError('acceptance cleanup incomplete')



if __name__ == '__main__':
    main()
