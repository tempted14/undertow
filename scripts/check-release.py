"""Reject local Windows home paths and configured personal addresses in a release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re

parser = argparse.ArgumentParser()
parser.add_argument('executable', nargs='?', default='target/release/undertow.exe')
args = parser.parse_args()
data = Path(args.executable).read_bytes()
assert not re.search(rb'(?i)[a-z]:[\\/]+users[\\/]+', data), 'Local home path embedded in release'
for offset in (0, 1):
    assert not re.search(r'(?i)[a-z]:[\\/]+users[\\/]+', data[offset:].decode('utf-16le', errors='ignore')), 'UTF-16 local home path embedded in release'
for value in json.loads(os.environ.get('UNDERTOW_FORBIDDEN_EMAILS', '[]')):
    for encoding in ('utf-8', 'utf-16le'):
        assert value.lower().encode(encoding) not in data.lower(), 'Personal address embedded in release'
print('Release privacy checks passed; SHA256:', hashlib.sha256(data).hexdigest())
