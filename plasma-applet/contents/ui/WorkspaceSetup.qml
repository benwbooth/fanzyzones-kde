import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: wizard
    title: "FanzyZones — Set up nested workspaces"
    width: 620
    height: 460
    minimumWidth: 560
    minimumHeight: 440
    visible: false
    color: "#202428"
    property int step: 0
    property bool arrange: true
    property bool iconOnly: true
    property bool busy: false
    property string errorText: ""
    signal completed(bool arrange, bool iconOnly)
    function open() { step = 0; errorText = ""; busy = false; show(); raise(); requestActivate(); }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 18
        Label {
            text: ["Make room for your work", "Check your desktop", "Choose your starting view"][wizard.step]
            font.pixelSize: 24
            font.bold: true
            Layout.fillWidth: true
            wrapMode: Text.Wrap
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: [
                "Drag a window to an edge to split a space. Pause over its center to create tabs. Drag the gaps to resize. Your arrangement is saved automatically.",
                "Use Plasma 6.6 or later with KWin. The matching native input module adds Alt + two-finger zoom and Meta + Shift drag framing. Keyboard navigation works without that module.\n\nDisable other automatic tiling scripts if they move the same windows. Review any conflicting shortcuts in FanzyZones Settings. No accessibility permission is needed on KDE.",
                "Arrange existing windows into nested spaces now, or keep them floating on the Desktop and dock them as you work. You can change this at any time."
            ][wizard.step]
        }
        Rectangle {
            visible: wizard.step === 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: palette.alternateBase
            radius: 8
            RowLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 8
                Rectangle { Layout.fillWidth: true; Layout.fillHeight: true; color: "#315f82"; radius: 5
                    Label { anchors.centerIn: parent; text: "Window"; color: "white" } }
                ColumnLayout { Layout.fillWidth: true; Layout.fillHeight: true
                    Rectangle { Layout.fillWidth: true; Layout.fillHeight: true; color: "#58628c"; radius: 5
                        Label { anchors.centerIn: parent; text: "Tabs"; color: "white" } }
                    Rectangle { Layout.fillWidth: true; Layout.fillHeight: true; color: "#376859"; radius: 5
                        Label { anchors.centerIn: parent; text: "Desktop"; color: "white" } }
                }
            }
        }
        Label {
            visible: wizard.step === 1
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: "Ctrl + Alt + Space shows everything. Double-click a title strip to focus. Ctrl + Alt + F toggles floating. To restore windows and leave nested mode, use the menu or Meta + Ctrl + Shift + F12."
        }
        ColumnLayout {
            visible: wizard.step === 2
            RadioButton { text: "Arrange my open windows"; checked: wizard.arrange; onClicked: wizard.arrange = true }
            RadioButton { text: "Keep my windows floating"; checked: !wizard.arrange; onClicked: wizard.arrange = false }
            CheckBox { text: "Show only an icon in the panel"; checked: wizard.iconOnly; onToggled: wizard.iconOnly = checked }
        }
        Item { Layout.fillHeight: true; visible: wizard.step !== 0 }
        Label { visible: wizard.errorText !== ""; text: wizard.errorText; color: "#d85b5b"; wrapMode: Text.Wrap; Layout.fillWidth: true }
        RowLayout {
            Layout.fillWidth: true
            Button { text: "Later"; enabled: !wizard.busy; onClicked: wizard.close() }
            Item { Layout.fillWidth: true }
            Button { text: "Back"; visible: wizard.step > 0; enabled: !wizard.busy; onClicked: wizard.step-- }
            Button {
                text: wizard.busy ? "Starting…" : wizard.step === 2 ? "Start workspace" : "Next"
                enabled: !wizard.busy
                onClicked: { if (wizard.step < 2) wizard.step++; else { wizard.busy = true; wizard.completed(wizard.arrange, wizard.iconOnly); } }
            }
        }
    }
}
