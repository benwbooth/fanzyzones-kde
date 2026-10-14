from support import *

value = status()
ids = {w['title']: identity for identity, w in value['model']['windows'].items() if w['present']}
a = ids['Workspace fixture Alpha']
action('float', window=a, display='Virtual-1')
action('dock', window=a, display='Virtual-1')
action('focus', window=a)
action('save-view', display='Virtual-1', slot=8)
before = settled()
run('kscreen-doctor', 'output.Virtual-1.disable')
time.sleep(.5)
value = settled()
assert [d['name'] for d in value['displays']] == ['Virtual-0']
assert value['model']['windows'][a]['output'] == 'Virtual-1'
assert not info(a)['minimized'] and info(a)['x'] < 1280
run('kscreen-doctor', 'output.Virtual-1.enable')
time.sleep(.5)
value = settled()
assert info(a)['x'] >= 1280
old = next(d for d in before['displays'] if d['name'] == 'Virtual-1')
new = next(d for d in value['displays'] if d['name'] == 'Virtual-1')
assert old['camera'] == new['camera'] and old['views'] == new['views']
print('PASS output disable, reachable fallback and reconnect restoration', flush=True)

run('kscreen-doctor', 'output.Virtual-1.scale.1.25')
time.sleep(.5)
value = settled()
assert_placements(value)
assert 1000 < info(a)['width'] < 1040, info(a)
run('kscreen-doctor', 'output.Virtual-1.scale.1')
time.sleep(.5)
assert_placements(settled())
assert info(a)['width'] > 1200
print('OUTPUT_PASS: fractional scale and native logical geometry', flush=True)
