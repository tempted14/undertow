"""Check a clean checkout for accidental runtime data before publishing."""
import json
import hashlib
import os
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parents[1]
excluded = {'.git', 'target', 'dist', 'artifacts', '__pycache__'}
forbidden = json.loads(os.environ.get('UNDERTOW_FORBIDDEN_EMAILS', '[]'))
errors = []
for path in root.rglob('*'):
    relative = path.relative_to(root)
    if not path.is_file() or excluded.intersection(relative.parts):
        continue
    # This checked-in, generated test tone is required by decoder regression tests.
    if relative.as_posix() == 'tests/fixtures/24bit-192khz.flac':
        if hashlib.sha256(path.read_bytes()).hexdigest() != '1fbbea80929f77f8f85eb8eaff3849c0b566776992589e33fd71399dcef0675a':
            errors.append(f'{relative}: generated fixture checksum changed')
        continue
    if path.suffix.lower() in {'.dpapi', '.exe', '.dll', '.zip', '.part', '.tmp', '.wav', '.flac', '.mp3', '.aac'}:
        errors.append(f'{relative}: runtime/build data')
        continue
    try:
        content = path.read_text(encoding='utf-8')
    except UnicodeError:
        continue
    if re.search(r'(?i)[a-z]:[\\/]+users[\\/]+', content):
        errors.append(f'{relative}: machine-specific path')
    if any(email and email.lower() in content.lower() for email in forbidden):
        errors.append(f'{relative}: forbidden personal address')
    if re.search(r'\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})\b', content):
        errors.append(f'{relative}: credential-shaped content')
    if path.suffix == '.json':
        try:
            value = json.loads(content)
        except json.JSONDecodeError:
            errors.append(f'{relative}: invalid JSON')
            continue
        def check(item):
            if isinstance(item, dict):
                for key, child in item.items():
                    if key.lower() in {'access_token', 'refresh_token', 'client_secret', 'password'} and child:
                        errors.append(f'{relative}: populated credential field')
                    check(child)
            elif isinstance(item, list):
                for child in item:
                    check(child)
        check(value)
if errors:
    print('\n'.join(errors))
    sys.exit(1)
print('Repository privacy checks passed; third-party attribution retained.')
