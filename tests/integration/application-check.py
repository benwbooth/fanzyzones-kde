from support import *

value = settled()
a = next(identity for identity, w in value['model']['windows'].items()
    if w['title'] == 'Workspace fixture Alpha' and w['present'])
action('float', window=a, display='Virtual-0')
action('focus', window=a)
action('new-window', display='Virtual-0', window=a)
for _ in range(100):
    value = settled()
    dolphin = next((identity for identity, w in value['model']['windows'].items()
        if w['present'] and 'dolphin' in w['appId']), None)
    if dolphin:
        break
    time.sleep(.1)
assert dolphin, 'Dolphin did not open'
assert info(dolphin)['noBorder'] and not info(dolphin)['minimized']
assert not info(a)['minimized']
print('PASS real Dolphin launch and visible split from a focused view', flush=True)

app = value['model']['windows'][dolphin]['appId']
action('exclude-app', app=app)
assert not info(dolphin)['noBorder'] and not info(dolphin)['minimized']
action('include-app', app=app)
assert settled()['model']['windows'][dolphin]['present']
print('PASS exclude and include application', flush=True)
set_window(dolphin, 'w.closeWindow();')

action('dock', window=a, display='Virtual-0')
set_window(a, 'w.fullScreen = true;')
assert info(a)['fullscreen'] and not info(a)['minimized']
set_window(a, 'w.fullScreen = false;')
assert_placements(settled())
print('PASS fullscreen and return to workspace', flush=True)
