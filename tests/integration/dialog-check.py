from support import *

value = status()
ids = {w['title']: identity for identity, w in value['model']['windows'].items() if w['present']}
a, b = ids['Workspace fixture Alpha'], ids['Workspace fixture Beta']
action('float', window=a)
action('dock', window=a, target=b, edge='center')
action('focus', window=a)
subprocess.run([str(repo / 'target/integration/input')],
    input='key 29 1\nkey 50 1\nkey 50 0\nkey 29 0\nwait 400\n', text=True, check=True)
value = settled()
assert value['dialogs'], 'The fixture modal did not open'
dialog = value['dialogs'][0]['id']
assert not info(dialog)['minimized']
action('next-tab', window=a)
assert info(a)['minimized'] and info(dialog)['minimized']
action('next-tab', window=b)
assert not info(a)['minimized'] and not info(dialog)['minimized']
set_window(dialog, 'w.closeWindow();')
print('DIALOG_PASS: modal follows tab visibility and restoration')
