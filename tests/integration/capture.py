# /// script
# requires-python = ">=3.11"
# dependencies = ["dbus-next==0.2.3", "pillow>=11,<13"]
# ///
"""Capture an isolated test compositor through KWin's own screenshot API."""
import asyncio
import os
from pathlib import Path
import sys

from dbus_next import Message, MessageType
from dbus_next.aio import MessageBus
from PIL import Image


async def main():
    root, destination = Path(sys.argv[1]), Path(sys.argv[2])
    bus = await MessageBus(bus_address=(root / 'bus-address').read_text(), negotiate_unix_fd=True).connect()
    read_fd, write_fd = os.pipe()

    def read_all():
        with os.fdopen(read_fd, 'rb') as stream:
            return stream.read()

    reader = asyncio.get_running_loop().run_in_executor(None, read_all)
    # KWin 6.6 CaptureWorkspace paints the whole area with the first output's
    # SceneView. Fullscreen effects therefore require separate CaptureScreen
    # requests to inspect the other output's real render path.
    output = sys.argv[3] if len(sys.argv) > 3 else None
    reply = await bus.call(Message(destination='org.kde.KWin', path='/org/kde/KWin/ScreenShot2',
        interface='org.kde.KWin.ScreenShot2', member='CaptureScreen' if output else 'CaptureWorkspace',
        signature='sa{sv}h' if output else 'a{sv}h', body=[output, {}, 0] if output else [{}, 0], unix_fds=[write_fd]))
    os.close(write_fd)
    if reply.message_type == MessageType.ERROR:
        raise RuntimeError(f'{reply.error_name}: {reply.body}')
    metadata = {k: v.value for k, v in reply.body[0].items()}
    pixels = await reader
    width, height, stride = metadata['width'], metadata['height'], metadata['stride']
    image_format = metadata['format']
    if image_format in (4, 5, 6):
        image = Image.frombytes('RGBA', (width, height), pixels, 'raw', 'BGRX' if image_format == 4 else 'BGRA', stride)
    elif image_format in (16, 17, 18):
        image = Image.frombytes('RGBA', (width, height), pixels, 'raw', 'RGBX' if image_format == 16 else 'RGBA', stride)
    else:
        raise RuntimeError(f'Unsupported screenshot format: {metadata}')
    image.convert('RGB').save(destination)
    if output and not any(high for low, high in image.convert('RGB').getextrema()):
        raise AssertionError(f'{output} rendered entirely black')
    print(f'{destination}: {width}x{height}')
    bus.disconnect()


asyncio.run(main())
