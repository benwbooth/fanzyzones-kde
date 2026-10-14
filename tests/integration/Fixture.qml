import QtQuick
import QtQuick.Window

Window {
    id: window
    readonly property string label: Qt.application.arguments[Qt.application.arguments.length - 1]
    visible: true
    width: 520
    height: 360
    minimumWidth: 160
    minimumHeight: 100
    title: "Workspace fixture " + label
    flags: label === "Frameless" ? Qt.Window | Qt.FramelessWindowHint : Qt.Window
    color: label === "Alpha" ? "#315b78" : label === "Beta" ? "#78513a" : "#4e6850"
    Shortcut { sequence: "Ctrl+M"; onActivated: modal.visible = true }
    Window {
        id: modal
        transientParent: window
        modality: Qt.WindowModal
        title: "Fixture dialog " + window.label
        width: 280; height: 160
        color: "#ddd6c9"
        Text { anchors.centerIn: parent; text: "Attached to " + window.label }
    }
    Column {
        anchors { fill: parent; margins: 22 }
        spacing: 18
        Text { text: window.title; color: "white"; font.pixelSize: 24 }
        Text { text: "Native client · " + window.width + " × " + window.height; color: "white" }
        Rectangle {
            width: parent.width; height: 44; radius: 4; color: "#eeeeee"
            TextInput {
                anchors { fill: parent; margins: 10 }
                focus: true
                text: "Type here after navigating"
                onTextEdited: console.log("FIXTURE_INPUT", window.label, text)
            }
        }
    }
}
