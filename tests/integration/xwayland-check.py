from support import *
import os

assert os.environ.get('DISPLAY') == (root / 'x11-display').read_text()
qml = os.environ.get('QML_BIN', 'qml')
env = os.environ.copy()
env['QT_QPA_PLATFORM'] = 'xcb'
log = (root / 'XWayland.log').open('w')
client = subprocess.Popen([qml, str(repo / 'tests/integration/Fixture.qml'), '--', 'XWayland'], env=env, stdout=log, stderr=log)
try:
    identity = None
    for _ in range(60):
        value = settled()
        identity = next((i for i, w in value['model']['windows'].items() if w['present'] and w['title'] == 'Workspace fixture XWayland'), None)
        if identity: break
        time.sleep(.1)
    assert identity, (root / 'XWayland.log').read_text()
    original = info(identity)
    value = action('dock', window=identity, display='Virtual-0', edge='right')
    value = action('show-all', display='Virtual-0')
    assert_placements(value)
    alpha = next(i for i, w in value['model']['windows'].items() if w['present'] and w['title'] == 'Workspace fixture Alpha')
    value = action('dock', window=identity, target=alpha, edge='center')
    assert info(alpha)['minimized'] and not info(identity)['minimized']
    action('next-tab', window=identity)
    assert not info(alpha)['minimized'] and info(identity)['minimized']
    action('next-tab', window=alpha)
    value = action('focus', window=identity)
    assert info(identity)['width'] > 1200
    assert_placements(value)
    action('show-all', display='Virtual-0')
    set_window(identity, 'w.minimized = true;')
    assert info(identity)['minimized']
    set_window(identity, 'w.minimized = false;')
    value = action('float', window=identity)
    assert not info(identity)['minimized'] and not info(identity)['noBorder']
    print('XWAYLAND_INTEGRATION_PASS: dock, geometry, mixed-protocol tabs, focus, minimize/restore, float', flush=True)
finally:
    client.terminate()
    client.wait(timeout=5)
    log.close()
