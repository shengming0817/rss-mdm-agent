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
                return SimpleNamespace(returncode=1 if args[1] == 'print' else 0)
            previous = os.umask(0)
            try:
                self.invoke(home, ['install', '--scope', 'user', '--binary', '/bin/sh'], run)
            finally:
                os.umask(previous)
            folder = home / 'Library/LaunchAgents'
            plist = folder / 'com.rss-mdm.agent.execution.user.plist'
            self.assertEqual(plist.stat().st_mode & 0o777, 0o600)
            self.assertEqual(folder.stat().st_mode & 0o777, 0o700)
            self.assertEqual((home / 'Library').stat().st_mode & 0o777, 0o700)
            self.assertTrue(any(command[1] == 'bootstrap' for command in calls))

    def test_remove_handles_missing_binary_and_already_absent_endpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            folder = home / 'Library/LaunchAgents'
            folder.mkdir(parents=True)
            label = 'com.rss-mdm.agent.execution.user'
            plist = folder / (label + '.plist')
            with plist.open('wb') as stream:
                plistlib.dump({'Label': label, 'ProgramArguments': ['/missing/rss-execution-service']}, stream)
            self.invoke(home, ['remove', '--scope', 'user', '--binary', '/missing/rss-execution-service'], lambda *args, **kwargs: SimpleNamespace(returncode=1))
            self.assertFalse(plist.exists())

    def test_status_does_not_require_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            self.invoke(Path(directory), ['status', '--scope', 'user'], lambda *args, **kwargs: SimpleNamespace(returncode=0))


if __name__ == '__main__':
    unittest.main()
