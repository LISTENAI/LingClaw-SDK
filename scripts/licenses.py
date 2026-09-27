#!/usr/bin/env python3
"""Regenerate the checked-in license report with cargo-about 0.9.2."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
subprocess.run(['cargo', 'about', 'generate', '--locked', '--fail',
                'about.hbs', '-o', 'THIRD_PARTY_NOTICES.html'], cwd=root, check=True)
p = root / 'THIRD_PARTY_NOTICES.html'
p.write_text('\n'.join(line.rstrip() for line in p.read_text().splitlines()) + '\n')
