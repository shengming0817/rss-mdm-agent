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
import selectors
import signal
import base64

# Only this owner defines the final macOS security matrix. Journey success alone is partial.
SECURITY_SCENARIOS = (
    'authorized_channel', 'wrong_image_same_uid', 'wrong_code_identity', 'cross_user',
    'login_generation', 'helper_wrong_identity', 'helper_stale_endpoint', 'worker_direct_access',
    'fake_server', 'malformed_empty', 'malformed_json', 'malformed_truncated',
    'unknown_version', 'unknown_method', 'unknown_field', 'system_wire_limit', 'native_frame_limit',
    'connection_repeat', 'connection_expiry', 'offer_tamper', 'offer_replay', 'offer_expiry',
    'revocation_preopened', 'revocation_new_connection', 'revocation_active_process',
    'restart_old_connection', 'refresh_preflight', 'refresh_after_stop',
    'desktop_service', 'desktop_codex', 'host_crash', 'launcher_crash', 'retained_descendant',
    'registration_failure', 'close_timeout', 'unknown_scope', 'process_scope_empty',
    'legacy_challenge',
)


def security_matrix():
    rows = {name: dict(status='notExecuted', reason='required native scenario not executed')
            for name in SECURITY_SCENARIOS}
    rows['cross_user']['reason'] = 'second real GUI login required; sudo -u is not evidence'
    rows['login_generation']['reason'] = 'isolated logout/login and retained old request required'
    rows['legacy_challenge'] = dict(status='notApplicable',
        reason='IPC V7 has no challenge; NSXPC one-shot/5s lifetime and backend offer expiry replace it',
        source='crates/execution-runner/src/{host.rs,macos_service.m}')
    return dict(platform='macOS', scenarios=rows, status='notExecuted')


def security_result(matrix, name, evidence):
    if name not in matrix['scenarios'] or not evidence:
        raise RuntimeError('security result requires a known scenario and actual evidence')
    matrix['scenarios'][name] = dict(status='passed', evidence=evidence)


def summarize_security(matrix):
    statuses = [row['status'] for row in matrix['scenarios'].values()]
    if set(statuses)-{'passed','failed','notApplicable','notExecuted'}:
        raise RuntimeError('unknown security scenario status')
    if 'failed' in statuses or matrix.get('failure'): status = 'failed'
    elif 'notExecuted' in statuses: status = 'partial' if 'passed' in statuses else 'notExecuted'
    else: status = 'passed'
    matrix['status'] = status
    return status


class NativeProbe:
    def __init__(self, binary, config, log):
        self.process = subprocess.Popen([str(binary), '--config', str(config)], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=log, text=True)
        self.exchange = BackendExchange(self.process, timeout=6)
        try:
            self.identity = self.exchange.read(time.monotonic() + 6)
            if self.identity.get('ready') is not True: raise RuntimeError('native probe not ready')
        except BaseException:
            self.exchange.close()
            raise
        self.sequence = 0
        self.open_evidence = {}

    def open(self, **values):
        self.sequence += 1
        connection = str(self.sequence)
        result = self.exchange.command('open', connection=connection, **values)
        if result.get('created') is not True: raise RuntimeError('native connection creation failed')
        if values.get('establish') and (result.get('established') is not True or result.get('peerUid')!=0 or result.get('peerPid',0)<=1):
            raise RuntimeError('native connection establishment not proven')
        self.open_evidence[connection]=result
        return connection

    def send(self, connection, payload=b'', **values):
        reply = self.exchange.command('send', connection=connection,
            payload=base64.b64encode(payload).decode(), **values)
        if 'error' in reply: raise RuntimeError('native probe control failed')
        reply['establishment']=self.open_evidence[connection]
        actual=payload
        if 'length' in values:
            length=values['length'];actual=payload[:length]+b'\0'*max(0,length-len(payload))
        reply['payloadSha256'] = hashlib.sha256(actual).hexdigest()
        if reply.get('transport') == 'reply' and reply.get('bytes'):
            raw = base64.b64decode(reply['replyBase64'], validate=True)
            try: reply['envelope'] = json.loads(raw)
            except (ValueError, UnicodeError): pass
        return reply

    def close_connection(self, connection):
        self.exchange.command('close', connection=connection)
        self.open_evidence.pop(connection,None)

    def request(self, request):
        # Typed baseline is emitted by the production wire owner. Mutations are explicit attacks.
        envelope = dict(self.identity['baseline'], request=request)
        connection = self.open()
        try: return self.send(connection, json.dumps(envelope).encode())
        finally: self.close_connection(connection)

    def close(self):
        self.exchange.close()


def assert_native_reply(result, kind):
    assert result.get('transport') == 'reply' and result.get('peerUid') == 0, 'authenticated native reply required'
    assert result.get('envelope', {}).get('reply', {}).get('kind') == kind, 'unexpected native reply'


def assert_connection_closed(result):
    # A timeout alone does not prove refusal. The OS error or explicit empty reply is required.
    assert result.get('transport') == 'error' or (
        result.get('transport') == 'reply' and result.get('bytes') == 0) or (
        result.get('transport')=='timeout' and (result.get('invalidated') or result.get('interrupted'))), 'connection closure not proven: '+json.dumps(result)


def native_transport_security(probe, matrix, query, command, administrator):
    baseline = probe.request(dict(method='serviceStatus'))
    assert_native_reply(baseline, 'serviceStatus')
    security_result(matrix, 'authorized_channel', dict(identity=probe.identity, response=baseline))
    before = query()
    backend_before = command('status')['startRequests']
    attacks = dict(
        malformed_empty=b'', malformed_json=b'not-json', malformed_truncated=b'{"version":',
        unknown_version=json.dumps(dict(probe.identity['baseline'], version=255)).encode(),
        unknown_method=json.dumps(dict(probe.identity['baseline'], request=dict(method='unknown'))).encode(),
        unknown_field=json.dumps(dict(probe.identity['baseline'], unknown=True)).encode(),
    )
    for name, payload in attacks.items():
        connection = probe.open()
        try:
            response = probe.send(connection, payload)
            matrix['scenarios'][name]=dict(status='failed',evidence=response,reason='native boundary assertion pending')
            if name=='malformed_empty':
                assert_connection_closed(response)
                response['boundary']='native FFI null/empty NSData guard; no business dispatch'
            else: assert_native_reply(response, 'rejected')
            security_result(matrix, name, response)
        finally: probe.close_connection(connection)
    for name, length in [('system_wire_limit', 65537), ('native_frame_limit', 8*1024*1024+1)]:
        connection = probe.open()
        try:
            response = probe.send(connection, b' ', length=length)
            if name == 'system_wire_limit': assert_native_reply(response, 'rejected')
            else: assert_connection_closed(response)
            security_result(matrix, name, response)
        finally: probe.close_connection(connection)
    payload = json.dumps(probe.identity['baseline']).encode()
    connection = probe.open(establish=True)
    try:
        first = probe.send(connection, payload); assert_native_reply(first, 'serviceStatus')
        second = probe.send(connection, payload); assert_connection_closed(second)
        security_result(matrix, 'connection_repeat', dict(first=first, second=second))
    finally: probe.close_connection(connection)
    log_before=administrator.command('diagnostics')['log']
    connection = probe.open(establish=True)
    try:
        log_open=administrator.command('diagnostics')['log']
        opened=[int(value) for value in re.findall(r'RSS_IPC_OPEN id=(\d+) clientPid='+str(probe.process.pid)+r'\b',log_open[len(log_before):])]
        assert len(opened)==1, 'one actual server connection required'
        serial=opened[0]
        time.sleep(5.3)
        expiry_deadline=time.monotonic()+3
        while True:
            log_expired=administrator.command('diagnostics')['log']
            if 'RSS_IPC_EXPIRED id='+str(serial)+' clientPid='+str(probe.process.pid) in log_expired: break
            assert time.monotonic()<expiry_deadline, 'server expiry not observed'
            time.sleep(.05)
        response = probe.send(connection, payload)
        matrix['scenarios']['connection_expiry']=dict(status='failed',evidence=response,reason='native lifetime assertion pending')
        if response.get('transport')=='reply' and response.get('bytes'):
            assert_native_reply(response,'serviceStatus')
            log_after=administrator.command('diagnostics')['log']
            executed=[int(value) for value in re.findall(r'RSS_IPC_EXECUTE id=(\d+) clientPid='+str(probe.process.pid)+r'\b',log_after[len(log_expired):])]
            assert executed and executed[-1]!=serial, 'expired server connection executed again'
            response['boundary']='old transport interrupted; NSXPC reauthenticated a replacement transport under the unchanged pins'
        else: assert_connection_closed(response)
        response['serverExpiredConnection']=serial
        assert response['ageMs'] >= 5000
        security_result(matrix, 'connection_expiry', response)
    finally: probe.close_connection(connection)
    for stale, name in [(False, 'helper_wrong_identity')]:
        connection = probe.open()
        try:
            response = probe.exchange.command('registerHelper', connection=connection, stale=stale)
            assert response.get('transport') == 'reply' and response.get('accepted') is False
            security_result(matrix, name, response)
        finally: probe.close_connection(connection)
    # First prove the fake endpoint is actually reachable. The system lookup target is never unpinned.
    connection = probe.open(target='fake', untrusted=True)
    try:
        fake = probe.send(connection, payload)
        assert fake.get('peerUid') == 0 and fake.get('envelope') == dict(fake=True)
    finally: probe.close_connection(connection)
    connection = probe.open(target='fake')
    try:
        rejected = probe.send(connection, payload); assert_connection_closed(rejected)
        security_result(matrix, 'fake_server', dict(reachable=fake, trustedPolicy=rejected))
    finally: probe.close_connection(connection)
    after = query()
    assert after == before and command('status')['startRequests'] == backend_before, 'attack changed service facts'
    healthy = probe.request(dict(method='serviceStatus')); assert_native_reply(healthy, 'serviceStatus')
    matrix['attackBoundaryProof'] = dict(before=before, after=after, startRequests=backend_before, healthy=healthy)


def offer_request(offer):
    return dict(method='startTask', request=offer['request'], task=offer['task'],
        attempt=offer['attempt'], revision=offer['revision'], origin=dict(kind='desktop'))


def await_offer(query, task):
    deadline = time.monotonic() + 15
    while True:
        available = [offer for offer in query()['available'] if offer['task'] == task]
        if available: return available[0]
        assert time.monotonic() < deadline, 'offer did not reach the authenticated service'
        time.sleep(.05)


def native_offer_security(probe, matrix, query, command, package, receipt, completed, effect):
    prior = query()['available']
    if prior:
        command('cancel')
        deadline=time.monotonic()+15
        while query()['available']:
            assert time.monotonic()<deadline, 'previous unselected catalog offer did not withdraw'
            time.sleep(.1)
    task = command('package', path=str(package), receipt=receipt, user=True)
    offer = await_offer(query, task['task'])
    request = offer_request(offer)
    baseline = command('status')['startRequests']
    for field, replacement in [('request', 'foreign-request'), ('attempt', str(uuid.uuid4())), ('revision', 'a'*64)]:
        changed = dict(request, **{field: replacement})
        response = probe.request(changed); assert_native_reply(response, 'rejected')
    assert command('status')['startRequests'] == baseline
    security_result(matrix, 'offer_tamper', dict(offer=offer, startRequests=baseline))
    # Exact replay before dispatch is consumed is admitted idempotently; observe both native replies.
    first = probe.request(request); assert_native_reply(first, 'queued')
    second = probe.request(request)
    assert second.get('transport')=='reply' and second.get('peerUid')==0
    assert second.get('envelope',{}).get('reply',{}).get('kind') in ('queued','pending','status')
    remote = completed(task['attempt'], 90)
    matching = [r for r in query()['value']['items'] if r['action']['initiator'].get('attempt') == task['attempt']]
    assert len(matching) == 1 and matching[0]['status']['attempts'] == 1
    assert remote['startRequests'] == baseline+1
    outcome = effect()
    assert outcome['receiptPresent'] and outcome['payloadMatches']
    security_result(matrix, 'offer_replay', dict(first=first, second=second, record=matching[0],
        backend=remote, effect=outcome))
    expiring = command('package', path=str(package), receipt=receipt, user=True, validitySeconds=6)
    offer = await_offer(query, expiring['task']); request = offer_request(offer)
    starts = command('status')['startRequests']
    time.sleep(max(0, offer['expiresAt'] + .2 - time.time()))
    # Expired offers may already have been removed; both closed business outcomes deny admission.
    replies = [probe.request(request) for _ in range(2)]
    for response in replies:
        assert response.get('envelope', {}).get('reply', {}).get('kind') in ('rejected', 'unavailable')
        assert response.get('transport') == 'reply' and response.get('peerUid') == 0
    assert command('status')['startRequests'] == starts
    assert not any(r['action']['initiator'].get('attempt') == expiring['attempt'] for r in query()['value']['items'])
    security_result(matrix, 'offer_expiry', dict(offer=offer, replies=replies, startRequests=starts))


def native_revocation_security(probe, matrix, query, command, package, receipt, protected):
    marker, tail = protected/'revoke-started', protected/'revoke-tail'
    active = command('script', timeout=60,
        body="umask 022; printf '%s' \"$$\" > "+shlex.quote(str(marker))+
            "; /bin/sleep 25; printf x > "+shlex.quote(str(tail))+"\n")
    deadline = time.monotonic()+15
    while not marker.exists():
        assert time.monotonic()<deadline, 'revocation process did not start'
        time.sleep(.05)
    task = command('package', path=str(package), receipt=receipt, user=True)
    offer = await_offer(query, task['task']); request = offer_request(offer)
    starts = command('status')['startRequests']
    connection = probe.open(establish=True)
    started = time.monotonic()
    command('revoke')
    while True:
        status = probe.request(dict(method='serviceStatus'))
        assert_native_reply(status, 'serviceStatus')
        if status['envelope']['reply']['value']['readiness']['phase'] == 'notReady': break
        assert time.monotonic()-started<4, 'revocation not observed inside the live connection window'
        time.sleep(.05)
    try:
        response = probe.send(connection, json.dumps(dict(probe.identity['baseline'],request=request)).encode())
        assert response['ageMs']<5000, 'connection expired before revocation assertion'
        assert_native_reply(response, 'rejected')
        security_result(matrix,'revocation_preopened',dict(response=response,readiness=status))
    finally: probe.close_connection(connection)
    response = probe.request(request); assert_native_reply(response, 'rejected')
    security_result(matrix,'revocation_new_connection',response)
    assert command('status')['startRequests']==starts, 'revoked caller created another attempt'
    deadline=time.monotonic()+15
    while True:
        records=[row for row in query()['value']['items'] if row['action']['initiator'].get('attempt')==active['attempt']]
        if records and records[0]['status']['process'] and records[0]['status']['process']['finished']: break
        assert time.monotonic()<deadline, 'revoked physical process termination not recorded'
        time.sleep(.1)
    pid=int(marker.read_text())
    try: os.kill(pid,0)
    except ProcessLookupError: pass
    else: raise AssertionError('revoked shell is still alive')
    time.sleep(max(0,started+26-time.monotonic()))
    assert not tail.exists(), 'revoked process reached its tail'
    security_result(matrix,'revocation_active_process',dict(record=records[0],pidAbsent=pid,tailAbsent=True))


def native_restart_security(probe,matrix,command,administrator):
    connection=probe.open(establish=True)
    starts=command('status')['startRequests']
    administrator.command('restart')
    try:
        response=probe.send(connection,json.dumps(probe.identity['baseline']).encode())
        if response.get('transport')=='reply' and response.get('bytes'):
            assert_native_reply(response,'serviceStatus')
            assert response['interrupted'] and response['peerPid']!=response['establishment']['peerPid'], 'old server incarnation did not end'
        else: assert_connection_closed(response)
        assert command('status')['startRequests']==starts
        security_result(matrix,'restart_old_connection',response)
    finally: probe.close_connection(connection)


def native_worker_security(probe, matrix, frozen, lab, untrusted_binary, config):
    if not frozen.get('runtime'):
        matrix['scenarios']['worker_direct_access']['reason']='fixed private worker runtime required'
        return
    directory=lab/'worker-attack';directory.mkdir(mode=0o700)
    (directory/'probe-input.json').write_text(json.dumps(dict(binary=str(untrusted_binary),config=str(config),
        payload=base64.b64encode(json.dumps(probe.identity['baseline']).encode()).decode())))
    root=Path(__file__).resolve().parents[2]
    source="""import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const input=JSON.parse(process.argv[2]);
const {WorkerPort,scopeAbsentWithin}=await import(pathToFileURL(join(input.root,'packages/ai-host/dist/process.js')));
const {openSqliteStore}=await import(pathToFileURL(join(input.root,'packages/ai-store-sqlite/dist/index.js')));
const {fixtureSession}=await import(pathToFileURL(join(input.root,'packages/ai-contract/dist/testing/index.js')));
const storeResult=openSqliteStore({path:join(input.directory,'host.sqlite'),mode:'create'});
assert.equal(storeResult.ok,true);const store=storeResult.value;
const runtime={launcher:join(input.runtime,'bin/rss-ai-worker-launcher'),
    manifestDigest:createHash('sha256').update(readFileSync(join(input.runtime,'worker-manifest.json'))).digest('hex')};
const budget=()=>({timeoutMs:10000,signal:new AbortController().signal});
const namespace=fixtureSession().namespace;
const port=new WorkerPort(runtime,store,namespace,pathToFileURL(join(input.root,'tests/ai-host/provider.mjs')).href+'?scenario=direct_service_probe');
let launches;
try {
    const started=await port.start({namespace,provider:'fake',config:{id:'security',revision:'1'},
        workingDirectory:input.directory,permissions:'tools_disabled'},budget());
    assert.equal(started.ok,true,JSON.stringify(started));
    launches=await store.launches();assert.equal(launches.ok,true);assert.equal(launches.value.length,1);
    const receipt=JSON.parse(readFileSync(join(input.directory,'probe-receipt.json')));
    assert.equal(receipt.workerParent,launches.value[0].scope.root);
    const closed=await port.close(budget());assert.equal(closed.ok,true,JSON.stringify(closed));
    assert.equal(await scopeAbsentWithin(runtime,launches.value[0].scope,budget()),true);
    let rejectedReservation=false;
    const rejectingStore=new Proxy(store,{get(target,key){
        if(key==='reserveLaunch')return async()=>{rejectedReservation=true;return {ok:false,error:{code:'storage_failure',retry:'never'}}};
        const value=Reflect.get(target,key,target);return typeof value==='function'?value.bind(target):value;
    }});
    const failedPort=new WorkerPort(runtime,rejectingStore,namespace,pathToFileURL(join(input.root,'tests/ai-host/provider.mjs')).href);
    const rejected=await failedPort.start({namespace,provider:'fake',config:{id:'security',revision:'1'},
        workingDirectory:input.directory,permissions:'tools_disabled'},budget());
    assert.equal(rejectedReservation,true);assert.equal(rejected.ok,false);
    assert.deepEqual((await store.launches()).value,[]);
    await failedPort.close(budget());
    console.log(JSON.stringify({receipt,launch:launches.value[0],scopeAbsent:true,
        reservationFailure:{injection:'existing launch-fence port before spawn',result:rejected,launches:[]}}));
} finally {await port.close(budget());await store.close(budget());}
"""
    result=run(str(Path(frozen['runtime']['path'])/'bin/node'),'--input-type=module','-',json.dumps(dict(root=str(root),
        directory=str(directory),runtime=frozen['runtime']['path'])),input=source,timeout=45)
    evidence=json.loads(result.stdout)
    response=evidence['receipt']['responses'][2]
    assert_connection_closed(response)
    security_result(matrix,'worker_direct_access',evidence)
    security_result(matrix,'registration_failure',evidence['reservationFailure'])


def native_stale_helper_security(probe, matrix, query, command, protected, prior, log_offset):
    marker=protected/'stale-helper-effect'
    starts=command('status')['startRequests']
    task=command('script',user=True,body='printf x > '+shlex.quote(str(marker))+'\n')
    deadline=time.monotonic()+15
    while not command('status')['received']:
        assert time.monotonic()<deadline, 'signed helper-bound task not received'
        time.sleep(.1)
    time.sleep(3)
    assert command('status')['startRequests']==starts, 'stale helper admitted a new attempt'
    assert not marker.exists()
    assert not any(r['action']['initiator'].get('attempt')==task['attempt'] for r in query()['value']['items'])
    matrix['staleHelperPending']=dict(task=task,baseline=prior,startRequests=starts,effectAbsent=True,logOffset=log_offset)


def complete_native_security(probe, matrix, query, command, package, package_receipt, completed, effect,
        protected, installer, binary, config, administrator):
    if matrix['scenarios']['restart_old_connection']['status']!='passed':
        native_restart_security(probe,matrix,command,administrator)
    prior=query()
    offset=(protected/'service-stderr.log').stat().st_size
    run('/usr/bin/python3',str(installer),'remove','--scope','user','--binary',str(binary),'--config',str(config))
    native_stale_helper_security(probe,matrix,query,command,protected,prior,offset)
    preflight=administrator.command('refresh_preflight')
    healthy=probe.request(dict(method='serviceStatus'));assert_native_reply(healthy,'serviceStatus')
    assert healthy['envelope']['reply']['value']['readiness']['phase']=='ready', 'preflight changed service readiness'
    query()
    pending=matrix.pop('staleHelperPending')
    diagnostics=preflight['driverDiagnostics'].encode()[offset:].decode(errors='replace')
    assert 'RSS_HELPER_ENDPOINT_INVALID' in diagnostics, 'stale endpoint refusal boundary not observed'
    security_result(matrix,'helper_stale_endpoint',dict(pending,diagnostic=diagnostics))
    security_result(matrix,'refresh_preflight',dict(preflight['preflight'],healthy=healthy))
    results=administrator.command('refresh')
    security_result(matrix,'refresh_after_stop',results['afterStop'])
    run('/usr/bin/python3',str(installer),'install','--scope','user','--binary',str(binary),'--config',str(config))
    query()
    native_offer_security(probe,matrix,query,command,package,package_receipt,completed,effect)
    native_revocation_security(probe,matrix,query,command,package,package_receipt,protected)


def root_refresh_security(root, binary, config, installer_source, phase):
    # This whole program is frozen before elevation; only the owned experiment is modified.
    import contextlib, copy, hashlib, io, json, os, plistlib, re, sqlite3, subprocess, time, uuid
    from pathlib import Path
    root,binary,config=Path(root),Path(binary),Path(config)
    namespace={'__name__':'acceptance_owner'}
    exec(compile(installer_source,'<fixed installation owner>','exec'),namespace)
    plist=Path('/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist')
    original=plist.read_bytes(); document=json.loads(config.read_text())
    args=[str(binary),'--config',str(config)]
    assert plistlib.loads(original)['ProgramArguments']==args
    identity_paths=[root/'state/identity-binding',root/'state/execution-initialized']
    identity_paths.extend(p for p in (root/'state/secrets').rglob('*') if p.is_file())
    identities={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in identity_paths}
    before_config=config.read_bytes()
    before_document=namespace['candidate'](binary,config)
    journal=root/'state/execution.sqlite'
    journal_before=journal_proof(journal)
    original_attempts={row['request']:(row['snapshot']['attempts'],(row['snapshot'].get('attempt') or {}).get('id'))
        for row in journal_before['records']}
    endpoint='system/com.rss-mdm.agent.execution'
    pid=namespace['verify_registered'](endpoint,args)
    next_config=root/'refresh-candidate.json'
    rows=[]
    cases=['mode','symlink','acl','hash','cdhash','version','ipc_version','tenant','origin','state_root','registration','marker','journal','communication','secret']
    assert phase in ('preflight','afterStop')
    for name in cases if phase=='preflight' else []:
        changed=copy.deepcopy(document); actual=next_config
        next_config.write_text(json.dumps(changed));next_config.chmod(0o644)
        held=None; held_source=None; copied_plist=None; unexpected=None; paused=False
        try:
            if name=='mode': next_config.chmod(0o666)
            elif name=='symlink':
                actual=root/'refresh-link';actual.symlink_to(next_config)
            elif name=='acl': subprocess.run(['/bin/chmod','+a','everyone allow write',str(next_config)],check=True)
            elif name in ['hash','cdhash']:
                changed['service']['sha256' if name=='hash' else 'cdhash']='0'*(64 if name=='hash' else 40)
            elif name in ['version','ipc_version']: changed[name]=255
            elif name=='tenant': changed['tenant']=str(uuid.uuid4())
            elif name=='origin': changed['origin']='https://localhost:1/'
            elif name=='state_root': changed['state_root']=str(root/'other-state')
            elif name in ['marker','journal','communication','secret']:
                # This registration owns only the isolated experiment copy. Prevent its
                # live communication owner from observing the deliberate missing-file fault.
                import signal
                os.kill(pid,signal.SIGSTOP); paused=True
                held_source=identity_paths[0] if name=='marker' else (
                    journal if name=='journal' else (
                        root/'state/communication/communication.sqlite' if name=='communication' else root/'state/secrets'))
                held=held_source.with_suffix('.held');held_source.rename(held)
            if name not in ['mode','symlink','acl']: next_config.write_text(json.dumps(changed))
            target_plist=plist
            if name=='registration':
                copied_plist=root/'unknown.plist';value=plistlib.loads(original);value['Label']='other'
                copied_plist.write_bytes(plistlib.dumps(value));copied_plist.chmod(0o600);target_plist=copied_plist
            # Invalid candidates must fail before refresh can stop a running service.
            if name in ['mode','symlink','acl','hash','cdhash','version','ipc_version']:
                operation=lambda: namespace['candidate'](binary,actual)
            else:
                operation=lambda: namespace['refresh'](target_plist,'com.rss-mdm.agent.execution','system',endpoint,args,config,binary,actual)
            try: operation()
            except (RuntimeError,subprocess.CalledProcessError,FileNotFoundError) as error:
                rows.append(dict(case=name,error=str(error),stderr=getattr(error,'stderr',None)))
            else: raise AssertionError('refresh negative unexpectedly admitted: '+name)
        finally:
            if held:
                if held_source.exists():
                    unexpected=held_source.with_name(held_source.name+'.unexpected-'+uuid.uuid4().hex)
                    held_source.rename(unexpected)
                held.rename(held_source)
            if paused: os.kill(pid,signal.SIGCONT)
            if copied_plist: copied_plist.unlink(missing_ok=True)
            if actual!=next_config: actual.unlink(missing_ok=True)
            subprocess.run(['/bin/chmod','-N',str(next_config)],check=True)
            next_config.unlink(missing_ok=True)
            if unexpected: raise AssertionError('preflight recreated state; both original and unexpected facts retained: '+str(unexpected))
        assert namespace['verify_registered'](endpoint,args)==pid
        assert plist.read_bytes()==original and config.read_bytes()==before_config
        assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest()==digest for p,digest in identities.items())
    preflight=dict(rows=rows,originalPid=pid,configurationUnchanged=True,identityDigests=identities)
    if phase=='preflight':
        return dict(preflight=preflight,journalBefore=journal_before,driverDiagnostics=(root/'service-stderr.log').read_text())
    # Real launchd startup failure after the owner has stopped the original process.
    changed=plistlib.loads(original);changed['UserName']='rss-no-such-user-'+uuid.uuid4().hex
    plist.write_bytes(plistlib.dumps(changed));plist.chmod(0o600)
    try:
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                namespace['refresh'](plist,'com.rss-mdm.agent.execution','system',endpoint,args,config,binary,config)
        except (RuntimeError,subprocess.CalledProcessError) as error: failure=str(error)
        else: failure='registration publication returned; readiness must still fail for nonexistent OS user'
        try: os.kill(pid,0)
        except ProcessLookupError: pass
        else: raise AssertionError('post-stop negative did not stop the original process')
        result=subprocess.run([str(binary),'--config',str(config),'--query'],capture_output=True,text=True)
        assert result.returncode!=0 or json.loads(result.stdout)['reply']['kind']!='tasks'
        lookup=subprocess.run(['/bin/launchctl','print',endpoint],capture_output=True,text=True)
        assert not re.search(r'^\s*pid = [0-9]+\s*$',lookup.stdout,re.MULTILINE), 'candidate unexpectedly running'
        assert json.loads(config.read_bytes())==before_document
        assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest()==digest for p,digest in identities.items())
        after_stop=dict(failure=failure,oldPidAbsent=pid,diagnostic=result.stderr,
            registration=plistlib.loads(plist.read_bytes()),identityDigests=identities)
    finally:
        lookup=subprocess.run(['/bin/launchctl','print',endpoint],capture_output=True)
        if lookup.returncode==0: subprocess.run(['/bin/launchctl','bootout',endpoint],check=True)
        else: assert lookup.returncode==113
        plist.write_bytes(original);plist.chmod(0o600)
        subprocess.run(['/bin/launchctl','bootstrap','system',str(plist)],check=True)
    journal_after=journal_proof(journal)
    assert {row['request']:(row['snapshot']['attempts'],(row['snapshot'].get('attempt') or {}).get('id'))
        for row in journal_after['records']}==original_attempts, 'refresh changed durable request/attempt facts'
    return dict(preflight=preflight,afterStop=after_stop,journalBefore=journal_before,journalAfter=journal_after,
        driverDiagnostics=(root/'service-stderr.log').read_text())


def login_security(args, frozen):
    if not frozen or not args.deployment_config or not args.login_state:
        raise RuntimeError('login phase requires candidate, deployment config and login state')
    # Operate only on an already installed candidate in a dedicated acceptance environment.
    document=json.loads(args.deployment_config.read_text())
    expected=frozen['binaries']['securityProbe']
    pins=[image for image in document['clients']['images'] if image['sha256']==expected['sha256']]
    if len(pins)!=1 or document['service']['sha256']!=frozen['binaries']['service']['sha256']:
        raise RuntimeError('login installation does not match the frozen candidate')
    args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    with (args.output/'probe.log').open('w') as log:
        probe=NativeProbe(Path(pins[0]['path']),args.deployment_config,log)
        matrix=security_matrix()
        digest=hashlib.sha256(args.candidate.read_bytes()).hexdigest()
        try:
            assert probe.identity.get('guiActive') is True, 'actual foreground GUI login required'
            if args.login_phase=='prepare':
                healthy=probe.request(dict(method='tasks',after=None));assert_native_reply(healthy,'tasks')
                offers=healthy['envelope']['reply']['available']
                assert len(offers)==1 and offers[0]['expiresAt']>time.time()+300, 'fresh user offer required before login switch'
                request=offer_request(offers[0])
                request['origin']=dict(kind='ai',config=dict(id='security-login',revision='1'),
                    conversation='security-login',toolCall='security-login')
                proposed=probe.request(request);assert_native_reply(proposed,'queued')
                assert proposed['envelope']['reply']['confirmationRequired'] is True
                after_proposal=probe.request(dict(method='tasks',after=None));assert_native_reply(after_proposal,'tasks')
                state=dict(candidateSha256=digest,identity=probe.identity,offer=offers[0],request=request,
                    baseline=healthy,proposed=proposed,afterProposal=after_proposal['envelope']['reply'])
                fd=os.open(args.login_state,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
                with os.fdopen(fd,'w') as stream: json.dump(state,stream,indent=2)
                matrix['checkpoint']=dict(phase='prepared',state=str(args.login_state),
                    reason='operator must retain this owned service/backend and switch the real GUI login')
            else:
                state=json.loads(args.login_state.read_text())
                assert state['candidateSha256']==digest, 'login candidate changed'
                assert time.time()<state['offer']['expiresAt'], 'expired offer cannot prove a login boundary'
                previous,current=state['identity'],probe.identity
                def ready_and_original_offer():
                    healthy=probe.request(dict(method='serviceStatus'));assert_native_reply(healthy,'serviceStatus')
                    assert healthy['envelope']['reply']['value']['readiness']['phase']=='ready', 'unready service cannot prove a login rejection'
                    snapshot=probe.request(dict(method='tasks',after=None));assert_native_reply(snapshot,'tasks')
                    value=snapshot['envelope']['reply']
                    assert state['offer'] in value['available'], 'original offer changed or disappeared'
                    assert value['value']==state['afterProposal']['value'] and value['preparations']==state['afterProposal']['preparations'], 'login attack changed original journal facts'
                    return dict(readiness=healthy,snapshot=snapshot)
                if args.login_phase=='cross-user':
                    assert previous['uid']!=current['uid'] and current['session']!=0, 'second real login required'
                    response=probe.request(state['request']);assert_native_reply(response,'rejected')
                    matrix['loginAttack']=dict(candidateSha256=digest,phase='cross-user',identity=current,response=response,
                        checkpointSha256=hashlib.sha256(args.login_state.read_bytes()).hexdigest())
                    matrix['scenarios']['cross_user']['reason']='rejection recorded; original login must confirm readiness, original offer and unchanged journal'
                elif args.login_phase=='resume':
                    assert previous['uid']==current['uid'] and previous['binding']!=current['binding'], 'real new login generation required'
                    before=ready_and_original_offer()
                    response=probe.request(state['request']);assert_native_reply(response,'rejected')
                    after=ready_and_original_offer()
                    security_result(matrix,'login_generation',dict(before=previous,after=current,original=state['proposed'],
                        readinessBefore=before,response=response,readinessAfter=after))
                else:
                    assert args.login_response and previous['uid']==current['uid'], 'cross-user confirmation requires the original OS user'
                    attack=json.loads(args.login_response.read_text())['security']['loginAttack']
                    assert attack['phase']=='cross-user' and attack['candidateSha256']==digest
                    assert attack['checkpointSha256']==hashlib.sha256(args.login_state.read_bytes()).hexdigest()
                    assert attack['identity']['uid']!=current['uid'] and attack['identity']['session']!=0
                    assert_native_reply(attack['response'],'rejected')
                    healthy=ready_and_original_offer()
                    security_result(matrix,'cross_user',dict(original=state['proposed'],attack=attack,confirmation=healthy))
            summarize_security(matrix)
            (args.output/'receipt.json').write_text(json.dumps(dict(status=matrix['status'],security=matrix),indent=2))
        except BaseException as error:
            matrix['status']='failed';matrix['failure']=str(error)
            (args.output/'receipt.json').write_text(json.dumps(dict(status='failed',security=matrix),indent=2))
            raise
        finally: probe.close()


# ref: CPython Lib/selectors.py. Wait for actual pipe bytes, including incomplete lines;
# TextIO.readline after readiness can still block forever while waiting for a newline.
class BackendExchange:
    def __init__(self, process, timeout=8):
        self.process = process
        self.timeout = timeout
        self.pending = bytearray()
        os.set_blocking(process.stdout.fileno(), False)
        os.set_blocking(process.stdin.fileno(), False)

    def _ready(self, stream, event, deadline):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('controlled backend I/O deadline exceeded')
        with selectors.DefaultSelector() as selector:
            selector.register(stream, event)
            if not selector.select(remaining):
                raise TimeoutError('controlled backend I/O deadline exceeded')

    def read(self, deadline):
        while True:
            newline = self.pending.find(b'\n')
            if newline >= 0:
                if newline > 8 * 1024 * 1024:
                    raise RuntimeError('controlled backend response limit exceeded')
                frame = bytes(self.pending[:newline])
                del self.pending[:newline + 1]
                try:
                    value = json.loads(frame)
                    if not isinstance(value, dict): raise ValueError()
                    return value
                except (ValueError, UnicodeError):
                    raise RuntimeError('invalid controlled backend response') from None
            if len(self.pending) > 8 * 1024 * 1024:
                raise RuntimeError('controlled backend response limit exceeded')
            self._ready(self.process.stdout, selectors.EVENT_READ, deadline)
            try:
                chunk = os.read(self.process.stdout.fileno(), 65536)
            except BlockingIOError:
                continue
            if not chunk:
                raise RuntimeError('controlled backend ended before its response')
            self.pending.extend(chunk)

    def command(self, kind, **values):
        deadline = time.monotonic() + self.timeout
        frame = json.dumps(dict(kind=kind, **values)).encode() + b'\n'
        if len(frame) > 8 * 1024 * 1024:
            raise RuntimeError('controlled backend request limit exceeded')
        remaining = memoryview(frame)
        while remaining:
            self._ready(self.process.stdin, selectors.EVENT_WRITE, deadline)
            try:
                written = os.write(self.process.stdin.fileno(), remaining)
            except BlockingIOError:
                continue
            if not written: raise RuntimeError('controlled backend input closed')
            remaining = remaining[written:]
        return self.read(deadline)

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)
        self.process.stdout.close()


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
    descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
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
            if diagnostic is None:
                import traceback
                diagnostic={'ownerFrames':[{'function':frame.name,'line':frame.lineno} for frame in traceback.extract_tb(error.__traceback__)],
                    'stderr':getattr(error,'stderr',None)}
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
            setup_ok = reply(0, 'setup')
            sequence = 0
            while True:
                line = reader.readline(4097)
                if not line: break
                message = json.loads(line)
                if len(line)>4096 or set(message)!={'id','operation'} or type(message['id']) is not int or message['id']!=sequence+1 or message['operation'] not in programs or (not setup_ok and message['operation']!='cleanup'):
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
    def __init__(self, setup, initialize, restart, cleanup, lab, password_file=None, security=None, refresh=None, diagnostics=None, refresh_preflight=None):
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
        if security: programs['security'] = security.read_text()
        if refresh: programs['refresh'] = refresh.read_text()
        if refresh_preflight: programs['refresh_preflight'] = refresh_preflight.read_text()
        if diagnostics: programs['diagnostics'] = diagnostics.read_text()
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
            failure=step['diagnostics']['failure']
            if failure is not None:
                assert failure=='capture_failed' and process.get('quiescent') is False
                assert record['status']['phase']=='outcomeUnknown' and step['after']['state']=='unknown'
            assert step['after']['state'] in ('present','unknown')
    present = all(effect[key] is True for key in ('receiptPresent','payloadPresent','payloadMatches'))
    absent = all(effect[key] is False for key in ('receiptPresent','payloadPresent','payloadMatches'))
    assert present or (cancelled and absent), 'independent device effect conflicts with terminal result'
    return {'request':request, 'record':record, 'backend':backend, 'effect':effect,
        'executionUncertain':record['status'].get('phase')=='outcomeUnknown'}


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


def verify_candidate(path):
    root = Path(__file__).resolve().parents[2]
    return json.loads(run('node', str(root/'scripts/native-candidate.mjs'), 'verify', str(path.resolve()), cwd=root).stdout)


def candidate_identity(path):
    path = path.resolve()
    run('/usr/bin/codesign', '--verify', '--strict', str(path))
    details = run('/usr/bin/codesign', '-d', '--verbose=4', str(path)).stderr
    return dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(), cdhash=re.search(r'CDHash=([0-9a-f]+)', details)[1],
                signingMode='controlled-ad-hoc' if 'Signature=adhoc' in details else 'signed')


def rejected_image_probe(image, binary, config):
    # First show the exact authorized channel is healthy, then change only the client image path.
    baseline = json.loads(run(str(binary), '--config', str(config), '--query').stdout)
    if baseline['reply']['kind'] != 'tasks':
        raise RuntimeError('security baseline unavailable; negative probe not executed')
    rejected = subprocess.run([str(image), '--config', str(config), '--query'], capture_output=True, text=True, timeout=15)
    try: reply = json.loads(rejected.stdout)['reply']
    except (ValueError, KeyError):
        raise RuntimeError('wrong-image probe did not reach a verifiable IPC reply') from None
    if reply['kind'] != 'rejected':
        raise RuntimeError('wrong-image client was not explicitly rejected')
    return dict(baseline=baseline, response=reply, exitCode=rejected.returncode, changed='client image path')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--desktop', type=Path)
    parser.add_argument('--candidate', type=Path, help='Frozen prebuilt inputs; no rebuilding or resigning')
    parser.add_argument('--security', action='store_true', help='Run native attacks on this isolated fixed candidate')
    parser.add_argument('--login-phase', choices=['prepare','cross-user','resume','confirm'])
    parser.add_argument('--login-state', type=Path)
    parser.add_argument('--login-response', type=Path)
    parser.add_argument('--deployment-config', type=Path)
    parser.add_argument('--dmg-upgrade-app', type=Path, help='Second approved notarized app version for the same exact target')
    parser.add_argument('--dmg-app', type=Path, help='Approved notarized .app to place in an exact local DMG; no signature bypass')
    parser.add_argument('--authorization-password-file', type=Path)
    args = parser.parse_args()
    frozen = verify_candidate(args.candidate) if args.candidate else None
    if args.login_phase:
        return login_security(args,frozen)
    if frozen:
        for name, field in [('service', 'binary'), ('backend', 'backend'), ('desktop', 'desktop')]:
            actual = getattr(args, field)
            expected = frozen['binaries'].get(name)
            if (actual is None) != (expected is None) or (actual is not None and str(actual.resolve()) != expected['path']):
                raise RuntimeError('candidate argument mismatch')
    if args.security and (not frozen or not frozen['binaries'].get('securityProbe') or not frozen['binaries'].get('untrustedProbe')):
        raise RuntimeError('security matrix requires frozen approved and wrong-code probe identities')
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
    receipt = dict(platform=os.uname().sysname, architecture=os.uname().machine,
                   osVersion=run('/usr/bin/sw_vers','-productVersion').stdout.strip(), subject=dict(uid=os.geteuid(),session=run('/bin/launchctl','managername').stdout.strip()), candidate=frozen, journal=None, scenarios={}, security=security_matrix(), sourceHead=run('/usr/bin/git', 'rev-parse', 'HEAD').stdout.strip())
    installed = helper = False
    administrator_session = backend = exchange = backend_log = proxy = proxy_thread = protected = native_probe = probe_log = None
    try:
        # Verify before copying. Installation must preserve these bytes and code identities.
        receipt['inputArtifacts'] = {field: candidate_identity(value) for field in ['binary','desktop'] if (value := getattr(args,field)) is not None}
        for field, leaf in [('binary', 'rss-execution-service'), ('desktop', 'rss-mdm-desktop')]:
            value = getattr(args, field)
            if value is not None:
                destination = inputs / leaf
                shutil.copyfile(value.resolve(), destination); destination.chmod(0o700)
                setattr(args, field, destination)

        backend_log = (lab / 'backend.log').open('w')
        backend = subprocess.Popen([str(args.backend.resolve())], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=backend_log, text=True)
        exchange = BackendExchange(backend)
        info = exchange.read(time.monotonic() + 8)
        command = exchange.command
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
        proxy_thread = threading.Thread(target=proxy.serve_forever, daemon=True)
        proxy_thread.start()
        tag = uuid.uuid4().hex
        protected = Path('/Library/Application Support/RSS MDM Agent') / ('verification-' + tag)
        binary, config = protected / 'rss-execution-service', protected / 'execution.json'
        helper_root = inputs / 'helper'
        helper_root.mkdir(mode=0o700)
        installer = inputs / 'execution-macos.py'
        installer_source = Path(__file__).with_name('execution-macos.py').read_text()
        installer.write_text(installer_source)
        installer.chmod(0o600)
        system_installer_source = installer_source.replace("'ProcessType': 'Background'", "'ProcessType': 'Background', 'StandardErrorPath': " + repr(str(protected/'service-stderr.log')))
        shutil.copyfile(lab / 'tls.pem', inputs / 'tls.pem')
        deployment = dict(version=2, ipc_version=7, origin=f'https://localhost:{proxy.server_port}/', tenant=info['tenant'],
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
(root/'service-stderr.log').touch(mode=0o600,exist_ok=False)
binary=root/'rss-execution-service'
copy_candidate(%r,binary,%r)
subprocess.run(['/usr/bin/codesign','--verify','--strict',str(binary)],check=True)
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
''' % (str(protected), str(args.binary.resolve()), hashlib.sha256(args.binary.read_bytes()).hexdigest(), deployment, str(os.geteuid()), (lab / 'tls.pem').read_bytes(), frozen_installer(system_installer_source, ['install','--scope','system','--binary',str(binary),'--config',str(config)])))
        if args.desktop:
            # Extend this same installation owner, retaining its protected binary/config checks.
            contents = setup.read_text()
            insertion = """desktop=root/'rss-mdm-desktop'
copy_candidate(%r,desktop,%r)
subprocess.run(['/usr/bin/codesign','--verify','--strict',str(desktop)],check=True)
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
        security = refresh = diagnostics = refresh_preflight = None
        if args.security:
            probe_source = Path(frozen['binaries']['securityProbe']['path'])
            staged_probe=inputs/'security-probe'
            copy_candidate(probe_source,staged_probe,frozen['binaries']['securityProbe']['sha256'])
            probe_source=staged_probe
            probe_binary = protected / 'security-probe'
            contents = setup.read_text()
            insertion = """probe=root/'security-probe'
copy_candidate(%r,probe,%r)
subprocess.run(['/usr/bin/codesign','--verify','--strict',str(probe)],check=True)
config['clients']['images'].append(artifact(probe))
""" % (str(probe_source), frozen['binaries']['securityProbe']['sha256'])
            setup.write_text(contents.replace("path=root/'execution.json'", insertion + "path=root/'execution.json'"))
            untrusted_source=Path(frozen['binaries']['untrustedProbe']['path'])
            staged_untrusted=inputs/'untrusted-probe'
            copy_candidate(untrusted_source,staged_untrusted,frozen['binaries']['untrustedProbe']['sha256'])
            untrusted_source=staged_untrusted
            assert frozen['binaries']['untrustedProbe']['signature']['cdhash']!=frozen['binaries']['securityProbe']['signature']['cdhash'], 'wrong-code stimulus must have distinct native identity'
            untrusted_binary=protected/'untrusted-probe'
            contents=setup.read_text()
            insertion="copy_candidate(%r,root/'untrusted-probe',%r)\n" % (
                str(untrusted_source),frozen['binaries']['untrustedProbe']['sha256'])
            setup.write_text(contents.replace("path=root/'execution.json'",insertion+"path=root/'execution.json'"))
            security = lab/'fake-service.py'
            security.write_text("""import json,plistlib,subprocess,os
from pathlib import Path
root=Path(%r);label='com.rss-mdm.agent.security.fake'
endpoint='system/'+label
assert subprocess.run(['/bin/launchctl','print',endpoint],capture_output=True).returncode==113, 'unknown fake registration'
plist=root/'fake.plist'
document=dict(Label=label,ProgramArguments=[%r,'--fake-service'],MachServices={label:True},
    RunAtLoad=True,StandardErrorPath=str(root/'fake-stderr.log'))
fd=os.open(plist,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
with os.fdopen(fd,'wb') as stream: plistlib.dump(document,stream)
subprocess.run(['/bin/launchctl','bootstrap','system',str(plist)],check=True)
print(json.dumps(dict(label=label,plist=str(plist))))
""" % (str(protected),str(probe_binary)))
            security.write_text(security.read_text().replace(
                "print(json.dumps(dict(label=label,plist=str(plist))))",
                """result=subprocess.run(%r,capture_output=True,text=True)
assert result.returncode!=0 and 'remove the matching helper' in result.stderr, 'loaded helper did not block refresh'
print(json.dumps(dict(label=label,plist=str(plist),helperGuard=result.stderr)))
""" % frozen_installer(installer_source,['refresh','--scope','system','--binary',str(binary),
                    '--config',str(config),'--candidate-binary',str(binary),'--candidate-config',str(config)])))
            refresh=lab/'refresh-security.py'
            refresh_preflight=lab/'refresh-preflight.py'
            refresh_source="import json,sqlite3\n"+inspect.getsource(journal_proof)+inspect.getsource(root_refresh_security)
            for phase,path in [('preflight',refresh_preflight),('afterStop',refresh)]:
                path.write_text(refresh_source+"\nprint(json.dumps(root_refresh_security(%r,%r,%r,%r,%r)))\n" %
                    (str(protected),str(binary),str(config),installer_source,phase))
            diagnostics=lab/'diagnostics.py'
            diagnostics.write_text("import json\nfrom pathlib import Path\nprint(json.dumps({'log':Path(%r).read_text()[-65536:]}))\n" % str(protected/'service-stderr.log'))
        cleanup = lab / 'remove.py'
        cleanup.write_text(inspect.getsource(journal_proof) + """import json,sqlite3,subprocess
from pathlib import Path
expected=%r
plist=Path('/Library/LaunchDaemons/com.rss-mdm.agent.execution.plist')
if plist.exists(): subprocess.run(expected,check=True)
journal=Path(%r)
if journal.exists():
    proof=journal_proof(journal)
    diagnostic=journal.parent.parent/'service-stderr.log'
    if diagnostic.exists(): proof['driverDiagnostics']=diagnostic.read_text()[-32768:]
    print(json.dumps(proof))
""" % (frozen_installer(installer_source, ['remove', '--scope', 'system', '--binary', str(binary), '--config', str(config)]), str(protected / 'state/execution.sqlite')))
        if args.security:
            cleanup.write_text("""import plistlib,subprocess
from pathlib import Path
plist=Path(%r)
if plist.exists():
    value=plistlib.loads(plist.read_bytes())
    assert value['ProgramArguments']==[%r,'--fake-service'], 'fake registration owner mismatch'
    result=subprocess.run(['/bin/launchctl','print','system/com.rss-mdm.agent.security.fake'],capture_output=True)
    if result.returncode==0: subprocess.run(['/bin/launchctl','bootout','system/com.rss-mdm.agent.security.fake'],check=True)
    else: assert result.returncode==113, 'fake registration lookup failed'
    plist.unlink()
""" % (str(protected/'fake.plist'),str(probe_binary)) + cleanup.read_text())
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
process_file=Path(%r)
if process_file.exists(): os.kill(int(process_file.read_text()),0)
subprocess.run(['/bin/launchctl','kickstart','-k','system/com.rss-mdm.agent.execution'],check=True)
""" % ([str(binary),'--config',str(config)],str(protected/'restart-pid')))
        cleanup.write_text("from pathlib import Path\nif Path(%r).exists(): Path(%r).write_text('finished')\n" % (str(protected/'restart-pid'),str(protected/'restart-finish')) + cleanup.read_text())
        receipt['journal'] = str(protected / 'state/execution.sqlite')
        first = None if args.desktop else command('script', body='printf \'{"fixture":"system"}\\n\'\n')
        if not args.desktop: command('result_failure')
        administrator_session = AdministratorSession(setup,initialize,restart,cleanup,lab,args.authorization_password_file,
            **(dict(security=security, refresh=refresh, diagnostics=diagnostics, refresh_preflight=refresh_preflight) if args.security else {}))
        administrator_session.start()
        installed = True
        if args.desktop and select.select([sys.stdin], [], [], 0)[0]:
            raise RuntimeError('native acceptance parent ended before readiness; refusing initialization')
        receipt['artifact'] = json.loads(config.read_text())['service']
        assert receipt['artifact']['sha256'] == receipt['inputArtifacts']['binary']['sha256'], 'installed service candidate changed'
        if frozen: verify_candidate(args.candidate)
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
            deadline = time.monotonic() + 15
            while True:
                reply = json.loads(run(str(binary), '--config', str(config), '--query').stdout)['reply']
                if reply['kind'] == 'tasks' and isinstance(reply['value']['items'], list):
                    return reply
                # Read-only queries can meet a busy single owner while it validates material.
                # Authentication rejection is terminal; only bounded Unavailable is retried.
                if reply['kind'] != 'unavailable' or time.monotonic() >= deadline:
                    raise RuntimeError('authenticated task query failed: ' + json.dumps(reply))
                time.sleep(.2)
        def completed(task, seconds=30):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                status = command('status')
                if acknowledged_result(status, task) is not None:
                    return status
                time.sleep(.2)
            raise RuntimeError('backend result deadline exceeded')
        if args.security:
            receipt['scenarios']['fake_registration'] = administrator_session.command('security')
            probe_log = (lab/'security-probe.log').open('w')
            native_probe = NativeProbe(probe_binary, config, probe_log)
            rejected_probe=NativeProbe(untrusted_binary,config,probe_log)
            try:
                response=rejected_probe.request(dict(method='serviceStatus'))
                assert_connection_closed(response)
                before=native_probe.request(dict(method='serviceStatus'));assert_native_reply(before,'serviceStatus')
                security_result(receipt['security'],'wrong_code_identity',dict(
                    baseline=before,response=response,artifact=frozen['binaries']['untrustedProbe']))
            finally: rejected_probe.close()
            native_worker_security(native_probe,receipt['security'],frozen,lab,untrusted_binary,config)
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
                elif method == 'security':
                    value = rejected_image_probe(args.binary,binary,config)
                    receipt['scenarios']['wrong_image_same_uid'] = value
                    security_result(receipt['security'], 'wrong_image_same_uid', value)
                    if native_probe: native_transport_security(native_probe, receipt['security'], query, command, administrator_session)
                elif method == 'status': value = command('status')
                elif method == 'processSecurity':
                    proof=request['proof']
                    assert frozen and proof['runtimeTreeSha256']==frozen['runtime']['sha256'], 'process proof candidate mismatch'
                    for name in ['host_crash','launcher_crash','retained_descendant','close_timeout','unknown_scope']:
                        security_result(receipt['security'],name,proof['scenarios'][name])
                    value=dict(recorded=True)
                elif method == 'catalog':
                    value = command('package', path=str(pkg), receipt=package_receipt, user=True)
                    receipt['scenarios']['second_unexecuted_catalog_offer'] = value
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
                    break
                else: raise RuntimeError('unknown native harness control')
                print(json.dumps({'id': request['id'], 'value': value}), flush=True)
                if method == 'finish': break
            validate_desktop_finish(finish, completion)
            receipt['scenarios']['completion'] = completion
            receipt['scenarios']['native_final'] = query()
            receipt['scenarios']['backend_final'] = command('status')
            security_result(receipt['security'],'desktop_service',completion)
            security_result(receipt['security'],'desktop_codex',dict(finish=finish,candidate=frozen))
            assert finish.get('processScopeEmpty') is True, 'native process cleanup not proven'
            scope_proof=finish.get('processScopeProof')
            assert scope_proof and scope_proof['observationComplete'] and scope_proof['scopes'], 'actual worker scope collection required'
            assert all(row['confirmed'] for row in scope_proof['scopes']), 'actual worker scope remains unknown or present'
            if frozen: assert scope_proof['runtimeTreeSha256']==frozen['runtime']['sha256'], 'scope proof candidate mismatch'
            security_result(receipt['security'],'process_scope_empty',dict(finish=finish))
            if native_probe:
                complete_native_security(native_probe,receipt['security'],query,command,pkg,package_receipt,
                    completed,effect,protected,installer,binary,config,administrator_session)
            receipt['status'] = 'passed'
            print(json.dumps(dict(id=finish['id'],value=dict(journeyStatus='passed'))),flush=True)
            return
        receipt['scenarios']['authenticated_ipc'] = query()
        receipt['scenarios']['wrong_image_same_uid'] = rejected_image_probe(args.binary, binary, config)
        security_result(receipt['security'], 'wrong_image_same_uid', receipt['scenarios']['wrong_image_same_uid'])
        system = completed(first['attempt'])
        validate_script(acknowledged_result(system, first['attempt']), 'system')
        if native_probe: native_transport_security(native_probe, receipt['security'], query, command, administrator_session)
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
        if args.dmg_app:
            image_source = lab/'contained'; image_source.mkdir()
            contained_pkg = lab/'contained.pkg';contained_receipt = package_receipt+'.dmg'
            run('/usr/bin/pkgbuild','--root',str(payload),'--identifier',contained_receipt,'--version','1.0','--install-location',str(protected/'contained-package-payload'),str(contained_pkg))
            run('/usr/bin/ditto',str(contained_pkg),str(image_source/'fixed.pkg'))
            contained_image = lab/'contained.dmg'; contained_volume = 'RSS-PKG-'+tag
            run('/usr/bin/hdiutil','create','-srcfolder',str(image_source),'-volname',contained_volume,'-format','UDRO',str(contained_image))
            pkg_task = command('dmg',path=str(contained_image),volume=contained_volume,receipt=contained_receipt,contained='fixed.pkg',
                length=contained_pkg.stat().st_size,sha256=list(hashlib.sha256(contained_pkg.read_bytes()).digest()),bundle=contained_receipt,version='1.0')
            pkg_remote = completed(pkg_task['attempt'],120)
            pkg_event = acknowledged_result(pkg_remote,pkg_task['attempt'])
            assert pkg_event['steps'][0]['process']['kind'] == 'exited' and pkg_event['steps'][0]['process']['code'] != 0
            assert pkg_event['steps'][0]['after']['state'] == 'absent'
            assert not (protected/'contained-package-payload/fixed.txt').exists()
            assert subprocess.run(['/usr/sbin/pkgutil','--pkg-info-plist',contained_receipt],capture_output=True).returncode != 0
            receipt['scenarios']['dmg_contained_pkg_unsigned_rejected'] = pkg_remote
        third = command('package', path=str(pkg), receipt=package_receipt)
        package = completed(third['attempt'], 60)
        event = acknowledged_result(package, third['attempt'])
        assert event['kind'] == 'software_result' and len(event['steps']) == 1
        step = event['steps'][0]
        assert step['process'] == {'kind':'exited','code':0}
        assert step['diagnostics']['failure'] == 'capture_failed'
        assert (protected / 'package-payload/fixed.txt').read_text() == 'controlled package payload\n'
        import plistlib
        installed_receipt = plistlib.loads(run('/usr/sbin/pkgutil', '--pkg-info-plist', package_receipt).stdout.encode())
        assert installed_receipt['pkg-version'] == '1.0'
        # macOS process groups cannot establish global quiescence. Keep Unknown while
        # independently requiring the actual installer exit, receipt and payload facts.
        assert step['after']['state'] in ('present', 'unknown')
        receipt['scenarios']['package_exit_and_independent_effect'] = package
        final = query()
        for attempt in [first['attempt'], second['attempt'], third['attempt']]:
            records = [r for r in final['value']['items'] if r['action']['initiator'].get('attempt') == attempt]
            assert len(records) == 1 and records[0]['status']['attempts'] == 1
        receipt['scenarios']['final_ipc'] = final
        if args.dmg_app:
            application = args.dmg_app.resolve()
            info = plistlib.loads((application/'Contents/Info.plist').read_bytes())
            run('/usr/bin/codesign','--verify','--deep','--strict',str(application))
            run('/usr/sbin/spctl','--assess','--type','execute',str(application))
            source = lab/'image-payload'; source.mkdir()
            run('/usr/bin/ditto',str(application),str(source/application.name))
            image = lab/'fixed.dmg'
            volume = 'RSS-'+tag
            run('/usr/bin/hdiutil','create','-srcfolder',str(source),'-volname',volume,'-format','UDRO',str(image))
            target_name = 'RSS-'+tag+'.app'
            installed_app = Path('/Applications')/target_name
            assert not installed_app.exists(), 'refuse existing application target'
            parameters = dict(path=str(image),volume=volume,application=application.name,bundle=info['CFBundleIdentifier'],
                              version=info['CFBundleShortVersionString'],target=target_name)
            app_task = command('dmg',**parameters)
            remote = completed(app_task['attempt'],120)
            app_event = acknowledged_result(remote,app_task['attempt'])
            assert app_event['steps'][0]['after']['state'] == 'present'
            assert app_event['steps'][0]['diagnostics']['failure'] is None
            run('/usr/bin/codesign','--verify','--deep','--strict',str(installed_app))
            run('/usr/sbin/spctl','--assess','--type','execute',str(installed_app))
            app_records = [r for r in query()['value']['items'] if r['action']['initiator'].get('attempt') == app_task['attempt']]
            assert len(app_records) == 1 and app_records[0]['status']['phase'] == 'verified'
            receipt['scenarios']['dmg_app_install'] = dict(backend=remote,record=app_records[0],imageSha256=hashlib.sha256(image.read_bytes()).hexdigest())
            if args.dmg_upgrade_app:
                upgrade_app = args.dmg_upgrade_app.resolve(); upgrade_info = plistlib.loads((upgrade_app/'Contents/Info.plist').read_bytes())
                assert upgrade_info['CFBundleIdentifier'] == info['CFBundleIdentifier'] and upgrade_info['CFBundleShortVersionString'] != info['CFBundleShortVersionString']
                run('/usr/bin/codesign','--verify','--deep','--strict',str(upgrade_app)); run('/usr/sbin/spctl','--assess','--type','execute',str(upgrade_app))
                upgrade_source = lab/'upgrade-image-payload';upgrade_source.mkdir(); run('/usr/bin/ditto',str(upgrade_app),str(upgrade_source/upgrade_app.name))
                upgrade_image = lab/'upgrade.dmg';upgrade_volume = 'RSS-UP-'+tag
                run('/usr/bin/hdiutil','create','-srcfolder',str(upgrade_source),'-volname',upgrade_volume,'-format','UDRO',str(upgrade_image))
                parameters.update(path=str(upgrade_image),volume=upgrade_volume,application=upgrade_app.name,version=upgrade_info['CFBundleShortVersionString'])
                upgrade_task = command('dmg',**parameters); upgraded = completed(upgrade_task['attempt'],120)
                upgrade_event = acknowledged_result(upgraded,upgrade_task['attempt'])
                assert upgrade_event['steps'][0]['before']['version'] == info['CFBundleShortVersionString']
                assert upgrade_event['steps'][0]['after']['version'] == upgrade_info['CFBundleShortVersionString'] and upgrade_event['steps'][0]['diagnostics']['failure'] is None
                observed_upgrade = plistlib.loads((installed_app/'Contents/Info.plist').read_bytes())
                assert observed_upgrade['CFBundleShortVersionString'] == upgrade_info['CFBundleShortVersionString']
                run('/usr/bin/codesign','--verify','--deep','--strict',str(installed_app))
                run('/usr/sbin/spctl','--assess','--type','execute',str(installed_app))
                receipt['scenarios']['dmg_app_upgrade'] = upgraded
            remove_task = command('dmg',uninstall=True,**parameters)
            removed = completed(remove_task['attempt'],120)
            remove_event = acknowledged_result(removed,remove_task['attempt'])
            assert remove_event['steps'][0]['after']['state'] == 'absent'
            assert remove_event['steps'][0]['diagnostics']['failure'] is None and not installed_app.exists()
            receipt['scenarios']['dmg_app_uninstall'] = removed
            mounts = plistlib.loads(run('/usr/bin/hdiutil','info','-plist').stdout.encode())
            # The worker mounts its protected cached image, rather than the lab source path.
            assert not any(Path(item.get('image-path','')).is_relative_to(protected)
                           for item in mounts.get('images',[])), 'owned cached image remains mounted'
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
        old_connection = native_probe.open(establish=True) if native_probe else None
        old_starts=command('status')['startRequests']
        administrator_session.command('restart')
        if native_probe:
            response = native_probe.send(old_connection, json.dumps(native_probe.identity['baseline']).encode())
            if response.get('transport')=='reply' and response.get('bytes'):
                assert_native_reply(response,'serviceStatus')
                assert response['interrupted'] and response['peerPid']!=response['establishment']['peerPid'], 'old server incarnation did not end'
            else: assert_connection_closed(response)
            assert command('status')['startRequests']==old_starts
            security_result(receipt['security'], 'restart_old_connection', response)
            native_probe.close_connection(old_connection)
        time.sleep(3)
        reopened = query()
        records = [r for r in reopened['value']['items'] if r['action']['initiator'].get('attempt') == fifth['attempt']]
        assert len(records) == 1 and records[0]['status']['attempts'] == 1
        assert counter.read_text() == 'x'
        assert records[0]['status']['phase'] == 'outcomeUnknown'
        receipt['scenarios']['restart_no_redispatch'] = records[0]

        if native_probe:
            def effect():
                detected = subprocess.run(['/usr/sbin/pkgutil','--pkg-info-plist',package_receipt],capture_output=True)
                payload_file=protected/'package-payload/fixed.txt'
                return dict(receiptPresent=detected.returncode==0,payloadMatches=payload_file.exists() and payload_file.read_text()=='controlled package payload\n')
            complete_native_security(native_probe,receipt['security'],query,command,pkg,package_receipt,
                completed,effect,protected,installer,binary,config,administrator_session)
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt['status'] = 'cancelled' if isinstance(error, (AuthorizationCancelled, InterruptedError)) else 'failed'
        receipt['error'] = str(error)
        if args.security: receipt['security']['failure'] = str(error)
        if installed:
            try:
                receipt['scenarios']['failure_snapshot'] = query()
                receipt['scenarios']['failure_backend'] = command('status')
            except BaseException:
                pass # Preserve the primary failure if the authenticated owner is unavailable.
        if isinstance(error, subprocess.CalledProcessError):
            (lab / 'command-error.txt').write_text((error.stdout or '') + (error.stderr or ''))
        raise
    finally:
        if protected is not None and protected.exists(): installed = True
        cleanup_errors = []
        if native_probe:
            try: native_probe.close()
            except BaseException as error: cleanup_errors.append(str(error))
        if probe_log: probe_log.close()
        if helper:
            try:
                helper_plist=Path.home()/'Library/LaunchAgents/com.rss-mdm.agent.execution.user.plist'
                if helper_plist.exists():
                    run('/usr/bin/python3', str(installer), 'remove', '--scope', 'user', '--binary', str(binary), '--config', str(config))
                else:
                    lookup=subprocess.run(['/bin/launchctl','print',f'gui/{os.geteuid()}/com.rss-mdm.agent.execution.user'],capture_output=True)
                    assert lookup.returncode==113, 'helper registration missing its owned removal evidence'
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
        if proxy is not None:
            if proxy_thread is not None and proxy_thread.is_alive(): proxy.shutdown()
            proxy.server_close()
        if backend is not None:
            try:
                (exchange or BackendExchange(backend)).close()
            except BaseException as error:
                cleanup_errors.append(str(error))
        if backend_log is not None: backend_log.close()
        if cleanup_errors:
            receipt['status'] = 'failed'
            receipt['cleanupErrors'] = cleanup_errors
        if frozen:
            try: verify_candidate(args.candidate)
            except BaseException as error:
                receipt['status']='failed'; receipt['candidateError']=str(error)
        receipt['journeyStatus'] = receipt.get('status', 'failed')
        summarize_security(receipt['security'])
        if receipt['journeyStatus'] == 'passed' and receipt['security']['status'] != 'passed':
            receipt['status'] = 'partial'
        receipt['installationCreated'] = installed
        receipt['cleanup'] = 'incomplete' if cleanup_errors else 'complete'
        receipt['inputStaging'] = str(receipt_inputs)
        (lab / 'receipt.json').write_text(json.dumps(receipt, indent=2))
        if not cleanup_errors:
            # The original helper work root can hold unresolved recovery evidence.
            for leaf in ['rss-execution-service','rss-mdm-desktop','security-probe','untrusted-probe','execution-macos.py','tls.pem']:
                (inputs / leaf).unlink(missing_ok=True)
        if cleanup_errors:
            raise RuntimeError('acceptance cleanup incomplete')
        if receipt.get('candidateError') and not receipt.get('error'):
            raise RuntimeError('fixed candidate validation failed at closeout')



if __name__ == '__main__':
    def cancel(signum, frame):
        raise InterruptedError('native service acceptance cancelled')
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    main()
