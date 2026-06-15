import QtQuick
import QtQuick.Layouts

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami
import org.kde.kquickcontrols as KQC
import org.kde.plasma.workspace.dbus as DBus

KCM.SimpleKCM {
    id: page

    readonly property string dbusService: "com.benwbooth.FanzyZones"
    readonly property string dbusPath: "/com/benwbooth/FanzyZones"
    readonly property string dbusInterface: "com.benwbooth.FanzyZones"

    // [{ id, friendly, sequence }]
    property var shortcuts: []

    Component.onCompleted: loadShortcuts()

    function backendCall(member, args, signature, onResult) {
        const pending = DBus.SessionBus.asyncCall({
            "service": dbusService,
            "path": dbusPath,
            "iface": dbusInterface,
            "member": member,
            "arguments": args || [],
            "signature": signature || "()"
        });
        pending.finished.connect(function() {
            if (pending.isError)
                return;
            const values = pending.values || [];
            const json = values.length > 0 ? values[0] : pending.value;
            if (onResult) {
                try {
                    onResult(JSON.parse(String(json)));
                } catch (error) {
                    // ignore malformed reply
                }
            }
        });
    }

    function loadShortcuts() {
        backendCall("Shortcuts", [], "()", function(list) {
            page.shortcuts = list;
        });
    }

    function setShortcut(id, sequence) {
        // Fire-and-forget; the KeySequenceItem already shows the new value.
        backendCall("SetShortcut", [id, sequence], "(ss)", null);
    }

    Kirigami.FormLayout {
        Repeater {
            model: page.shortcuts

            delegate: KQC.KeySequenceItem {
                required property var modelData
                Kirigami.FormData.label: modelData.friendly + ":"
                keySequence: modelData.sequence
                onCaptureFinished: page.setShortcut(modelData.id, keySequence.toString())
            }
        }
    }
}
