#!/usr/bin/env python3
"""Ensure the OS acceptance harness cannot accept failed or unacknowledged results."""
import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile
import json
import sqlite3
import socket
import threading
import os
import time
import inspect
import subprocess
from unittest.mock import patch, MagicMock

spec = importlib.util.spec_from_file_location('acceptance', Path(__file__).with_name('verify-execution-macos.py'))
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class BackendLifecycleTests(unittest.TestCase):
    def test_fixed_operation_cancellation_reaps_child_and_restores_before_cleanup(self):
        for disconnect in (False,True):
          with self.subTest(disconnect=disconnect), tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            root=Path(directory);endpoint=str(root/'control');pid_file=root/'pid';restored=root/'restored';cleaned=root/'cleaned'
            listener=socket.socket(socket.AF_UNIX);listener.bind(endpoint);listener.listen(1)
            setup="import subprocess\nfrom pathlib import Path\ntry:\n subprocess.run(['/bin/sh','-c',%r],timeout=%r)\nfinally:\n Path(%r).write_text('restored')" % (
                'echo $$ > '+str(pid_file)+'; exec /bin/sleep 60',10 if disconnect else .3,str(restored))
            programs={'setup':setup,'cleanup':"from pathlib import Path\nPath(%r).write_text('cleaned')" % str(cleaned)}
            failures=[]
            def run():
                try: acceptance.authorized_steps(programs,endpoint,123,501,time.clock_gettime(time.CLOCK_MONOTONIC)+10)
                except BaseException as error: failures.append(error)
            with patch.object(acceptance,'peer_identity',return_value=(123,501)):
                worker=threading.Thread(target=run);worker.start()
                connection,_=listener.accept();connection.settimeout(3)
                connection.sendall(b'{"id":0,"operation":"setup"}\n')
                end=time.monotonic()+2
                while not pid_file.exists():
                    self.assertLess(time.monotonic(),end);time.sleep(.01)
                if disconnect: connection.close()
                else:
                    with connection,connection.makefile('rb') as reader:
                        result=json.loads(reader.readline());self.assertFalse(result['ok'])
                        self.assertEqual(result['value']['reason'],'operation budget exceeded')
                        connection.sendall(b'{"id":1,"operation":"cleanup"}\n')
                        self.assertTrue(json.loads(reader.readline())['ok'])
                worker.join(3);self.assertFalse(worker.is_alive())
                self.assertTrue(restored.exists());self.assertTrue(cleaned.exists())
                with self.assertRaises(ProcessLookupError): os.kill(int(pid_file.read_text()),0)
            listener.close()

    def test_started_security_failure_is_distinct_from_unexecuted_scenario(self):
        matrix=acceptance.security_matrix()
        acceptance.security_begin(matrix,'offer_replay')
        self.assertEqual(matrix['scenarios']['offer_replay']['status'],'failed')
        self.assertEqual(matrix['scenarios']['offer_expiry']['status'],'notExecuted')
        self.assertEqual(acceptance.summarize_security(matrix),'failed')
        acceptance.security_result(matrix,'offer_replay',{'actual':'evidence'})
        self.assertEqual(matrix['scenarios']['offer_replay']['status'],'passed')

    def test_failed_setup_keeps_only_the_fixed_cleanup_channel_available(self):
        with tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            endpoint=str(Path(directory)/'control')
            listener=socket.socket(socket.AF_UNIX);listener.bind(endpoint);listener.listen(1)
            programs={'setup':'raise PermissionError("protected input")',
                      'cleanup':'import json\nprint(json.dumps({"cleaned":True}))'}
            failures=[]
            def run():
                try: acceptance.authorized_steps(programs,endpoint,123,501,time.clock_gettime(time.CLOCK_MONOTONIC)+10)
                except BaseException as error: failures.append(error)
            with patch.object(acceptance,'peer_identity',return_value=(123,501)):
                worker=threading.Thread(target=run);worker.start()
                connection,_=listener.accept();connection.settimeout(3)
                with connection,connection.makefile('rb') as reader:
                    connection.sendall(b'{"id":0,"operation":"setup"}\n')
                    setup=json.loads(reader.readline())
                    self.assertFalse(setup['ok']);self.assertEqual(setup['error'],'PermissionError')
                    connection.sendall(b'{"id":1,"operation":"cleanup"}\n')
                    closed=json.loads(reader.readline())
                    self.assertTrue(closed['ok']);self.assertEqual(closed['value'],{'cleaned':True})
                worker.join(3);self.assertFalse(worker.is_alive());self.assertEqual(failures,[])
            listener.close()

    def backend(self, source):
        return subprocess.Popen(['/usr/bin/python3', '-u', '-c', source], stdin=subprocess.PIPE, stdout=subprocess.PIPE)

    def test_partial_line_has_a_deadline_and_owned_backend_is_reaped(self):
        child=self.backend("import time;print('{',end='',flush=True);time.sleep(60)")
        owner=acceptance.BackendExchange(child)
        started=time.monotonic()
        try:
            with self.assertRaises(TimeoutError): owner.read(time.monotonic()+.1)
            self.assertLess(time.monotonic()-started,1)
        finally: owner.close()
        self.assertIsNotNone(child.poll())

    def test_split_lines_and_large_requests_use_actual_readiness(self):
        child=self.backend("import sys,json,time;sys.stdout.write('{');sys.stdout.flush();time.sleep(.02);print('\"ready\":true}');v=json.loads(sys.stdin.readline());print(json.dumps({'size':len(v['body'])}))")
        owner=acceptance.BackendExchange(child)
        try:
            self.assertEqual(owner.read(time.monotonic()+1),{'ready':True})
            self.assertEqual(owner.command('script',body='x'*200_000),{'size':200_000})
        finally: owner.close()

    def test_full_input_pipe_also_has_a_deadline(self):
        child=self.backend("import time;print('{}',flush=True);time.sleep(60)")
        owner=acceptance.BackendExchange(child,timeout=.1)
        try:
            owner.read(time.monotonic()+1)
            started=time.monotonic()
            with self.assertRaises(TimeoutError): owner.command('script',body='x'*500_000)
            self.assertLess(time.monotonic()-started,1)
        finally: owner.close()

    def test_invalid_startup_response_runs_preparation_cleanup_without_installation(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);backend=root/'backend';pid=root/'pid';output=root/'output'
            backend.write_text('#!/usr/bin/python3\nimport os,time\nfrom pathlib import Path\nPath('+repr(str(pid))+').write_text(str(os.getpid()))\nprint("[]",flush=True)\ntime.sleep(60)\n');backend.chmod(0o700)
            with patch.object(acceptance.sys,'argv',['verify','--binary','/bin/sh','--backend',str(backend),'--output',str(output)]), patch.object(acceptance,'AdministratorSession') as administrator:
                with self.assertRaisesRegex(RuntimeError,'invalid controlled backend response'): acceptance.main()
                administrator.assert_not_called()
            receipt=json.loads((output/'receipt.json').read_text())
            self.assertEqual(receipt['cleanup'],'complete');self.assertFalse(receipt['installationCreated'])
            with self.assertRaises(ProcessLookupError): os.kill(int(pid.read_text()),0)
            staged=Path(receipt['inputStaging']);self.assertFalse((staged/'rss-execution-service').exists());staged.rmdir()

    def test_prepared_administrator_programs_remain_valid_before_authority(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);backend=root/'backend';output=root/'output'
            info={'origin':'http://127.0.0.1:1','tenant':'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa','key':'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'}
            backend.write_text('#!/usr/bin/python3\nimport sys,json\nprint('+repr(json.dumps(info))+',flush=True)\nfor line in sys.stdin: pass\n');backend.chmod(0o700)
            programs=[]
            class NoAuthority:
                reader=None
                def __init__(self,setup,initialize,restart,cleanup,*args):
                    for path in (setup,initialize,restart,cleanup):
                        program=path.read_text();compile(program,str(path),'exec');programs.append(program)
                def start(self): raise RuntimeError('stop before administrator authority')
                def close(self): pass
            with patch.object(acceptance.sys,'argv',['verify','--binary','/bin/sh','--desktop','/bin/sh','--backend',str(backend),'--output',str(output)]),patch.object(acceptance,'AdministratorSession',NoAuthority):
                with self.assertRaisesRegex(RuntimeError,'stop before administrator authority'): acceptance.main()
            self.assertEqual(len(programs),4)
            receipt=json.loads((output/'receipt.json').read_text());self.assertEqual(receipt['cleanup'],'complete');self.assertFalse(receipt['installationCreated'])
            staged=Path(receipt['inputStaging']);(staged/'helper').rmdir();staged.rmdir()


class EvidenceTests(unittest.TestCase):
    def test_partial_matrix_cannot_claim_complete_security_from_a_successful_journey(self):
        matrix=acceptance.security_matrix()
        self.assertEqual(acceptance.summarize_security(matrix),'notExecuted')
        acceptance.security_result(matrix,'authorized_channel',{'peerUid':0,'reply':'actual baseline'})
        self.assertEqual(acceptance.summarize_security(matrix),'partial')
        self.assertEqual(matrix['scenarios']['cross_user']['status'],'notExecuted')
        self.assertEqual(matrix['scenarios']['legacy_challenge']['status'],'notApplicable')
        matrix['failure']='native attack assertion failed'
        self.assertEqual(acceptance.summarize_security(matrix),'failed')

    def test_security_refuses_unknown_empty_and_unavailable_evidence(self):
        matrix=acceptance.security_matrix()
        for name,evidence in [('unknown',{'reply':True}),('authorized_channel',None)]:
            with self.assertRaises(RuntimeError): acceptance.security_result(matrix,name,evidence)
        for result in [{'transport':'timeout'}, {'transport':'reply','peerUid':0,'envelope':{'reply':{'kind':'unavailable'}}},
                       {'transport':'reply','peerUid':501,'envelope':{'reply':{'kind':'rejected'}}}]:
            with self.assertRaises(AssertionError): acceptance.assert_native_reply(result,'rejected')
        for result in [{'transport':'timeout'}, {'transport':'reply','bytes':1}]:
            with self.assertRaises(AssertionError): acceptance.assert_connection_closed(result)

    def test_invalid_native_probe_startup_reaps_its_owned_process(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);binary=root/'probe'
            binary.write_text('#!/usr/bin/python3\nimport time\nprint("{}",flush=True)\ntime.sleep(60)\n')
            binary.chmod(0o700)
            opened=[];spawn=acceptance.subprocess.Popen
            def capture(*args,**kwargs):
                process=spawn(*args,**kwargs);opened.append(process);return process
            with patch.object(acceptance.subprocess,'Popen',side_effect=capture):
                with (root/'log').open('w') as log:
                    with self.assertRaisesRegex(RuntimeError,'not ready'):
                        acceptance.NativeProbe(binary,root/'config',log)
            self.assertEqual(len(opened),1)
            self.assertIsNotNone(opened[0].poll())

    def test_authorization_password_file_is_private_and_never_an_argument(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);password=root/'.passwd';password.write_text('synthetic-admin-password\n');password.chmod(0o600)
            self.assertEqual(acceptance.read_authorization_password(password),'synthetic-admin-password')
            subprocess.run(['/bin/chmod','+a','everyone allow read,write',str(password)],check=True)
            try:
                self.assertEqual(password.stat().st_mode & 0o777,0o600)
                with self.assertRaisesRegex(RuntimeError,'extended ACL'):
                    acceptance.read_authorization_password(password)
            finally: subprocess.run(['/bin/chmod','-N',str(password)],check=True)
            password.chmod(0o644)
            with self.assertRaisesRegex(RuntimeError,'private'): acceptance.read_authorization_password(password)
            password.chmod(0o600);link=root/'link';link.symlink_to(password)
            with self.assertRaises(OSError): acceptance.read_authorization_password(link)
            script=root/'fixed.py';script.write_text('pass')
            process=MagicMock()
            with patch.object(acceptance.subprocess,'Popen',return_value=process) as popen:
                session=acceptance.AdministratorSession(script,script,script,script,root,password)
            session.close()
            self.assertEqual(popen.call_args.args[0],['/usr/bin/osascript'])
            self.assertIn(b'password "synthetic-admin-password"',process.stdin.write.call_args.args[0])
            self.assertNotIn('synthetic-admin-password',(root/'administrator-session.log').read_text())
            password.write_text('')
            with self.assertRaisesRegex(RuntimeError,'empty'): acceptance.read_authorization_password(password)
            fifo=root/'fifo';os.mkfifo(fifo,0o600)
            source='import os,stat\n'+inspect.getsource(acceptance.read_authorization_password)+f'\ntry:\n read_authorization_password({str(fifo)!r})\nexcept RuntimeError:\n pass\nelse:\n raise AssertionError("FIFO accepted")'
            subprocess.run(['/usr/bin/python3','-I','-c',source],check=True,timeout=2)

    def test_candidate_copy_publishes_only_the_verified_read(self):
        with tempfile.TemporaryDirectory() as directory:
            source, target = Path(directory)/'source', Path(directory)/'target'
            source.write_bytes(b'verified')
            original = Path.read_bytes
            def read(path):
                value = original(path)
                if path == source: source.write_bytes(b'replaced')
                return value
            with patch.object(Path, 'read_bytes', read):
                acceptance.copy_candidate(source, target, acceptance.hashlib.sha256(b'verified').hexdigest())
            self.assertEqual(target.read_bytes(), b'verified')
            self.assertEqual(source.read_bytes(), b'replaced')

    def test_privileged_installer_is_frozen_not_reopened(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)/'installer.py'; source.write_text('print("original")')
            command = acceptance.frozen_installer(source.read_text(), ['remove'])
            source.write_text('raise RuntimeError("replaced")')
            self.assertEqual(command[:3], ['/usr/bin/python3', '-I', '-c'])
            self.assertEqual(command[3], 'print("original")')
            self.assertNotIn(str(source), command)

    def test_desktop_pass_requires_terminal_acknowledged_process_and_independent_effect(self):
        record = {'action':{'initiator':{'kind':'backend','attempt':'attempt'}},'status':{'operationRequestId':'request','attempts':1,'process':{'finished':True,'end':'exited','exitCode':0}}}
        event = {'kind':'software_result','steps':[{'process':{'kind':'exited','code':0},'diagnostics':{'failure':None},'after':{'state':'unknown'}}]}
        backend = {'startRequests':1,'results':{'op':{'attemptId':'attempt','event':event}},'acknowledged':['op']}
        effect = {'receiptPresent':True,'payloadPresent':True,'payloadMatches':True}
        acceptance.validate_desktop_completion([record], backend, effect, 'request')
        for failure in ['running','unacknowledged','failed','effect','request','redispatch']:
            r, b, e = copy.deepcopy(record), copy.deepcopy(backend), copy.deepcopy(effect)
            if failure=='running': r['status']['process']['finished']=False
            elif failure=='unacknowledged': b['acknowledged']=[]
            elif failure=='failed': b['results']['op']['event']['steps'][0]['process']['code']=1
            elif failure=='effect': e['payloadMatches']=False
            elif failure=='request': r['status']['operationRequestId']='other'
            elif failure=='redispatch': r['status']['attempts']=2
            with self.subTest(failure=failure), self.assertRaises(AssertionError):
                acceptance.validate_desktop_completion([r], b, e, 'request')
        record['status']['process']={'finished':True,'end':'cancelled','exitCode':None}
        backend['results']['op']['event']={'kind':'cancelled'}
        absent={key:False for key in effect}
        acceptance.validate_desktop_completion([record], backend, absent, 'request')
        with self.assertRaises(AssertionError):
            acceptance.validate_desktop_completion([record], backend, {**absent,'receiptPresent':True}, 'request')

    def test_known_macos_uncertainty_is_preserved_and_requires_independent_effect(self):
        record={'action':{'initiator':{'kind':'backend','attempt':'a'}},'status':{
            'operationRequestId':'r','attempts':1,'phase':'outcomeUnknown',
            'process':{'finished':True,'end':'exited','exitCode':0,'quiescent':False}}}
        event={'kind':'software_result','steps':[{'process':{'kind':'exited','code':0},
            'diagnostics':{'failure':'capture_failed'},'after':{'state':'unknown'}}]}
        backend={'startRequests':1,'results':{'op':{'attemptId':'a','event':event}},'acknowledged':['op']}
        effect={key:True for key in ('receiptPresent','payloadPresent','payloadMatches')}
        result=acceptance.validate_desktop_completion([record],backend,effect,'r')
        self.assertTrue(result['executionUncertain'])
        for change in ('phase','quiescent','effect','failure'):
            r,b,e=copy.deepcopy(record),copy.deepcopy(backend),copy.deepcopy(effect)
            if change=='phase': r['status']['phase']='completed'
            elif change=='quiescent':r['status']['process']['quiescent']=True
            elif change=='effect':e['payloadMatches']=False
            else:b['results']['op']['event']['steps'][0]['diagnostics']['failure']='cancelled'
            with self.subTest(change=change),self.assertRaises(AssertionError):
                acceptance.validate_desktop_completion([r],b,e,'r')

    def test_journal_proof_reads_capture_for_the_original_attempt_not_snapshot_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'journal.sqlite'
            db=sqlite3.connect(path)
            db.executescript('CREATE TABLE executions(request_id TEXT,snapshot BLOB); CREATE TABLE process_evidence(attempt_id TEXT,body BLOB);')
            snapshot={'attempts':1,'attempt':{'id':'local-attempt'}}
            capture={'attemptId':'local-attempt','finished':True,'end':'exited','exitCode':0,'contentDigest':'digest','stdout':[],'stderr':[]}
            db.execute('INSERT INTO executions VALUES(?,?)',('request',json.dumps(snapshot)))
            db.execute('INSERT INTO process_evidence VALUES(?,?)',('local-attempt',json.dumps(capture)))
            db.commit(); db.close()
            proof=acceptance.journal_proof(path)
            completion={'request':'request','record':{'status':{'attemptId':'local-attempt','process':{'end':'exited','exitCode':0}}}}
            acceptance.validate_journal_completion(proof,completion)
            self.assertNotIn('stdout',proof['records'][0]['process'])
            for bad_code in [1, None]:
                proof['records'][0]['process']['exitCode']=bad_code
                with self.assertRaises(AssertionError): acceptance.validate_journal_completion(proof,completion)
            proof['records'][0]['process']['exitCode']=None
            completion['record']['status']['process']['exitCode']=None
            proof['records'][0]['process']['end']='cancelled'
            completion['record']['status']['process']['end']='cancelled'
            acceptance.validate_journal_completion(proof,completion)
            proof['records'][0]['process']=None
            with self.assertRaises(AssertionError): acceptance.validate_journal_completion(proof,completion)

    def test_eof_or_failed_parent_cannot_finish_worker_as_passed(self):
        for finish in [None, {'status':'failed'}, {'status':'cancelled'}, {'status':'passed'}]:
            with self.assertRaises(AssertionError): acceptance.validate_desktop_finish(finish, None)
        acceptance.validate_desktop_finish({'status':'passed','request':'r'}, {'request':'r'})

    def test_native_authorization_cancel_has_closed_diagnostic_without_frozen_command(self):
        with tempfile.TemporaryDirectory() as directory:
            script=Path(directory)/'install.py'; script.write_text('sensitive implementation payload')
            process=MagicMock();process.poll.return_value=1
            with patch.object(acceptance.subprocess,'Popen',return_value=process):
                session=acceptance.AdministratorSession(script,script,script,script,Path(directory))
            session.log.write('execution error: User canceled. (-128)');session.log.flush()
            try:
                with self.assertRaisesRegex(acceptance.AuthorizationCancelled,'native administrator authorization cancelled') as caught:
                    session.start()
            finally: session.close()
            self.assertNotIn('sensitive',str(caught.exception))
            self.assertIn('User canceled', (script.parent/'administrator-session.log').read_text())

    def test_successful_connection_after_the_authorization_deadline_is_not_admitted(self):
        session=object.__new__(acceptance.AdministratorSession)
        connection=MagicMock();session.listener=MagicMock()
        session.listener.accept.return_value=(connection,None)
        session.connection=None;session.deadline=120;session.process=MagicMock()
        session.receive=lambda identity: None
        with patch.object(acceptance.time,'clock_gettime',return_value=121), patch.object(acceptance,'peer_identity',return_value=(1,0)):
            with self.assertRaisesRegex(RuntimeError,'authorization deadline exceeded'): session.start()
        connection.sendall.assert_not_called()

    def test_one_frozen_authorized_session_rejects_arbitrary_commands_and_cleans_on_disconnect(self):
        with tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            path=Path(directory)/'control'; marker=Path(directory)/'cleaned'
            listener=socket.socket(socket.AF_UNIX); listener.bind(str(path));listener.listen(1)
            programs={'setup':'pass','initialize':'raise RuntimeError("injected initialization failure")','restart':'pass','cleanup':f'from pathlib import Path;Path({str(marker)!r}).write_text("cleaned")'}
            worker=threading.Thread(target=acceptance.authorized_steps,args=(programs,str(path),os.getpid(),os.geteuid(),time.clock_gettime(time.CLOCK_MONOTONIC)+60))
            worker.start();connection,_=listener.accept();reader=connection.makefile('rb')
            connection.sendall(b'{"id":0,"operation":"setup"}\n')
            self.assertTrue(json.loads(reader.readline())['ok'])
            connection.sendall(b'{"id":1,"operation":"initialize"}\n')
            self.assertFalse(json.loads(reader.readline())['ok'])
            connection.sendall(b'{"id":2,"operation":"execute","path":"/bin/sh"}\n')
            self.assertFalse(json.loads(reader.readline())['ok'])
            reader.close();connection.close();listener.close();worker.join(5)
            self.assertFalse(worker.is_alive());self.assertEqual(marker.read_text(),'cleaned')

    def test_late_authorization_cannot_execute_setup_or_cleanup(self):
        with tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            path=Path(directory)/'control';marker=Path(directory)/'changed'
            listener=socket.socket(socket.AF_UNIX);listener.bind(str(path));listener.listen(1)
            change=f'from pathlib import Path;Path({str(marker)!r}).write_text("changed")'
            worker=threading.Thread(target=acceptance.authorized_steps,args=({'setup':change,'cleanup':change},str(path),os.getpid(),os.geteuid(),time.clock_gettime(time.CLOCK_MONOTONIC)-1))
            worker.start();connection,_=listener.accept();reader=connection.makefile('rb')
            connection.sendall(b'{"id":0,"operation":"setup"}\n')
            self.assertEqual(reader.readline(),b'')
            reader.close();connection.close();listener.close();worker.join(5)
            self.assertFalse(worker.is_alive());self.assertFalse(marker.exists())

    def test_independent_system_python_uses_the_same_authorization_deadline_clock(self):
        with tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            path=Path(directory)/'control';marker=Path(directory)/'changed'
            listener=socket.socket(socket.AF_UNIX);listener.bind(str(path));listener.listen(1);listener.settimeout(5)
            change=f'from pathlib import Path;Path({str(marker)!r}).write_text("changed")'
            source='import socket,ctypes,struct,io,contextlib,json,time\n'+inspect.getsource(acceptance.peer_identity)+inspect.getsource(acceptance.authorized_steps)
            programs={'setup':change,'cleanup':change}
            source+=f'authorized_steps({programs!r},{str(path)!r},{os.getpid()},{os.geteuid()},{time.clock_gettime(time.CLOCK_MONOTONIC)-1!r})'
            worker=subprocess.Popen(['/usr/bin/python3','-I','-c',source],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            connection,_=listener.accept();reader=connection.makefile('rb')
            connection.sendall(b'{"id":0,"operation":"setup"}\n')
            reply=reader.readline()
            reader.close();connection.close();listener.close();worker.communicate(timeout=5)
            self.assertEqual(reply,b'');self.assertFalse(marker.exists())

    def test_http_failure_is_not_acknowledgement(self):
        status = {'results': {'op': {'attemptId': 'a', 'event': {'kind': 'result'}}}, 'acknowledged': []}
        self.assertIsNone(acceptance.acknowledged_result(status, 'a'))
        status['acknowledged'].append('op')
        self.assertEqual(acceptance.acknowledged_result(status, 'a'), {'kind': 'result'})
        self.assertIsNone(acceptance.acknowledged_result(status, 'other'))
        status['results']['other-op'] = status['results']['op']
        status['acknowledged'].append('other-op')
        with self.assertRaises(AssertionError):
            acceptance.acknowledged_result(status, 'a')

    def test_failed_exit_wrong_output_or_failed_capture_cannot_pass(self):
        event = {'kind': 'result', 'exitCode': 0, 'output': {'fixture': 'system'},
                 'quality': 'partial', 'diagnostics': {'failure': None}}
        acceptance.validate_script(event, 'system')
        for field, value in [('exitCode', None), ('exitCode', 1), ('quality', 'failed'),
                             ('output', {'fixture': 'user'}), ('diagnostics', {'failure': 'launch_failed'})]:
            changed = copy.deepcopy(event)
            changed[field] = value
            with self.assertRaises(AssertionError):
                acceptance.validate_script(changed, 'system')


if __name__ == '__main__':
    unittest.main()
