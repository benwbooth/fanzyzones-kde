from support import *


input_process = subprocess.Popen([str(repo / 'target/integration/input')], stdin=subprocess.PIPE, text=True)


def events(commands):
    input_process.stdin.write(commands)
    input_process.stdin.flush()
    time.sleep(.15)


def driver(method, signature='', *args):
    command = ['busctl', '--user', '--json=short', 'call', 'com.benwbooth.FanzyZonesTest',
        '/Input', 'com.benwbooth.FanzyZonesTest', method]
    if signature:
        command.extend([signature, *map(str, args)])
    result = run(*command)
    time.sleep(.15)
    return json.loads(result)['data'][0] if result else None


def scope(value):
    return next(s for s in value['model']['scopes'].values() if s['output'] == 'Virtual-0')


value = settled()
assert value['nativeInput']
ids = {w['title'].removeprefix('Workspace fixture '): identity for identity, w in value['model']['windows'].items() if w['present']}
a, b, c = (ids[name] for name in ['Alpha', 'Beta', 'Gamma'])
action('float-all')
for identity in [a, b, c]:
    action('float', window=identity, display='Virtual-0')
action('dock', window=a, display='Virtual-0', edge='left')
action('dock', window=b, target=a, edge='bottom')
action('dock', window=c, target=b, edge='left')
value = action('show-all', display='Virtual-0')
before = scope(value)
g = info(a)
events(f"move {g['x']+g['width']/2} {g['y']+g['height']/2}\nkey 56 1\n")
driver('PinchBegin', 'i', 2)
for scale in [1.2, 1.7, 1.4, 4]:
    driver('PinchUpdate', 'd', scale)
    current = scope(status())['camera']['rect']
    assert abs(current['width'] - 1 / scale) < .0001, (scale, current)
driver('PinchEnd', 'b', 'false')
events('key 56 0\n')
value = settled()
assert scope(value)['camera'] != before['camera']
assert len(scope(value)['back']) == len(before['back']) + 1
assert_placements(value)
print('PASS continuous two-finger scale, reversal, best-fit settling, one history entry', flush=True)

action('show-all', display='Virtual-0')
g = info(a)
events(f"move {g['x']+80} {g['y']+80}\nkey 56 1\n")
before = scope(settled())['camera']
driver('PinchBegin', 'i', 2)
driver('PinchUpdate', 'd', 2.1)
driver('PinchEnd', 'b', 'true')
events('key 56 0\n')
assert scope(settled())['camera'] == before
driver('PinchBegin', 'i', 2)  # No Alt on an application: leave its gesture alone.
driver('PinchUpdate', 'd', 2)
driver('PinchEnd', 'b', 'false')
assert scope(settled())['camera'] == before
print('PASS cancelled pinch restores camera; unmodified application pinch passes through', flush=True)

events('move 1120 500\n')
driver('PinchBegin', 'i', 2)
driver('PinchUpdate', 'd', 2)
assert status()['animating']
driver('PinchEnd', 'b', 'false')
settled()
print('PASS unmodified pinch on bare Desktop', flush=True)

# Float a fixture with a native KWin title bar; ask KWin's decoration for the
# actual hit regions rather than assuming Breeze button positions.
action('float', window=a, display='Virtual-0')
g = info(a)
events(f"move {g['x']+g['width']/2} {g['y']+10}\n")
decoration = json.loads(driver('DecorationAtPointer'))
assert decoration['id'] == a and decoration.get('title'), decoration
t = decoration['title']
x, y = t['x'] + t['width']/2, t['y'] + t['height']/2
events(f'move {x} {y}\nbutton 272 1\nbutton 272 0\nwait 60\nbutton 272 1\nbutton 272 0\n')
assert scope(settled())['camera'] == {'window': a}
assert info(a)['width'] > 1200
action('back', display='Virtual-0')
g = info(a)
events(f"move {g['x']+g['width']/2} {g['y']+10}\n")
decoration = json.loads(driver('DecorationAtPointer'))
button = next(b for b in decoration['buttons'] if b['maximize'])
events(f"move {button['x']+button['width']/2} {button['y']+button['height']/2}\nbutton 272 1\nbutton 272 0\n")
value = settled()
assert info(a)['noBorder'], info(a)
assert_placements(value)
print('PASS native title double-click and maximize-button dock toggle', flush=True)
# Alt retains the native maximize action and its state survives reconfiguration.
action('float', window=a, display='Virtual-0')
g = info(a)
events(f"move {g['x']+g['width']/2} {g['y']+10}\n")
decoration = json.loads(driver('DecorationAtPointer'))
button = next(b for b in decoration['buttons'] if b['maximize'])
events(f"move {button['x']+button['width']/2} {button['y']+button['height']/2}\nkey 56 1\nbutton 272 1\nbutton 272 0\nkey 56 0\n")
assert info(a)['maximizeHorizontal'] and info(a)['maximizeVertical'], info(a)
action('save-view', slot=8, display='Virtual-0')
assert info(a)['maximizeHorizontal'], info(a)
set_window(a, 'w.setMaximize(false, false);')
assert not info(a)['maximizeHorizontal']
print('PASS Alt-click preserves native maximize and restore', flush=True)

print('GESTURE_INTEGRATION_PASS', flush=True)

input_process.stdin.close()
assert input_process.wait() == 0
