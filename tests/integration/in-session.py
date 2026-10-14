#!/usr/bin/env python3
"""Execute a command against the private compositor, never the user's session."""
import os
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
env = os.environ.copy()
env.update(
    DBUS_SESSION_BUS_ADDRESS=(root / 'bus-address').read_text(),
    WAYLAND_DISPLAY=(root / 'socket').read_text(),
    XDG_CONFIG_HOME=str(root / 'config'),
    XDG_DATA_HOME=str(root / 'data'),
    XDG_CACHE_HOME=str(root / 'cache'),
    XDG_STATE_HOME=str(root / 'state'),
    QT_QPA_PLATFORM='wayland',
    QT_LOGGING_TO_CONSOLE='1',
    QT_NO_XDG_DESKTOP_PORTAL='1',
    QT_QUICK_CONTROLS_STYLE='Basic',
)
if (root / 'runtime').exists():
    env['XDG_RUNTIME_DIR'] = str(root / 'runtime')
os.execvpe(sys.argv[2], sys.argv[2:], env)
