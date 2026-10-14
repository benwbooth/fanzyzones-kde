from support import *

if len(sys.argv) > 2 and sys.argv[2] == 'save':
    action('save-view', display='Virtual-1', slot=7)
    time.sleep(.5)
    (root / 'restart-before.json').write_text(json.dumps(status()))
    print('Saved restart comparison; restart session.sh with this same directory')
else:
    before = json.loads((root / 'restart-before.json').read_text())
    after = settled()
    def assignments(value):
        return {w['title']: w['output'] for w in value['model']['windows'].values() if w['present']}
    assert assignments(before) == assignments(after)
    for key, scope in before['model']['scopes'].items():
        assert scope['camera'] == after['model']['scopes'][key]['camera']
        assert scope['views'] == after['model']['scopes'][key]['views']
    assert_placements(after)
    print('RESTART_PASS: window matching, output ownership, cameras, saved views and native geometry')
