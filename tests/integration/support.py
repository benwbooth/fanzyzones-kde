#!/usr/bin/env python3
"""Exercise the real KWin effect and independently verify native window state.

Run through in-session.py, with session.sh's private compositor and fixtures.
"""
import json
from pathlib import Path
import subprocess
import sys
import time

root = Path(sys.argv[1]).resolve()
repo = Path(__file__).resolve().parents[2]
assert str(root / 'runtime') == __import__('os').environ['XDG_RUNTIME_DIR']
assert (root / 'bus-address').read_text() == __import__('os').environ['DBUS_SESSION_BUS_ADDRESS']
cli = str(repo / 'target/debug/fanzyzones-kde')


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def status():
    return json.loads(run(cli, 'workspace', 'status'))


def settled():
    for _ in range(100):
        value = status()
        assert not value.get('error'), value
        if value.get('ready') and not value.get('animating'):
            return value
        time.sleep(.05)
    raise AssertionError('workspace did not settle')


def action(command, **options):
    args = [cli, 'workspace', command]
    for key, value in options.items():
        args.extend(['--' + key.replace('_', '-'), str(value)])
    run(*args)
    return settled()


def info(identity):
    response = json.loads(run('busctl', '--user', '--json=short', 'call', 'org.kde.KWin',
        '/KWin', 'org.kde.KWin', 'getWindowInfo', 's', identity))
    return {key: value['data'] for key, value in response['data'][0].items()}


def script(source):
    path = root / 'control.js'
    path.write_text(source)
    identifier = json.loads(run('busctl', '--user', '--json=short', 'call', 'org.kde.KWin',
        '/Scripting', 'org.kde.kwin.Scripting', 'loadScript', 'ss', str(path), 'parity-test'))['data'][0]
    run('busctl', '--user', 'call', 'org.kde.KWin', f'/Scripting/Script{identifier}',
        'org.kde.kwin.Script', 'run')
    time.sleep(.35)
    run('busctl', '--user', 'call', 'org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting',
        'unloadScript', 's', 'parity-test')


def set_window(identity, code):
    script(f'const w = workspace.stackingOrder.find(w => w.internalId.toString() === {json.dumps(identity)}); {code}')
    return settled()


def assert_placements(value):
    for identity, geometry in value['geometry'].items():
        actual = info(identity)
        assert not actual['minimized'], (identity, actual)
        for key in ['x', 'y', 'width', 'height']:
            assert abs(actual[key] - geometry[key]) < 2, (identity, key, actual[key], geometry[key])
