"""Exercise distribution guards, including Python's optimized execution mode."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class ReleasePrivacy(unittest.TestCase):
    def run_guard(self, payload, secret=None):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'fixture.bin'
            path.write_bytes(payload)
            env = os.environ.copy()
            env['UNDERTOW_FORBIDDEN_SECRETS'] = json.dumps([secret] if secret else [])
            return subprocess.run([sys.executable, '-O', str(Path(__file__).with_name('check-release.py')), str(path)], env=env, capture_output=True).returncode

    def test_known_credentials_are_rejected_in_both_encodings(self):
        secret = 'fixture-credential-for-guard-check-only'
        for encoding in ('utf-8', 'utf-16le'):
            self.assertNotEqual(self.run_guard(secret.encode(encoding), secret), 0)

    def test_machine_paths_are_rejected_in_both_utf16_alignments(self):
        path = 'C:' + chr(92) + 'Users' + chr(92) + 'example' + chr(92) + 'source.rs'
        self.assertNotEqual(self.run_guard(path.encode()), 0)
        for prefix in (b'', b'x'):
            self.assertNotEqual(self.run_guard(prefix + path.encode('utf-16le')), 0)

    def test_clean_bytes_are_accepted(self):
        self.assertEqual(self.run_guard(b'MZ test fixture with no credentials'), 0)


if __name__ == '__main__':
    unittest.main()
