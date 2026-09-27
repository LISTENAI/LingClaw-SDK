#!/usr/bin/env python3
"""Package a native simulator build with examples, documentation and notices."""
import hashlib
import plistlib
import shutil
import subprocess
import sys
import tarfile
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    platform, target = sys.argv[1:]
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=ROOT))
    version = next(p['version'] for p in metadata['packages'] if p['name'] == 'lingclaw-sdk')
    name = f'LingClaw-SDK-{version}-{platform}'
    stage = ROOT / 'target/packages' / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    binary = ROOT / 'target' / target / 'release' / ('lingclaw-sdk.exe' if platform.startswith('windows') else 'lingclaw-sdk')
    if platform.startswith('macos'):
        contents = stage / 'LingClaw Simulator.app/Contents'
        (contents / 'MacOS').mkdir(parents=True)
        shutil.copy2(binary, contents / 'MacOS/lingclaw-sdk')
        with (contents / 'Info.plist').open('wb') as out:
            plistlib.dump(dict(CFBundleName='LingClaw Simulator', CFBundleDisplayName='LingClaw Simulator',
                CFBundleIdentifier='com.listenai.lingclaw.simulator', CFBundleExecutable='lingclaw-sdk',
                CFBundlePackageType='APPL', CFBundleShortVersionString=version, NSHighResolutionCapable=True), out)
        subprocess.run(['codesign', '--force', '--sign', '-', str(contents.parent)], check=True)
    else:
        shutil.copy2(binary, stage / binary.name)
    for file in ['README.md', 'LICENSE', 'NOTICE', 'THIRD_PARTY.md', 'CONTRIBUTING.md']:
        shutil.copy2(ROOT / file, stage / file)
    for folder in ['docs', 'examples']:
        shutil.copytree(ROOT / folder, stage / folder)
    (stage / 'simulator').mkdir()
    shutil.copy2(ROOT / 'simulator/README.md', stage / 'simulator/README.md')
    for relative in ['simulator/assets/OFL.txt', 'simulator/vendor/lvgl/LICENCE.txt']:
        destination = stage / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    shutil.copy2(ROOT / 'THIRD_PARTY_NOTICES.html', stage / 'THIRD_PARTY_NOTICES.html')
    licenses = stage / 'licenses'
    licenses.mkdir()
    shutil.copy2(ROOT / 'simulator/assets/OFL.txt', licenses / 'FONT-OFL.txt')
    shutil.copy2(ROOT / 'simulator/vendor/lvgl/LICENCE.txt', licenses / 'LVGL-MIT.txt')
    notices = ROOT / 'THIRD_PARTY_NOTICES.html'
    if notices.exists():
        shutil.copy2(notices, licenses / notices.name)
    else:
        raise SystemExit('Generate THIRD_PARTY_NOTICES.html before packaging')
    out = ROOT / 'dist'
    out.mkdir(exist_ok=True)
    if platform.startswith('linux'):
        archive = out / f'{name}.tar.gz'
        with tarfile.open(archive, 'w:gz') as tgz:
            tgz.add(stage, arcname=name)
    else:
        archive = Path(shutil.make_archive(str(out / name), 'zip', stage.parent, name))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (out / f'{archive.name}.sha256').write_text(f'{digest}  {archive.name}\n')
    print(archive)


if __name__ == '__main__':
    main()
