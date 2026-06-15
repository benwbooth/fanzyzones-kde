import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Dialogs as QtDialogs

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami
import org.kde.plasma.plasma5support as P5Support

KCM.SimpleKCM {
    id: page

    readonly property string cli: "$HOME/.local/share/fanzyzones-kde/fanzyzones-kde"

    property var settings: ({})
    // Suppress change handlers while we populate controls from the CLI.
    property bool loading: true
    // Read by the config dialog to enable the Apply button.
    property bool unsavedChanges: false
    property int cliNonce: 0
    property var executableCallbacks: ({})

    Component.onCompleted: loadState()

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

    function loadState() {
        loading = true;
        runCli("state-json", function(out) {
            try {
                const state = JSON.parse(out);
                if (state.settings !== undefined) {
                    page.settings = state.settings;
                    page.applyToControls(state.settings);
                }
            } catch (error) {
                // ignore malformed state
            }
            loading = false;
            page.unsavedChanges = false;
        });
    }

    function applyToControls(s) {
        const mode = s.snap_mode || "modifier";
        snapAuto.checked = mode === "auto";
        snapShiftDrag.checked = mode !== "auto";
        gapSpin.value = s.gap || 0;
        paddingSpin.value = s.outer_padding || 0;
        shortcutsCheck.checked = s.keyboard_shortcuts_enabled !== false;
        zoneNumbersCheck.checked = s.show_zone_numbers !== false;
        layoutPickerCheck.checked = s.enable_zone_selector === true;
        layoutBottomUpCheck.checked = s.layout_menu_bottom_up !== false;
        opacitySlider.value = s.overlay_opacity !== undefined ? s.overlay_opacity : 0.35;
        if (s.highlight_color)
            colorSwatch.color = Qt.rgba(s.highlight_color.red, s.highlight_color.green, s.highlight_color.blue, 1);
    }

    // Mark the page dirty so the dialog enables Apply; nothing is written yet.
    function markDirty() {
        if (!loading)
            page.unsavedChanges = true;
    }

    // Called by the config dialog when Apply/OK is pressed.
    function saveConfig() {
        const patch = {
            "snap_mode": snapAuto.checked ? "auto" : "modifier",
            "gap": gapSpin.value,
            "outer_padding": paddingSpin.value,
            "keyboard_shortcuts_enabled": shortcutsCheck.checked,
            "show_zone_numbers": zoneNumbersCheck.checked,
            "enable_zone_selector": layoutPickerCheck.checked,
            "layout_menu_bottom_up": layoutBottomUpCheck.checked,
            "overlay_opacity": opacitySlider.value,
            "highlight_color": {
                "red": colorSwatch.color.r,
                "green": colorSwatch.color.g,
                "blue": colorSwatch.color.b
            }
        };
        runCli("invoke-action " + shellQuote(JSON.stringify({
            "action": "updateSettings",
            "patch": patch,
            "closeMenu": false
        })), null);
        page.unsavedChanges = false;
    }

    Kirigami.FormLayout {
        QQC2.ButtonGroup { id: snapGroup }

        QQC2.RadioButton {
            id: snapShiftDrag
            Kirigami.FormData.label: i18n("Snap mode:")
            QQC2.ButtonGroup.group: snapGroup
            text: i18n("Hold Shift and drag (KWin tiling)")
            onToggled: page.markDirty()
        }

        QQC2.RadioButton {
            id: snapAuto
            QQC2.ButtonGroup.group: snapGroup
            text: i18n("Auto-snap on drag (no modifier)")
            onToggled: page.markDirty()
        }

        QQC2.Label {
            text: i18n("Shift+drag always tiles into the active layout across every\nmonitor using KWin's built-in tiling. Auto-snap instead shows\nthe FanzyZones overlay and snaps on any drag, no key held.")
            wrapMode: Text.WordWrap
            opacity: 0.7
        }

        Item { Kirigami.FormData.isSection: true }

        QQC2.SpinBox {
            id: gapSpin
            Kirigami.FormData.label: i18n("Gap between zones (px):")
            from: 0
            to: 200
            onValueModified: page.markDirty()
        }

        QQC2.SpinBox {
            id: paddingSpin
            Kirigami.FormData.label: i18n("Outer padding (px):")
            from: 0
            to: 200
            onValueModified: page.markDirty()
        }

        Item { Kirigami.FormData.isSection: true }

        QQC2.CheckBox {
            id: shortcutsCheck
            Kirigami.FormData.label: i18n("Keyboard shortcuts:")
            text: i18n("Enable global shortcuts")
            onToggled: page.markDirty()
        }

        QQC2.CheckBox {
            id: zoneNumbersCheck
            Kirigami.FormData.label: i18n("Overlay:")
            text: i18n("Show zone numbers")
            onToggled: page.markDirty()
        }

        QQC2.CheckBox {
            id: layoutPickerCheck
            text: i18n("Show layout picker bar while dragging (auto-snap)")
            onToggled: page.markDirty()
        }

        QQC2.CheckBox {
            id: layoutBottomUpCheck
            Kirigami.FormData.label: i18n("Layout menu:")
            text: i18n("Active (most recent) layout at the bottom")
            onToggled: page.markDirty()
        }

        RowLayout {
            Kirigami.FormData.label: i18n("Overlay opacity:")
            QQC2.Slider {
                id: opacitySlider
                from: 0.05
                to: 0.95
                stepSize: 0.05
                Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                onMoved: page.markDirty()
            }
            QQC2.Label { text: Math.round(opacitySlider.value * 100) + "%" }
        }

        RowLayout {
            Kirigami.FormData.label: i18n("Highlight color:")

            Rectangle {
                id: colorSwatch
                implicitWidth: Kirigami.Units.gridUnit * 3
                implicitHeight: Kirigami.Units.gridUnit * 1.4
                radius: 3
                color: Qt.rgba(0.18, 0.48, 0.96, 1)
                border.color: Kirigami.Theme.textColor
                border.width: 1

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: colorDialog.open()
                }
            }

            QQC2.Button {
                text: i18n("Choose…")
                onClicked: colorDialog.open()
            }
        }
    }

    QtDialogs.ColorDialog {
        id: colorDialog
        selectedColor: colorSwatch.color
        onAccepted: {
            colorSwatch.color = selectedColor;
            page.markDirty();
        }
    }
}
