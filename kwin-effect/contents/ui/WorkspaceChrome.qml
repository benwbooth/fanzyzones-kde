import QtQuick
import QtQuick.Window
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Window {
    id: chrome
    required property var controller
    required property var entry
    property string contextWindowId: ""
    title: "FanzyZones Workspace"
    // KWin treats Qt.Tool's Popup flag bits as a popup grab. Ordinary internal
    // windows keep chrome interactive without closing it on an outside click.
    flags: Qt.Window | Qt.BypassWindowManagerHint | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus
    color: "transparent"
    visible: controller.chromeVisible && !!entry.rect && !entry.blocked
    x: entry.rect ? Math.round(entry.rect.x) : 0
    y: entry.rect ? Math.round(entry.rect.y) : 0
    width: entry.rect ? Math.max(1, Math.round(entry.rect.width)) : 1
    height: entry.rect ? Math.max(1, Math.round(entry.rect.height)) : 1

    Rectangle {
        anchors.fill: parent
        radius: entry.type === "placeholder" ? 6 : 3
        color: entry.type === "seam" ? (seamMouse.containsMouse ? "#8caee9" : "#292f3b")
            : entry.type === "placeholder" ? controller.appColor(entry.id) : "#293445"
        border.width: entry.type === "placeholder" ? 1 : 0
        border.color: "#6884aa"

        MouseArea {
            id: seamMouse
            anchors.fill: parent
            enabled: entry.type === "seam"
            hoverEnabled: true
            cursorShape: entry.axis === "x" ? Qt.SplitHCursor : Qt.SplitVCursor
            onPressed: controller.beginSeam(entry)
            onPositionChanged: if (pressed) controller.updateSeam()
            onReleased: controller.endSeam()
            onCanceled: controller.endSeam()
        }

        Column {
            anchors.centerIn: parent
            width: parent.width - 12
            spacing: 6
            visible: entry.type === "placeholder"
            Kirigami.Icon {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.max(12, Math.min(48, chrome.height - 40, chrome.width - 20))
                height: width
                source: controller.clients[entry.id] ? controller.clients[entry.id].icon : "user-desktop"
            }
            Text {
                width: parent.width
                text: entry.windows && entry.windows.length > 1 ? entry.windows.length + " windows" : entry.title || "Desktop"
                color: "#f4f6fc"
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                font.pixelSize: 11
                visible: chrome.height > 52
            }
        }
        Repeater {
            model: entry.type === "placeholder" && entry.splits ? entry.splits : []
            Rectangle {
                required property var modelData
                x: modelData.x * chrome.width
                y: modelData.y * chrome.height
                width: modelData.width * chrome.width
                height: modelData.height * chrome.height
                color: "transparent"
                border { width: 1; color: "#406884aa" }
            }
        }
        MouseArea {
            anchors.fill: parent
            enabled: entry.type === "placeholder"
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onDoubleClicked: entry.id && !entry.splits ? controller.focusWindow(entry.id) : controller.focusNode(entry.scope, entry.node.split(":")[0])
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton) { chrome.contextWindowId = entry.id || ""; contextMenu.popup(); }
            }
        }

        Row {
            id: tabs
            anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
            width: parent.width - (entry.id ? 68 : 0)
            visible: entry.type === "header"
            Repeater {
                model: entry.windows && entry.windows.length && entry.kind !== "desktop" ? entry.windows : [""]
                Rectangle {
                    id: tab
                    required property string modelData
                    width: Math.max(24, tabs.width / (entry.kind === "desktop" ? 1 : Math.max(1, (entry.windows || []).length)))
                    height: tabs.height
                    color: modelData === entry.id && (entry.windows || []).length > 1 ? "#455e82" : tabMouse.containsMouse ? "#35465e" : "transparent"
                    Kirigami.Icon {
                        id: icon
                        x: 6
                        anchors.verticalCenter: parent.verticalCenter
                        width: 16; height: 16
                        source: controller.clients[tab.modelData] ? controller.clients[tab.modelData].icon : "user-desktop"
                    }
                    Text {
                        anchors { left: icon.right; right: parent.right; margins: 6; verticalCenter: parent.verticalCenter }
                        text: controller.model.windows[tab.modelData] ? controller.model.windows[tab.modelData].title : "Desktop"
                        color: "#f1f5fb"
                        font.pixelSize: 11
                        elide: Text.ElideRight
                    }
                    MouseArea {
                        id: tabMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                        property point start
                        property bool moving: false
                        onPressed: function(mouse) {
                            start = Qt.point(mouse.x, mouse.y); moving = false;
                            if (mouse.button === Qt.RightButton) { chrome.contextWindowId = tab.modelData; contextMenu.popup(); }
                            else if (mouse.button === Qt.MiddleButton && controller.clients[tab.modelData]) controller.clients[tab.modelData].closeWindow();
                        }
                        onPositionChanged: function(mouse) {
                            if (!pressed || !(pressedButtons & Qt.LeftButton) || !tab.modelData) return;
                            if (!moving && Math.abs(mouse.x - start.x) + Math.abs(mouse.y - start.y) > 8) {
                                moving = true;
                                controller.beginDrag(tab.modelData, true, !!(mouse.modifiers & Qt.ShiftModifier));
                            }
                            if (moving) controller.updateDrag();
                        }
                        onReleased: function(mouse) {
                            if (moving) controller.finishDrag();
                            else if (mouse.button === Qt.LeftButton && tab.modelData) controller.selectTab(entry.scope, entry.node, tab.modelData);
                            moving = false;
                        }
                        onCanceled: { if (moving) controller.finishDrag(true); moving = false; }
                        onDoubleClicked: tab.modelData ? controller.focusWindow(tab.modelData) : controller.focusNode(entry.scope, entry.node)
                    }
                }
            }
        }
        Row {
            anchors { right: parent.right; top: parent.top; bottom: parent.bottom }
            visible: entry.type === "header" && !!entry.id
            Repeater {
                model: ["−", "↗", "×"]
                Rectangle {
                    required property string modelData
                    width: 22; height: chrome.height
                    color: buttonMouse.containsMouse ? (modelData === "×" ? "#b94b55" : "#50678a") : "transparent"
                    Text { anchors.centerIn: parent; text: modelData; color: "#f4f6ff"; font.pixelSize: 15 }
                    MouseArea {
                        id: buttonMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        Controls.ToolTip.visible: containsMouse
                        Controls.ToolTip.text: modelData === "×" ? "Close window" : modelData === "−" ? "Minimize window" : "Float or dock window"
                        onClicked: {
                            const w = controller.clients[entry.id];
                            if (!w) return;
                            if (modelData === "×") w.closeWindow();
                            else if (modelData === "−") w.minimized = true;
                            else controller.execute({ action: "toggle-float", window: entry.id });
                        }
                    }
                }
            }
        }
        Controls.Menu {
            id: contextMenu
            Controls.MenuItem {
                text: "Focus this space"
                onTriggered: {
                    if (chrome.contextWindowId) controller.selectTab(entry.scope, entry.node, chrome.contextWindowId);
                    if (chrome.contextWindowId) controller.focusWindow(chrome.contextWindowId);
                    else controller.focusNode(entry.scope, entry.node);
                }
            }
            Controls.MenuItem { text: "Float window"; enabled: !!chrome.contextWindowId; onTriggered: controller.execute({ action: "float", window: chrome.contextWindowId }) }
            Controls.MenuItem { text: "Leave this application alone"; enabled: !!chrome.contextWindowId; onTriggered: controller.execute({ action: "exclude-app", window: chrome.contextWindowId }) }
            Controls.MenuSeparator {}
            Controls.MenuItem { text: "Close window"; enabled: !!chrome.contextWindowId; onTriggered: if (controller.clients[chrome.contextWindowId]) controller.clients[chrome.contextWindowId].closeWindow() }
        }
    }
}
