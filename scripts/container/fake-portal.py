#!/usr/bin/env python3
"""A stand-in for the desktop portal's Settings (org.freedesktop.portal.Settings on the session bus),
for scripts/container/appearance-check.sh. It only knows the org.freedesktop.appearance color-scheme.

  fake-portal.py <scheme> [--legacy] [--then <scheme> <seconds>]

<scheme> is what ReadOne answers: 1 prefers dark, 2 prefers light, 0 no preference. With --then, the
portal emits SettingChanged with the second scheme after that many seconds and keeps answering the
first one, so a theme that arrives afterwards can only have come through the signal. With --legacy,
ReadOne is an unknown method and only the older Read answers (its value in a variant of a variant).
"""
import sys

from gi.repository import Gio, GLib

NAMESPACE = "org.freedesktop.appearance"
KEY = "color-scheme"
PATH = "/org/freedesktop/portal/desktop"
XML = """
<node>
  <interface name="org.freedesktop.portal.Settings">
    <method name="ReadOne">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <method name="Read">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <signal name="SettingChanged">
      <arg type="s" name="namespace"/>
      <arg type="s" name="key"/>
      <arg type="v" name="value"/>
    </signal>
    <property name="version" type="u" access="read"/>
  </interface>
</node>
"""


def main():
    args = sys.argv[1:]
    scheme = int(args.pop(0))
    legacy = "--legacy" in args
    then = None
    if "--then" in args:
        i = args.index("--then")
        then = (int(args[i + 1]), float(args[i + 2]))
    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]

    def on_call(conn, sender, path, iface, method, params, invocation):
        if method == "ReadOne" and not legacy:
            invocation.return_value(GLib.Variant("(v)", (GLib.Variant("u", scheme),)))
        elif method == "Read":
            inner = GLib.Variant("v", GLib.Variant("u", scheme))
            invocation.return_value(GLib.Variant("(v)", (inner,)))
        else:
            invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method)

    def on_bus(conn, name):
        conn.register_object(PATH, info, on_call, lambda *a: GLib.Variant("u", 2), None)
        if then:
            value, seconds = then

            def emit():
                body = GLib.Variant("(ssv)", (NAMESPACE, KEY, GLib.Variant("u", value)))
                conn.emit_signal(None, PATH, "org.freedesktop.portal.Settings", "SettingChanged", body)
                return False

            GLib.timeout_add(int(seconds * 1000), emit)

    Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.portal.Desktop", Gio.BusNameOwnerFlags.NONE, on_bus, None, None)
    GLib.MainLoop().run()


main()
