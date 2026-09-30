#!/usr/bin/env python3
"""Ensure the OS acceptance harness cannot accept failed or unacknowledged results."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('acceptance', Path(__file__).with_name('verify-execution-macos.py'))
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class EvidenceTests(unittest.TestCase):
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
