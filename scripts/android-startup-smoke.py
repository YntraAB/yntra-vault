"""Exercise signed release APK startup on a disposable Android emulator only.

Reproduce 0.2.4's blocked startup, upgrade without clearing data, then verify
restart and a clean installation. Never run this against a physical device.
"""
import argparse
from pathlib import Path
import re
import subprocess
import time
import xml.etree.ElementTree as ET

PACKAGE = 'com.yntravault.app'
BLOCKED = 'Saved app settings could not be loaded.'
parser = argparse.ArgumentParser()
parser.add_argument('--baseline', required=True, type=Path)
parser.add_argument('--baseline-mode', choices=['broken', 'healthy'], default='broken')
parser.add_argument('--apk', required=True, type=Path)
parser.add_argument('--output', default='android-smoke-results', type=Path)
args = parser.parse_args()
for apk in (args.baseline, args.apk):
    if not apk.is_file():
        raise SystemExit('Missing test APK')
args.output.mkdir(parents=True, exist_ok=True)


def command(*parts, timeout=30):
    return subprocess.check_output(parts, text=True, stderr=subprocess.STDOUT, timeout=timeout)


serial = command('adb', 'get-serialno').strip()
if not re.fullmatch(r'emulator-\d+', serial):
    raise SystemExit('This destructive fixture test requires a disposable emulator')


def adb(*parts, timeout=30):
    return command('adb', '-s', serial, *parts, timeout=timeout)


if adb('shell', 'getprop', 'ro.kernel.qemu').strip() != '1':
    raise SystemExit('Refusing to operate outside an Android emulator')


def launch():
    adb('shell', 'am', 'force-stop', PACKAGE)
    adb('shell', 'am', 'start', '-W', '-n', PACKAGE + '/.MainActivity')


def wait_for(phase, *expected, blocked=False):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            adb('shell', 'rm', '-f', '/sdcard/yntra-smoke.xml')
            adb('shell', 'uiautomator', 'dump', '/sdcard/yntra-smoke.xml')
            xml = adb('shell', 'cat', '/sdcard/yntra-smoke.xml')
            tree = ET.fromstring(xml)
            text = ' '.join(node.get('text', '') + ' ' + node.get('content-desc', '') for node in tree.iter())
            (args.output / (phase + '.xml')).write_text(xml, encoding='utf-8')
            if not blocked and BLOCKED in text:
                raise RuntimeError('Metadata bootstrap still blocks ' + phase)
            if all(label in text for label in expected):
                print('Passed Android startup phase:', phase, flush=True)
                return tree
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, ET.ParseError):
            pass  # Accessibility may not yet be ready after an activity launch.
        time.sleep(2)
    raise RuntimeError('Android UI did not reach expected state: ' + phase)


def ready(phase):
    return wait_for(phase, 'New Vault', 'Open File')


adb('install', '-g', str(args.baseline), timeout=120)
launch()
if args.baseline_mode == 'broken':
    wait_for('024-reproduced', BLOCKED, blocked=True)
else:
    ready('baseline-healthy')

# In-place installation deliberately retains the baseline's app data/lock file.
adb('install', '-r', '-g', str(args.apk), timeout=120)
launch()
tree = ready('updated-upgraded')
node = next(node for node in tree.iter() if node.get('text') == 'New Vault')
bounds = re.fullmatch(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', node.get('bounds', ''))
if not bounds:
    raise RuntimeError('Missing New Vault control bounds')
x1, y1, x2, y2 = map(int, bounds.groups())
adb('shell', 'input', 'tap', str((x1 + x2) // 2), str((y1 + y2) // 2))
wait_for('updated-interactive', 'Create New Vault', 'Vault Name')
launch()
ready('updated-restarted')

# Clear only this fixture application's data on the verified disposable emulator.
if 'Success' not in adb('shell', 'pm', 'clear', PACKAGE):
    raise RuntimeError('Could not reset the emulator fixture')
launch()
ready('updated-clean-install')
launch()
ready('updated-clean-restarted')
print(f'Android release startup regression passed: {args.baseline_mode} baseline, upgrade, interaction, clean start and restart.')
