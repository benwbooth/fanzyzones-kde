import QtQuick
import QtQuick.Window

Window {
    required property var controller
    readonly property var target: controller.dropTarget
    title: "FanzyZones Workspace Drop Preview"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus | Qt.WindowTransparentForInput
    color: "transparent"
    visible: controller.enabled && !!target
    x: target ? Math.round(target.rect.x) : 0
    y: target ? Math.round(target.rect.y) : 0
    width: target ? Math.max(1, Math.round(target.rect.width)) : 1
    height: target ? Math.max(1, Math.round(target.rect.height)) : 1
    Rectangle {
        anchors { fill: parent; margins: 5 }
        radius: 8
        color: target && target.armed ? "#5584baff" : "#224c7ab0"
        border { width: 2; color: "#9dcaff" }
        Text { anchors.centerIn: parent; text: target ? target.label : ""; color: "white"; font.pixelSize: 16 }
    }
}
