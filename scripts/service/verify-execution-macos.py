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
        run('/usr/bin/python3', str(installer), 'install', '--scope', 'user', '--binary', str(binary), '--config', str(config))
        helper = True
        def query():
            return json.loads(run(str(binary), '--config', str(config), '--query').stdout)
        def completed(task, seconds=30):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                status = command('status')
                if any(item.get('attemptId') == task for item in status['results'].values()):
                    return status
                time.sleep(.2)
            raise RuntimeError('backend result deadline exceeded')
        receipt['scenarios']['authenticated_ipc'] = query()
        receipt['scenarios']['system_script'] = completed(first['attempt'])
        second = command('script', user=True, body='printf \'{"fixture":"user"}\\n\'\n')
        receipt['scenarios']['user_script'] = completed(second['attempt'])
        payload = lab / 'payload'; payload.mkdir()
        (payload / 'fixed.txt').write_text('controlled package payload\n')
        pkg = lab / 'fixed.pkg'
        package_receipt = 'org.rss.verification.' + tag
        run('/usr/bin/pkgbuild', '--root', str(payload), '--identifier', package_receipt,
            '--version', '1.0', '--install-location', str(protected / 'package-payload'), str(pkg))
        third = command('package', path=str(pkg), receipt=package_receipt)
        receipt['scenarios']['package'] = completed(third['attempt'], 60)
        receipt['scenarios']['final_ipc'] = query()
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt['status'] = 'failed'
        receipt['error'] = str(error)
        raise
    finally:
        (lab / 'receipt.json').write_text(json.dumps(receipt, indent=2))
        if helper:
            run('/usr/bin/python3', str(installer), 'remove', '--scope', 'user', '--binary', str(binary), '--config', str(config))
        if installed:
            administrator(cleanup)
        proxy.shutdown()
        backend.stdin.close()
        backend.wait(timeout=10)


if __name__ == '__main__':
    main()
