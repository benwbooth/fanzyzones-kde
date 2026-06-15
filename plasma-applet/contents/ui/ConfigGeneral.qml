import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Dialogs as QtDialogs

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami
import org.kde.plasma.workspace.dbus as DBus

KCM.SimpleKCM {
    id: page

    readonly property string dbusService: "com.benwbooth.FanzyZones"
    readonly property string dbusPath: "/com/benwbooth/FanzyZones"
    readonly property string dbusInterface: "com.benwbooth.FanzyZones"

    property var settings: ({})
    // Suppress change handlers while we populate controls from the daemon.
    property bool loading: true

    Component.onCompleted: loadState()

    function backendCall(member, args, signature, onSuccess) {
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
            const stateJson = values.length > 0 ? values[0] : pending.value;
            try {
                const state = JSON.parse(String(stateJson));
                if (state.settings !== undefined && onSuccess)
                    onSuccess(state.settings);
            } catch (error) {
                // ignore malformed state
            }
        });
    }

    function loadState() {
        loading = true;
        backendCall("State", [], "()", function(s) {
            page.settings = s;
            page.applyToControls(s);
            loading = false;
        });
    }

    function applyToControls(s) {
        const mode = s.snap_mode || "modifier";
        snapModifier.checked = mode === "modifier";
        snapAuto.checked = mode === "auto";
        const mods = s.modifiers || ["shift"];
        modShift.checked = mods.indexOf("shift") >= 0;
        modCtrl.checked = mods.indexOf("control") >= 0;
        modAlt.checked = mods.indexOf("alt") >= 0;
        modMeta.checked = mods.indexOf("meta") >= 0;
        gapSpin.value = s.gap || 0;
        paddingSpin.value = s.outer_padding || 0;
        shortcutsCheck.checked = s.keyboard_shortcuts_enabled !== false;
        zoneNumbersCheck.checked = s.show_zone_numbers !== false;
        opacitySlider.value = s.overlay_opacity !== undefined ? s.overlay_opacity : 0.35;
        if (s.highlight_color)
            colorButton.color = Qt.rgba(s.highlight_color.red, s.highlight_color.green, s.highlight_color.blue, 1);
    }

    function pushPatch(patch) {
        if (loading)
            return;
        backendCall(
            "InvokeAction",
            [JSON.stringify({"action": "updateSettings", "patch": patch, "closeMenu": false})],
            "(s)",
            function(s) { page.settings = s; }
        );
    }

    function currentModifiers() {
        const m = [];
        if (modShift.checked) m.push("shift");
        if (modCtrl.checked) m.push("control");
        if (modAlt.checked) m.push("alt");
        if (modMeta.checked) m.push("meta");
        return m.length > 0 ? m : ["shift"];
    }

    Kirigami.FormLayout {
        QQC2.ButtonGroup { id: snapGroup }

        QQC2.RadioButton {
            id: snapModifier
            Kirigami.FormData.label: i18n("Snap mode:")
            QQC2.ButtonGroup.group: snapGroup
            text: i18n("Hold a modifier and drag")
            onToggled: if (checked) page.pushPatch({"snap_mode": "modifier"})
        }

        QQC2.RadioButton {
            id: snapAuto
            QQC2.ButtonGroup.group: snapGroup
            text: i18n("Auto-snap on drag")
            onToggled: if (checked) page.pushPatch({"snap_mode": "auto"})
        }

        RowLayout {
            Kirigami.FormData.label: i18n("Drag modifiers:")
            QQC2.CheckBox { id: modShift; text: i18n("Shift"); onToggled: page.pushPatch({"modifiers": page.currentModifiers()}) }
            QQC2.CheckBox { id: modCtrl; text: i18n("Ctrl"); onToggled: page.pushPatch({"modifiers": page.currentModifiers()}) }
            QQC2.CheckBox { id: modAlt; text: i18n("Alt"); onToggled: page.pushPatch({"modifiers": page.currentModifiers()}) }
            QQC2.CheckBox { id: modMeta; text: i18n("Meta"); onToggled: page.pushPatch({"modifiers": page.currentModifiers()}) }
        }

        Item { Kirigami.FormData.isSection: true }

        QQC2.SpinBox {
            id: gapSpin
            Kirigami.FormData.label: i18n("Gap between zones (px):")
            from: 0
            to: 200
            onValueModified: page.pushPatch({"gap": value})
        }

        QQC2.SpinBox {
            id: paddingSpin
            Kirigami.FormData.label: i18n("Outer padding (px):")
            from: 0
            to: 200
            onValueModified: page.pushPatch({"outer_padding": value})
        }

        Item { Kirigami.FormData.isSection: true }

        QQC2.CheckBox {
            id: shortcutsCheck
            Kirigami.FormData.label: i18n("Keyboard shortcuts:")
            text: i18n("Enable global shortcuts")
            onToggled: page.pushPatch({"keyboard_shortcuts_enabled": checked})
        }

        QQC2.CheckBox {
            id: zoneNumbersCheck
            Kirigami.FormData.label: i18n("Overlay:")
            text: i18n("Show zone numbers")
            onToggled: page.pushPatch({"show_zone_numbers": checked})
        }

        RowLayout {
            Kirigami.FormData.label: i18n("Overlay opacity:")
            QQC2.Slider {
                id: opacitySlider
                from: 0.05
                to: 0.95
                stepSize: 0.05
                Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                onMoved: page.pushPatch({"overlay_opacity": value})
            }
            QQC2.Label { text: Math.round(opacitySlider.value * 100) + "%" }
        }

        QQC2.Button {
            id: colorButton
            Kirigami.FormData.label: i18n("Highlight color:")
            property color color: Qt.rgba(0.18, 0.48, 0.96, 1)
            implicitWidth: Kirigami.Units.gridUnit * 4
            onClicked: colorDialog.open()

            background: Rectangle {
                color: colorButton.color
                radius: 3
                border.color: Kirigami.Theme.textColor
                border.width: 1
            }
        }
    }

    QtDialogs.ColorDialog {
        id: colorDialog
        selectedColor: colorButton.color
        onAccepted: {
            colorButton.color = selectedColor;
            page.pushPatch({"highlight_color": {
                "red": selectedColor.r,
                "green": selectedColor.g,
                "blue": selectedColor.b
            }});
        }
    }
}
