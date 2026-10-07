# Asks an X window to close the way a window manager's close button does, with
# a WM_DELETE_WINDOW message, which xdotool cannot send.
#
#   python3 close-window.py WINDOW
import ctypes
import sys

x11 = ctypes.CDLL("libX11.so.6")
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
x11.XInternAtom.restype = ctypes.c_ulong
x11.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]


class ClientMessage(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("message_type", ctypes.c_ulong),
        ("format", ctypes.c_int),
        ("data", ctypes.c_long * 5),
    ]


class Event(ctypes.Union):
    _fields_ = [("client", ClientMessage), ("pad", ctypes.c_long * 24)]


CLIENT_MESSAGE = 33

display = x11.XOpenDisplay(None)
if not display:
    sys.exit("close-window.py: cannot open the display")
window = int(sys.argv[1], 0)
event = Event()
event.client.type = CLIENT_MESSAGE
event.client.window = window
event.client.message_type = x11.XInternAtom(display, b"WM_PROTOCOLS", 0)
event.client.format = 32
event.client.data[0] = x11.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
x11.XSendEvent(display, window, 0, 0, ctypes.byref(event))
x11.XFlush(display)
x11.XCloseDisplay(display)
