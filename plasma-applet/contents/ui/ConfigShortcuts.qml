import QtQuick
import QtQuick.Layouts

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami
import org.kde.kquickcontrols as KQC
import org.kde.plasma.plasma5support as P5Support

KCM.SimpleKCM {
    id: page

    readonly property string cli: "$HOME/.local/share/fanzyzones-kde/fanzyzones-kde"

    // [{ id, friendly, sequence }]
    property var shortcuts: []
    property int cliNonce: 0
    property var executableCallbacks: ({})

    Component.onCompleted: loadShortcuts()

    P5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            const cb = page.executableCallbacks[source];
            delete page.executableCallbacks[source];
            executable.disconnectSource(source);
            if (cb)
                cb((data["stdout"] || "").trim());
        }
    }

    function shellQuote(text) {
        return "'" + String(text).replace(/'/g, "'\\''") + "'";
    }

    function runCli(commandSuffix, onResult) {
        const source = page.cli + " " + commandSuffix + " #" + (page.cliNonce++);
        page.executableCallbacks[source] = onResult || function() {};
        executable.connectSource(source);
    }

    function loadShortcuts() {
        runCli("shortcuts", function(out) {
            try {
                page.shortcuts = JSON.parse(out);
            } catch (error) {
                // ignore malformed reply
            }
        });
    }

    function setShortcut(id, sequence) {
        runCli("set-shortcut " + shellQuote(id) + " " + shellQuote(sequence), null);
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
