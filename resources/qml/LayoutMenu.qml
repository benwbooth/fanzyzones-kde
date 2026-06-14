import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    readonly property string actionPrefix: "FANZYZONES_ACTION "
    property var settings: parseSettings()
    property int activeLayout: settings.active_layout || 0
    property color accent: Qt.rgba(
        settings.highlight_color ? settings.highlight_color.red : 0.18,
        settings.highlight_color ? settings.highlight_color.green : 0.48,
        settings.highlight_color ? settings.highlight_color.blue : 0.96,
        1
    )

    visible: true
    width: 346
    height: Math.min(menuColumn.implicitHeight + 18, Screen.desktopAvailableHeight - 80)
    x: Math.max(12, Screen.desktopAvailableWidth - width - 18)
    y: 42
    color: "transparent"
    title: "FanzyZones"
    flags: Qt.Popup | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus

    function parseSettings() {
        for (let i = 0; i < Qt.application.arguments.length; i++) {
            const arg = Qt.application.arguments[i];
            if (arg.length > 0 && arg[0] === "{")
                return JSON.parse(arg);
        }
        return {"active_layout": 0, "layouts": []};
    }

    function orderedLayoutIndexes() {
        const count = settings.layouts ? settings.layouts.length : 0;
        const indexes = [];
        if (activeLayout >= 0 && activeLayout < count)
            indexes.push(activeLayout);
        for (let i = 0; i < count; i++) {
            if (i !== activeLayout)
                indexes.push(i);
        }
        return indexes;
    }

    function emitAction(action) {
        console.log(actionPrefix + JSON.stringify(action));
        Qt.quit();
    }

    function zoneRect(zone, area) {
        return Qt.rect(
            area.x + zone.x * area.width,
            area.y + zone.y * area.height,
            Math.max(3, zone.width * area.width),
            Math.max(3, zone.height * area.height)
        );
    }

    Shortcut {
        sequence: StandardKey.Cancel
        onActivated: Qt.quit()
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: "#f8fafc"
        border.color: "#cbd5e1"
        border.width: 1

        Flickable {
            anchors.fill: parent
            anchors.margins: 9
            contentWidth: width
            contentHeight: menuColumn.implicitHeight
            clip: true

            Column {
                id: menuColumn
                width: parent.width
                spacing: 5

                Item {
                    width: parent.width
                    height: 32

                    Text {
                        x: 6
                        y: 3
                        text: "FanzyZones"
                        color: "#0f172a"
                        font.pixelSize: 15
                        font.bold: true
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        y: 6
                        text: "KDE"
                        color: accent
                        font.pixelSize: 11
                        font.bold: true
                    }
                }

                Rectangle {
                    width: parent.width
                    height: 1
                    color: "#e2e8f0"
                }

                Text {
                    width: parent.width
                    leftPadding: 6
                    rightPadding: 6
                    text: "Layouts - pane snaps, name activates"
                    color: "#475569"
                    font.pixelSize: 11
                }

                Repeater {
                    model: orderedLayoutIndexes()

                    delegate: LayoutRow {
                        required property int modelData

                        width: menuColumn.width
                        layoutIndex: modelData
                        layout: settings.layouts[modelData]
                        active: modelData === activeLayout
                        accent: root.accent

                        onSetActive: function(index) {
                            root.activeLayout = index;
                            root.emitAction({"action": "setLayout", "layout": index});
                        }

                        onSnapZone: function(index, zone) {
                            root.emitAction({"action": "snap", "layout": index, "zone": zone});
                        }

                        onEditLayout: function(index) {
                            root.emitAction({"action": "editLayout", "layout": index});
                        }

                        onDeleteLayout: function(index) {
                            root.emitAction({"action": "deleteLayout", "layout": index});
                        }
                    }
                }

                Rectangle {
                    width: parent.width
                    height: 1
                    color: "#e2e8f0"
                }

                Row {
                    width: parent.width
                    height: 30
                    spacing: 6

                    MenuPill {
                        width: (parent.width - 6) / 2
                        text: settings.snap_mode === "modifier" ? "Hold modifier and drag" : "Use modifier drag"
                        checked: settings.snap_mode === "modifier"
                        accent: root.accent
                        onClicked: root.emitAction({"action": "setSnapMode", "mode": "modifier"})
                    }

                    MenuPill {
                        width: (parent.width - 6) / 2
                        text: settings.snap_mode === "auto" ? "Auto-snap on drag" : "Use auto-snap"
                        checked: settings.snap_mode === "auto"
                        accent: root.accent
                        onClicked: root.emitAction({"action": "setSnapMode", "mode": "auto"})
                    }
                }

                MenuAction {
                    width: parent.width
                    text: "Create Custom Layout..."
                    onClicked: root.emitAction({"action": "createLayout"})
                }

                MenuAction {
                    width: parent.width
                    text: "Settings..."
                    onClicked: root.emitAction({"action": "openSettings"})
                }
            }
        }
    }

    component MenuAction: Rectangle {
        id: actionRoot

        signal clicked()
        property alias text: label.text

        height: 30
        radius: 5
        color: mouse.containsMouse ? "#e2e8f0" : "transparent"

        Text {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            x: 8
            width: parent.width - 16
            elide: Text.ElideRight
            color: "#0f172a"
            font.pixelSize: 12
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: actionRoot.clicked()
        }
    }

    component MenuPill: Rectangle {
        id: pillRoot

        signal clicked()
        property alias text: label.text
        property bool checked: false
        property color accent: "#3b82f6"

        height: 28
        radius: 5
        color: checked ? Qt.rgba(accent.r, accent.g, accent.b, 0.14) : (mouse.containsMouse ? "#e2e8f0" : "transparent")
        border.color: checked ? accent : "#cbd5e1"
        border.width: checked ? 1 : 0

        Text {
            id: label
            anchors.centerIn: parent
            width: parent.width - 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            color: checked ? pillRoot.accent : "#334155"
            font.pixelSize: 11
            font.bold: checked
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: pillRoot.clicked()
        }
    }

    component LayoutRow: Item {
        id: row

        signal setActive(int index)
        signal snapZone(int index, int zone)
        signal editLayout(int index)
        signal deleteLayout(int index)

        property int layoutIndex: 0
        property var layout
        property bool active: false
        property color accent: "#3b82f6"
        property int hoveredZone: -1
        readonly property real padding: 6
        readonly property real labelWidth: 150
        readonly property real diagramX: padding + labelWidth + padding

        height: 92

        Rectangle {
            anchors.fill: parent
            radius: 6
            color: "#ffffff"
            border.color: active ? row.accent : "#e2e8f0"
            border.width: active ? 1 : 0
        }

        Rectangle {
            x: row.padding
            y: row.padding
            width: row.labelWidth
            height: parent.height - row.padding * 2
            radius: 5
            color: labelMouse.containsMouse ? "#e0f2fe" : "transparent"

            MouseArea {
                id: labelMouse
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton
                cursorShape: Qt.PointingHandCursor
                onClicked: row.setActive(row.layoutIndex)
            }

            Text {
                x: 3
                y: 4
                width: parent.width - 6
                text: row.layout.name
                elide: Text.ElideRight
                color: "#0f172a"
                font.pixelSize: 12
                font.bold: row.active
            }

            Text {
                x: 3
                y: 36
                visible: row.active
                text: "Active"
                color: row.accent
                font.pixelSize: 10
            }

            Row {
                x: 3
                y: parent.height - 18
                spacing: 12
                visible: !row.layout.is_built_in

                Text {
                    text: "Edit"
                    color: editMouse.containsMouse ? row.accent : "#64748b"
                    font.pixelSize: 10

                    MouseArea {
                        id: editMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: row.editLayout(row.layoutIndex)
                    }
                }

                Text {
                    text: "Delete"
                    color: deleteMouse.containsMouse ? "#dc2626" : "#64748b"
                    font.pixelSize: 10

                    MouseArea {
                        id: deleteMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: row.deleteLayout(row.layoutIndex)
                    }
                }
            }
        }

        Rectangle {
            id: diagram
            x: row.diagramX
            y: row.padding
            width: parent.width - row.diagramX - row.padding
            height: parent.height - row.padding * 2
            radius: 6
            color: "#f8fafc"
            border.color: row.active ? row.accent : "#cbd5e1"
            border.width: row.active ? 2 : 1

            Repeater {
                model: row.layout.zones

                delegate: Rectangle {
                    required property var modelData
                    required property int index

                    readonly property rect zr: root.zoneRect(modelData, Qt.rect(0, 0, diagram.width, diagram.height))
                    x: zr.x + 2
                    y: zr.y + 2
                    width: Math.max(4, zr.width - 4)
                    height: Math.max(4, zr.height - 4)
                    radius: 3
                    color: Qt.rgba(row.accent.r, row.accent.g, row.accent.b, zoneMouse.containsMouse ? 0.52 : 0.18)
                    border.color: Qt.rgba(row.accent.r, row.accent.g, row.accent.b, zoneMouse.containsMouse ? 0.95 : 0.42)
                    border.width: zoneMouse.containsMouse ? 2 : 1

                    MouseArea {
                        id: zoneMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: row.snapZone(row.layoutIndex, index)
                    }
                }
            }
        }
    }
}
