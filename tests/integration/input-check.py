from support import *


def events(commands):
    subprocess.run([str(repo / 'target/integration/input')], input=commands, text=True, check=True)
    time.sleep(.4)
    return settled()


value = settled()
assert value['nativeInput'], 'Build and install the matching native bridge first'
ids = {w['title'].removeprefix('Workspace fixture '): identity for identity, w in value['model']['windows'].items() if w['present']}
a, b, c = (ids[name] for name in ['Alpha', 'Beta', 'Gamma'])
action('float-all')
# Set a known left-hand tree so pointer coordinates do not depend on process
# launch order or on the order of windows restored by earlier checks.
for identity in [a, b, c]:
    action('float', window=identity, display='Virtual-0')
action('dock', window=a, display='Virtual-0', edge='left')
action('dock', window=b, target=a, edge='bottom')
value = action('dock', window=c, target=b, edge='left')

# Native Meta+Shift drag, including paired button/modifier release.
value = events('move 80 80\nkey 125 1\nkey 42 1\nbutton 272 1\nmove 500 380\nbutton 272 0\nkey 42 0\nkey 125 0\n')
camera = next(d for d in value['displays'] if d['name'] == 'Virtual-0')['camera']['rect']
assert abs(camera['width'] - 420 / 1280) < .001, camera
assert abs(camera['height'] - 300 / 800) < .001, camera
print('PASS native Meta+Shift drag framing', flush=True)

# Escape cancels framing and releases the grab cleanly.
before = camera
value = events('move 90 90\nkey 125 1\nkey 42 1\nbutton 272 1\nmove 400 300\nkey 1 1\nkey 1 0\nbutton 272 0\nkey 42 0\nkey 125 0\n')
assert next(d for d in value['displays'] if d['name'] == 'Virtual-0')['camera']['rect'] == before
print('PASS Escape cancels rectangle framing', flush=True)

# Use the real global shortcut to return to the overview.
value = events('key 29 1\nkey 56 1\nkey 57 1\nkey 57 0\nkey 56 0\nkey 29 0\n')
scope = next(s for s in value['model']['scopes'].values() if s['output'] == 'Virtual-0')
assert scope['camera'] == {'node': scope['root']['id']}
print('PASS global keyboard shortcut', flush=True)

# Drag the main seam using the actual chrome window.
before = info(a)
events('move 640 200\nbutton 272 1\nmove 800 200\nbutton 272 0\n')
assert info(a)['width'] > before['width'] + 60, (before, info(a))
print('PASS pointer seam resizing', flush=True)

# Native title double-click focuses the space; typing reaches the real client.
g = info(a)
x, y = int(g['x'] + 70), int(g['y'] - 15)
events(f'move {x} {y}\nbutton 272 1\nbutton 272 0\nwait 70\nbutton 272 1\nbutton 272 0\n')
assert info(a)['width'] > 1200
events('move 100 150\nbutton 272 1\nbutton 272 0\nkey 45 1\nkey 45 0\n')
assert 'FIXTURE_INPUT Alpha' in (root / 'Alpha.log').read_text()
print('PASS title double-click focus and real client keyboard input', flush=True)

action('show-all', display='Virtual-0')
events('move 1120 500\nbutton 272 1\nbutton 272 0\nwait 70\nbutton 272 1\nbutton 272 0\n')
value = settled()
scope = next(s for s in value['model']['scopes'].values() if s['output'] == 'Virtual-0')
def desktop(node):
    if node['kind'] == 'desktop': return node
    return next((value for child in node.get('children', []) if (value := desktop(child))), None)
assert scope['camera']['node'] == desktop(scope['root'])['id']
print('PASS bare Desktop double-click', flush=True)

action('show-all', display='Virtual-0')
ga, gb = info(a), info(b)
events(f"move {ga['x'] + 60} {ga['y'] - 14}\nbutton 272 1\nmove {ga['x'] + 90} {ga['y'] + 20}\nmove {gb['x'] + gb['width']/2} {gb['y'] + gb['height']/2}\nwait 450\nbutton 272 0\n")
assert info(b)['minimized'] and not info(a)['minimized']
g = info(a)
events(f"move {g['x'] + 30} {g['y'] - 14}\nbutton 272 1\nbutton 272 0\n")
assert not info(b)['minimized'] and info(a)['minimized']
print('PASS pointer dwell creates tabs and pointer selects a tab', flush=True)

# Tear off the second tab onto the floating Desktop.
g = info(b)
events(f"move {g['x'] + (g['width'] - 68)*.75} {g['y'] - 14}\nbutton 272 1\nmove {g['x'] + (g['width'] - 68)*.75} {g['y'] + 30}\nmove 1120 500\nwait 450\nbutton 272 0\n")
assert not info(a)['minimized'] and not info(a)['noBorder']
print('PASS pointer tab tear-off to Desktop', flush=True)

action('dock', window=a, target=b, edge='center')
g = info(a)
events(f"move {g['x'] + 40} {g['y'] - 14}\nkey 42 1\nbutton 272 1\nmove {g['x'] + 70} {g['y'] + 30}\nmove 2550 350\nwait 100\nbutton 272 0\nkey 42 0\n")
value = settled()
assert value['model']['windows'][a]['output'] == 'Virtual-1'
assert value['model']['windows'][b]['output'] == 'Virtual-1'
assert info(a)['x'] >= 1280
print('PASS Shift-drag moves the tab group across displays', flush=True)
print('INPUT_INTEGRATION_PASS', flush=True)
