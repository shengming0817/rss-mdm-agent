"""Filesystem/CLI tests; launchctl is controlled and no system service is installed."""
import importlib.util
import os
from pathlib import Path
import plistlib
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('execution_install', Path(__file__).with_name('execution-macos.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class Installation(unittest.TestCase):
    def invoke(self, home, args, callback):
        with patch.object(Path, 'home', return_value=home), patch('sys.argv', ['execution-macos.py', *args]), patch.object(module.subprocess, 'run', side_effect=callback):
            module.main()

    def test_permissive_umask_does_not_publish_writable_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            calls = []
            def run(args, **kwargs):
                calls.append(args)
                return SimpleNamespace(returncode=113 if args[1] == 'print' else 0)
            previous = os.umask(0)
            try:
                self.invoke(home, ['install', '--scope', 'user', '--binary', '/bin/sh'], run)
            finally:
                os.umask(previous)
            folder = home / 'Library/LaunchAgents'
            plist = folder / 'com.rss-mdm.agent.execution.user.plist'
            self.assertEqual(plist.stat().st_mode & 0o777, 0o600)
            with plist.open('rb') as stream:
                config = plistlib.load(stream)
            self.assertEqual(config['ProgramArguments'], ['/bin/sh', '--user-helper'])
            self.assertEqual(config['ExitTimeOut'], 10)
            self.assertEqual(folder.stat().st_mode & 0o777, 0o700)
            self.assertEqual((home / 'Library').stat().st_mode & 0o777, 0o700)
            self.assertTrue(any(command[1] == 'bootstrap' for command in calls))

    def test_failed_plist_publication_rolls_back_owned_file(self):
        for target in ['dump', 'fsync']:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as directory:
                home = Path(directory)
                owner = module.plistlib if target == 'dump' else module.os
                with patch.object(owner, target, side_effect=OSError('injected publication failure')):
                    with self.assertRaises(OSError):
                        self.invoke(home, ['install', '--scope', 'user', '--binary', '/bin/sh'], lambda *args, **kwargs: SimpleNamespace(returncode=113))
                self.assertFalse((home / 'Library/LaunchAgents/com.rss-mdm.agent.execution.user.plist').exists())

    def test_remove_handles_missing_binary_and_already_absent_endpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            folder = home / 'Library/LaunchAgents'
            folder.mkdir(parents=True)
            label = 'com.rss-mdm.agent.execution.user'
            plist = folder / (label + '.plist')
            with plist.open('wb') as stream:
                plistlib.dump({'Label': label, 'ProgramArguments': ['/missing/rss-execution-service', '--user-helper']}, stream)
            self.invoke(home, ['remove', '--scope', 'user', '--binary', '/missing/rss-execution-service'], lambda *args, **kwargs: SimpleNamespace(returncode=113))
            self.assertFalse(plist.exists())

    def test_uncertain_bootstrap_compensates_before_deleting_configuration(self):
        for rollback_fails in [False, True]:
            with self.subTest(rollback_fails=rollback_fails), tempfile.TemporaryDirectory() as directory:
                home = Path(directory)
                calls = []
                def run(args, **kwargs):
                    calls.append((args, kwargs))
                    if args[1] == 'print':
                        return SimpleNamespace(returncode=113)
                    self.assertTrue(kwargs.get('check'))
                    if args[1] == 'bootstrap' or rollback_fails:
                        raise module.subprocess.CalledProcessError(5, args)
                    return SimpleNamespace(returncode=0)
                with self.assertRaises(module.subprocess.CalledProcessError):
                    self.invoke(home, ['install', '--scope', 'user', '--binary', '/bin/sh'], run)
                endpoint = f'gui/{os.geteuid()}/com.rss-mdm.agent.execution.user'
                self.assertEqual(calls[0], (['/bin/launchctl', 'print', endpoint], {'capture_output': True}))
                self.assertEqual(calls[1], (['/bin/launchctl', 'bootstrap', f'gui/{os.geteuid()}', str(home / 'Library/LaunchAgents/com.rss-mdm.agent.execution.user.plist')], {'check': True}))
                self.assertEqual(calls[-1], (['/bin/launchctl', 'bootout', endpoint], {'check': True}))
                self.assertEqual((home / 'Library/LaunchAgents/com.rss-mdm.agent.execution.user.plist').exists(), rollback_fails)

    def test_unknown_lookup_result_cannot_remove_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            home=Path(directory)
            folder=home / 'Library/LaunchAgents'; folder.mkdir(parents=True)
            plist=folder / 'com.rss-mdm.agent.execution.user.plist'
            with plist.open('wb') as stream:
                plistlib.dump({'Label':'com.rss-mdm.agent.execution.user','ProgramArguments':['/missing/service', '--user-helper']},stream)
            with self.assertRaises(module.subprocess.CalledProcessError):
                self.invoke(home,['remove','--scope','user','--binary','/missing/service'],lambda *args,**kwargs:SimpleNamespace(returncode=5))
            self.assertTrue(plist.exists())

    def test_status_does_not_require_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            def run(args, **kwargs):
                self.assertEqual(args, ['/bin/launchctl', 'print', f'gui/{os.geteuid()}/com.rss-mdm.agent.execution.user'])
                self.assertEqual(kwargs, {'check': True})
                return SimpleNamespace(returncode=0)
            self.invoke(Path(directory), ['status', '--scope', 'user'], run)


if __name__ == '__main__':
    unittest.main()
