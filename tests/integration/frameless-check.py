from support import *
import os

qml = os.environ.get('QML_BIN', 'qml')
log = (root / 'Frameless.log').open('w')
client = subprocess.Popen([qml, str(repo / 'tests/integration/Fixture.qml'), '--', 'Frameless'], stdout=log, stderr=log)
def events(commands):
    subprocess.run([str(repo / 'target/integration/input')], input=commands, text=True, check=True)
    time.sleep(.3)
    return settled()
try:
    identity = None
    for _ in range(60):
        value = settled()
        identity = next((i for i, w in value['model']['windows'].items() if w['present'] and w['title'] == 'Workspace fixture Frameless'), None)
        if identity: break
        time.sleep(.1)
    assert identity, (root / 'Frameless.log').read_text()
    value = action('float', window=identity, display='Virtual-0')
    g = info(identity)
    events(f"move {g['x']+70} {g['y']-14}\nbutton 272 1\nbutton 272 0\nwait 60\nbutton 272 1\nbutton 272 0\n")
    assert any(d['camera'] == {'window':identity} for d in settled()['displays'])
    assert info(identity)['width'] > 1200
    action('back', display='Virtual-0')
    g = info(identity)
    events(f"move {g['x']+g['width']-33} {g['y']-14}\nbutton 272 1\nbutton 272 0\n")
    assert_placements(settled())
    value = action('float', window=identity, display='Virtual-0')
    for _ in range(2):
        g = info(identity)
        events(f"move {g['x']+70} {g['y']-14}\nbutton 272 1\nmove {g['x']+100} {g['y']+30}\nmove 900 500\nbutton 272 0\n")
        assert abs(info(identity)['height']-g['height']) < 2, (g,info(identity))
    print('FRAMELESS_INTEGRATION_PASS: floating header, focus, dock, drag preserves size', flush=True)
finally:
    client.terminate()
    client.wait(timeout=5)
    log.close()
