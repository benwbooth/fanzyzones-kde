import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    readonly property string actionPrefix: "FANZYZONES_ACTION "
    readonly property int menuMargin: 8
    property var settings: parseSettings()
    property var anchor: parseAnchor()
    property string integrationStatus: parseStatus()
    property int activeLayout: settings.active_layout || 0
    property bool closeOnDeactivate: false
    property color accent: Qt.rgba(
        settings.highlight_color ? settings.highlight_color.red : 0.18,
        settings.highlight_color ? settings.highlight_color.green : 0.48,
        settings.highlight_color ? settings.highlight_color.blue : 0.96,
        1
    )
    readonly property bool darkMode: (
        systemPalette.window.r * 0.2126
        + systemPalette.window.g * 0.7152
        + systemPalette.window.b * 0.0722
    ) < 0.5
    readonly property color menuBg: systemPalette.window
    readonly property color cardBg: systemPalette.base
    readonly property color textColor: systemPalette.windowText
    readonly property color mutedTextColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.66)
    readonly property color subtleTextColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.48)
    readonly property color borderColor: darkMode ? Qt.rgba(1, 1, 1, 0.18) : Qt.rgba(0, 0, 0, 0.16)
    readonly property color separatorColor: darkMode ? Qt.rgba(1, 1, 1, 0.13) : Qt.rgba(0, 0, 0, 0.12)
    readonly property color hoverBg: darkMode ? Qt.rgba(1, 1, 1, 0.08) : Qt.rgba(0, 0, 0, 0.06)
    readonly property color labelHoverBg: Qt.rgba(accent.r, accent.g, accent.b, darkMode ? 0.24 : 0.14)
    readonly property color dangerColor: "#dc2626"

    SystemPalette {
        id: systemPalette
        colorGroup: SystemPalette.Active
    }

    visible: true
    width: 346
    height: Math.min(menuColumn.implicitHeight + 18, Math.max(220, Screen.desktopAvailableHeight - 80))
    x: anchor.valid
        ? clamp(anchor.x - width + 24, menuMargin, Screen.desktopAvailableWidth - width - menuMargin)
        : Math.max(menuMargin, Screen.desktopAvailableWidth - width - 18)
    y: anchor.valid
        ? (anchor.y > Screen.desktopAvailableHeight / 2
            ? clamp(anchor.y - height - menuMargin, menuMargin, Screen.desktopAvailableHeight - height - menuMargin)
            : clamp(anchor.y + menuMargin, menuMargin, Screen.desktopAvailableHeight - height - menuMargin))
        : 42
    color: "transparent"
    title: "FanzyZones"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    onActiveChanged: {
        if (closeOnDeactivate && !active)
            Qt.quit();
    }

    function parseSettings() {
        for (let i = 0; i < Qt.application.arguments.length; i++) {
            const arg = Qt.application.arguments[i];
            if (arg.length > 0 && arg[0] === "{")
                return JSON.parse(arg);
        }
        return {"active_layout": 0, "layouts": []};
    }

    function parseAnchor() {
        for (let i = 0; i < Qt.application.arguments.length - 2; i++) {
            if (Qt.application.arguments[i] !== "--fanzyzones-anchor")
                continue;
            const parsedX = Number(Qt.application.arguments[i + 1]);
            const parsedY = Number(Qt.application.arguments[i + 2]);
            if (isFinite(parsedX) && isFinite(parsedY))
                return {"valid": true, "x": parsedX, "y": parsedY};
        }
        return {"valid": false, "x": 0, "y": 0};
    }

    function parseStatus() {
        for (let i = 0; i < Qt.application.arguments.length - 1; i++) {
            if (Qt.application.arguments[i] === "--fanzyzones-status")
                return Qt.application.arguments[i + 1];
        }
        return "KWin integration ready";
    }

    function clamp(value, minimum, maximum) {
        return Math.max(minimum, Math.min(maximum, value));
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

    Component.onCompleted: {
        root.raise();
        root.requestActivate();
        closeTimer.start();
    }

    Timer {
        id: closeTimer
        interval: 150
        repeat: false
        onTriggered: root.closeOnDeactivate = true
    }

    Shortcut {
        sequences: [StandardKey.Cancel]
        onActivated: Qt.quit()
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: root.menuBg
        border.color: root.borderColor
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
                        color: root.textColor
                        font.pixelSize: 15
                        font.bold: true
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        y: 6
                        text: "KDE"
                        color: root.accent
                        font.pixelSize: 11
                        font.bold: true
                    }
                }

                Separator { width: parent.width }

                MenuAction {
                    width: parent.width
                    text: "KWin Integration: " + root.integrationStatus
                    actionEnabled: false
                    labelColor: root.integrationStatus.indexOf("Error:") === 0 ? root.dangerColor : root.mutedTextColor
                }

                Separator { width: parent.width }

                SectionLabel {
                    width: parent.width
                    text: "Layouts - pane snaps, name activates"
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

                Separator { width: parent.width }

                SectionLabel {
                    width: parent.width
                    text: "Snap Mode"
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

                Separator { width: parent.width }

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

                MenuAction {
                    width: parent.width
                    text: "Reveal Config in File Manager"
                    onClicked: root.emitAction({"action": "revealConfig"})
                }

                Separator { width: parent.width }

                SectionLabel {
                    width: parent.width
                    text: "Window"
                }

                MenuAction {
                    width: parent.width
                    text: "Previous Zone"
                    onClicked: root.emitAction({"action": "previousZone"})
                }

                MenuAction {
                    width: parent.width
                    text: "Next Zone"
                    onClicked: root.emitAction({"action": "nextZone"})
                }

                Separator { width: parent.width }

                SectionLabel {
                    width: parent.width
                    text: "KWin"
                }

                MenuAction {
                    width: parent.width
                    text: "Install or Upgrade KWin Script"
                    onClicked: root.emitAction({"action": "sync"})
                }

                MenuAction {
                    width: parent.width
                    text: "Reload Settings"
                    onClicked: root.emitAction({"action": "reloadSettings"})
                }

                MenuAction {
                    width: parent.width
                    text: "Reload KWin"
                    onClicked: root.emitAction({"action": "reloadKwin"})
                }

                Separator { width: parent.width }

                MenuAction {
                    width: parent.width
                    text: "About FanzyZones"
                    onClicked: root.emitAction({"action": "about"})
                }

                MenuAction {
                    width: parent.width
                    text: "Quit FanzyZones"
                    onClicked: root.emitAction({"action": "quit"})
                }
            }
        }
    }

    component Separator: Rectangle {
        height: 1
        color: root.separatorColor
    }

    component SectionLabel: Text {
        leftPadding: 6
        rightPadding: 6
        text: ""
        color: root.subtleTextColor
        font.pixelSize: 11
        font.bold: true
    }

    component MenuAction: Rectangle {
        id: actionRoot

        signal clicked()
        property alias text: label.text
        property bool actionEnabled: true
        property color labelColor: root.textColor

        height: 30
        radius: 5
        color: mouse.containsMouse && actionEnabled ? root.hoverBg : "transparent"
        opacity: actionEnabled ? 1 : 0.78

        Text {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            x: 8
            width: parent.width - 16
            elide: Text.ElideRight
            color: actionRoot.labelColor
            font.pixelSize: 12
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            enabled: actionRoot.actionEnabled
            hoverEnabled: actionRoot.actionEnabled
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
        color: checked
            ? Qt.rgba(accent.r, accent.g, accent.b, root.darkMode ? 0.28 : 0.14)
            : (mouse.containsMouse ? root.hoverBg : "transparent")
        border.color: checked ? accent : root.borderColor
        border.width: checked ? 1 : 0

        Text {
            id: label
            anchors.centerIn: parent
            width: parent.width - 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            color: checked ? pillRoot.accent : root.textColor
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
        readonly property real padding: 6
        readonly property real labelWidth: 150
        readonly property real diagramX: padding + labelWidth + padding

        height: 92

        Rectangle {
            anchors.fill: parent
            radius: 6
            color: root.cardBg
            border.color: active ? row.accent : root.separatorColor
            border.width: active ? 1 : 0
        }

        Rectangle {
            x: row.padding
            y: row.padding
            width: row.labelWidth
            height: parent.height - row.padding * 2
            radius: 5
            color: labelMouse.containsMouse ? root.labelHoverBg : "transparent"

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
                color: root.textColor
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
                    color: editMouse.containsMouse ? row.accent : root.subtleTextColor
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
                    color: deleteMouse.containsMouse ? root.dangerColor : root.subtleTextColor
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
            color: root.darkMode ? Qt.rgba(1, 1, 1, 0.035) : Qt.rgba(0, 0, 0, 0.035)
            border.color: row.active ? row.accent : root.borderColor
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
                    color: Qt.rgba(row.accent.r, row.accent.g, row.accent.b, zoneMouse.containsMouse ? 0.52 : (root.darkMode ? 0.28 : 0.18))
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
