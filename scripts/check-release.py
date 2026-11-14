"""Reject local paths and configured credentials in a distributed executable."""
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
def require(condition, message):
    if not condition:
        raise SystemExit(message)
require(not re.search(rb'(?i)[a-z]:[\\/]+users[\\/]+', data), 'Local home path embedded in release')
for offset in (0, 1):
    require(not re.search(r'(?i)[a-z]:[\\/]+users[\\/]+', data[offset:].decode('utf-16le', errors='ignore')), 'UTF-16 local home path embedded in release')
for value in json.loads(os.environ.get('UNDERTOW_FORBIDDEN_EMAILS', '[]')):
    for encoding in ('utf-8', 'utf-16le'):
        require(value.lower().encode(encoding) not in data.lower(), 'Personal address embedded in release')
secrets = json.loads(os.environ.get('UNDERTOW_FORBIDDEN_SECRETS', '[]'))
secrets += [value for name, value in os.environ.items() if 'TIDAL' in name.upper() and any(part in name.upper() for part in ['SECRET', 'API_KEY', 'DEVELOPER_KEY']) and len(value) > 8]
for value in secrets:
    if value:
        for encoding in ('utf-8', 'utf-16le'):
            require(value.encode(encoding) not in data, 'Configured credential embedded in release')
require(b'-----BEGIN PRIVATE KEY-----' not in data and b'-----BEGIN RSA PRIVATE KEY-----' not in data, 'Private key embedded in release')
print('Release privacy checks passed; SHA256:', hashlib.sha256(data).hexdigest())
