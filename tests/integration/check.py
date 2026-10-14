from support import *

initial = settled()
ids = {w['title'].removeprefix('Workspace fixture '): identity for identity, w in initial['model']['windows'].items() if w['present']}
assert set(ids) == {'Alpha', 'Beta', 'Gamma'}, ids
a, b, c = (ids[name] for name in ['Alpha', 'Beta', 'Gamma'])
action('float-all')
value = action('arrange', display='Virtual-0')
assert_placements(value)
assert all(info(identity)['noBorder'] for identity in ids.values())
before = {identity: info(identity) for identity in ids.values()}
print('PASS nested arrangement and native geometry', flush=True)

set_window(c, 'w.minimized = true;')
assert info(c)['minimized']
assert any(info(identity)['width'] != before[identity]['width'] or info(identity)['height'] != before[identity]['height'] for identity in [a, b])
set_window(c, 'w.minimized = false;')
for identity in ids.values():
    assert all(info(identity)[key] == before[identity][key] for key in ['x', 'y', 'width', 'height'])
print('PASS minimize reflow and slot restoration', flush=True)

value = action('dock', window=b, target=a, edge='center')
assert info(a)['minimized'] and not info(b)['minimized']
action('next-tab', window=b)
assert info(b)['minimized'] and not info(a)['minimized']
print('PASS tabs and tab switching', flush=True)

overview = info(a)
action('focus', window=a)
assert info(a)['width'] > 1200 and info(c)['minimized']
for slot in range(1, 10):
    action('save-view', display='Virtual-0', slot=slot)
action('show-all', display='Virtual-0')
action('load-view', display='Virtual-0', slot=9)
assert info(a)['width'] > 1200
action('back', display='Virtual-0')
assert all(info(a)[key] == overview[key] for key in ['x', 'y', 'width', 'height'])
action('forward', display='Virtual-0')
assert info(a)['width'] > 1200
print('PASS focus, nine saved views, back and forward', flush=True)

action('dock', window=c, display='Virtual-1')
assert info(c)['x'] >= 1280
action('focus', window=c)
assert info(c)['width'] > 1200
assert info(a)['width'] > 1200
print('PASS independent display cameras and cross-display docking', flush=True)

action('float', window=a, display='Virtual-0')
action('focus', window=a)
assert info(a)['width'] > 1200 and info(a)['noBorder']
action('focus', window=a)
assert info(a)['width'] < 1000 and not info(a)['noBorder']
print('PASS floating-window focus and return without docking', flush=True)

action('float-all')
assert all(not info(identity)['minimized'] and not info(identity)['noBorder'] for identity in ids.values())
action('disable')
assert all(not info(identity)['minimized'] and not info(identity)['noBorder'] for identity in ids.values())
print('PASS float-all and disable restore native windows', flush=True)
print('INTEGRATION_PASS', flush=True)
