import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    readonly property string actionPrefix: "FANZYZONES_ACTION "
    readonly property int menuMargin: 8
    property string commandUrl: parseArgument("--fanzyzones-command-url", "")
    readonly property bool hostMode: commandUrl.length > 0
    property int commandSequence: -1
    property bool commandReadInFlight: false
    property bool menuVisible: !hostMode
    property var settings: parseSettings()
    property var anchor: parseAnchor()
    property var placementAnchor: normalizeAnchor(anchor)
    property string integrationStatus: parseStatus()
    property string actionUrl: parseActionUrl()
    property bool debugPlacement: parseFlag("--fanzyzones-debug-placement")
    property int activeLayout: activeLayoutFromSettings(settings)
    property bool closeOnDeactivate: false
    property bool actionEmitted: false
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
    readonly property color highlightBg: systemPalette.highlight
    readonly property color highlightTextColor: systemPalette.highlightedText
    readonly property color mutedTextColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.66)
    readonly property color subtleTextColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.48)
    readonly property color borderColor: darkMode ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.18)
    readonly property color separatorColor: darkMode ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(0, 0, 0, 0.10)
    readonly property color hoverBg: highlightBg
    readonly property color hoverTextColor: highlightTextColor
    readonly property color checkedBg: Qt.rgba(highlightBg.r, highlightBg.g, highlightBg.b, darkMode ? 0.22 : 0.12)
    readonly property color dangerColor: "#dc2626"
    readonly property int menuWindowFlags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    readonly property int idleWindowFlags: menuWindowFlags | Qt.WindowTransparentForInput
    readonly property real menuWidth: 346
    readonly property real menuHeight: Math.min(
        Math.max(menuColumn.implicitHeight + 18, 220),
        Math.max(220, Screen.desktopAvailableHeight - 80)
    )
    readonly property real screenLeft: Screen.virtualX
    readonly property real screenTop: Screen.virtualY
    readonly property real screenWidth: Screen.width
    readonly property real screenHeight: Screen.height
    readonly property real availableLeft: Math.min(0, Screen.virtualX)
    readonly property real availableTop: Math.min(0, Screen.virtualY)
    readonly property real availableRight: availableLeft + Screen.desktopAvailableWidth
    readonly property real availableBottom: availableTop + Screen.desktopAvailableHeight

    SystemPalette {
        id: systemPalette
        colorGroup: SystemPalette.Active
    }

    visible: true
    opacity: hostMode && !menuVisible ? 0 : 1
    width: hostMode && !menuVisible ? 1 : screenWidth
    height: hostMode && !menuVisible ? 1 : screenHeight
    x: hostMode && !menuVisible ? availableLeft : screenLeft
    y: hostMode && !menuVisible ? availableTop : screenTop
    color: "transparent"
    title: "FanzyZones"
    flags: hostMode && !menuVisible ? idleWindowFlags : menuWindowFlags

    onActiveChanged: {
        if (!menuVisible || !closeOnDeactivate || actionEmitted)
            return;
        if (active)
            deactivateCloseTimer.stop();
        else
            deactivateCloseTimer.restart();
    }

    function invalidAnchor() {
        return {"valid": false, "x": 0, "y": 0};
    }

    function parseArgument(flag, fallback) {
        for (let i = 0; i < Qt.application.arguments.length - 1; i++) {
            if (Qt.application.arguments[i] === flag)
                return Qt.application.arguments[i + 1];
        }
        return fallback;
    }

    function activeLayoutFromSettings(value) {
        if (value && value.active_layout !== undefined)
            return value.active_layout;
        if (value && value.activeLayout !== undefined)
            return value.activeLayout;
        return 0;
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
        return invalidAnchor();
    }

    function parseStatus() {
        return parseArgument("--fanzyzones-status", "KWin integration ready");
    }

    function parseActionUrl() {
        return parseArgument("--fanzyzones-action-url", "");
    }

    function parseFlag(flag) {
        for (let i = 0; i < Qt.application.arguments.length; i++) {
            if (Qt.application.arguments[i] === flag)
                return true;
        }
        return false;
    }

    function clamp(value, minimum, maximum) {
        return Math.max(minimum, Math.min(maximum, value));
    }

    function screenScale() {
        const dpr = Number(Screen.devicePixelRatio);
        if (isFinite(dpr) && dpr > 1)
            return dpr;
        return 1;
    }

    function normalizeAnchor(rawAnchor) {
        const scale = screenScale();
        if (!rawAnchor.valid || scale <= 1)
            return {
                "valid": rawAnchor.valid,
                "x": rawAnchor.x,
                "y": rawAnchor.y,
                "scale": scale,
                "normalized": false
            };

        const outsideLogicalScreen = rawAnchor.x > availableRight + menuMargin
            || rawAnchor.y > availableBottom + menuMargin
            || rawAnchor.x < availableLeft - menuMargin
            || rawAnchor.y < availableTop - menuMargin;
        if (!outsideLogicalScreen)
            return {
                "valid": true,
                "x": rawAnchor.x,
                "y": rawAnchor.y,
                "scale": scale,
                "normalized": false
            };

        return {
            "valid": true,
            "x": rawAnchor.x / scale,
            "y": rawAnchor.y / scale,
            "scale": scale,
            "normalized": true
        };
    }

    function contextMenuX() {
        const minX = availableLeft + menuMargin;
        const maxX = availableRight - menuWidth - menuMargin;
        if (!placementAnchor.valid)
            return clamp(availableRight - menuWidth - menuMargin, minX, maxX);

        let proposed = placementAnchor.x;
        if (proposed + menuWidth > availableRight - menuMargin)
            proposed = placementAnchor.x - menuWidth;
        return clamp(proposed, minX, maxX);
    }

    function contextMenuY() {
        const minY = availableTop + menuMargin;
        const maxY = availableBottom - menuHeight - menuMargin;
        if (!placementAnchor.valid)
            return clamp(availableTop + menuMargin, minY, maxY);

        let proposed = placementAnchor.y;
        if (proposed + menuHeight > availableBottom - menuMargin)
            proposed = placementAnchor.y - menuHeight;
        return clamp(proposed, minY, maxY);
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

    function emitAction(action, closeAfter) {
        const shouldClose = closeAfter !== false || !hostMode;
        if (shouldClose) {
            actionEmitted = true;
            deactivateCloseTimer.stop();
        } else {
            action.closeMenu = false;
        }
        const payload = JSON.stringify(action);
        const posted = postPayload(payload);
        if (!posted)
            print(actionPrefix + payload);
        if (!shouldClose)
            return posted;
        if (hostMode)
            closeMenu(false);
        else
            Qt.quit();
        return posted;
    }

    function postPayload(payload) {
        if (actionUrl.length === 0)
            return false;

        try {
            const request = new XMLHttpRequest();
            request.open("POST", actionUrl, false);
            request.setRequestHeader("Content-Type", "text/plain");
            request.send(payload);
            return true;
        } catch (error) {
            return false;
        }
    }

    function zoneRect(zone, area) {
        return Qt.rect(
            area.x + zone.x * area.width,
            area.y + zone.y * area.height,
            Math.max(3, zone.width * area.width),
            Math.max(3, zone.height * area.height)
        );
    }

    function placementDetails() {
        return {
            "anchor": anchor,
            "placementAnchor": placementAnchor,
            "x": contextMenuX(),
            "y": contextMenuY(),
            "width": menuWidth,
            "height": menuHeight,
            "windowX": root.x,
            "windowY": root.y,
            "windowWidth": root.width,
            "windowHeight": root.height,
            "availableLeft": availableLeft,
            "availableTop": availableTop,
            "availableRight": availableRight,
            "availableBottom": availableBottom,
            "screen": {
                "width": Screen.width,
                "height": Screen.height,
                "desktopAvailableWidth": Screen.desktopAvailableWidth,
                "desktopAvailableHeight": Screen.desktopAvailableHeight,
                "devicePixelRatio": Screen.devicePixelRatio,
                "virtualX": Screen.virtualX,
                "virtualY": Screen.virtualY
            }
        };
    }

    function logPlacement() {
        if (!debugPlacement)
            return;

        const payload = JSON.stringify({
            "action": "debugPlacement",
            "placement": placementDetails()
        });
        if (!postPayload(payload))
            console.log("FANZYZONES_PLACEMENT " + payload);
    }

    function readCommand() {
        if (!hostMode)
            return;
        if (commandReadInFlight)
            return;

        try {
            commandReadInFlight = true;
            const request = new XMLHttpRequest();
            request.onreadystatechange = function() {
                if (request.readyState !== XMLHttpRequest.DONE)
                    return;

                commandReadInFlight = false;
                try {
                    if (request.status !== 0 && request.status !== 200)
                        return;

                    const command = JSON.parse(request.responseText);
                    if (command.sequence === undefined || command.sequence === commandSequence)
                        return;

                    commandSequence = command.sequence;
                    applyCommand(command);
                } catch (error) {
                }
            };
            request.open("GET", commandUrl + "?sequence=" + commandSequence + "&t=" + Date.now(), true);
            request.send();
        } catch (error) {
            commandReadInFlight = false;
        }
    }

    function applyCommand(command) {
        if (command.settings !== undefined)
            settings = command.settings;
        activeLayout = activeLayoutFromSettings(settings);
        if (command.status !== undefined)
            integrationStatus = command.status;
        debugPlacement = !!command.debugPlacement;
        if (!command.visible) {
            closeMenu(false);
            return;
        }

        if (command.anchor !== undefined) {
            anchor = command.anchor;
            placementAnchor = normalizeAnchor(anchor);
        }
        actionEmitted = false;
        closeOnDeactivate = false;
        menuVisible = true;
        raise();
        requestActivate();
        placementLogTimer.restart();
        closeTimer.restart();
    }

    function closeMenu(emitClosed) {
        if (!hostMode) {
            Qt.quit();
            return;
        }

        const wasVisible = menuVisible;
        deactivateCloseTimer.stop();
        closeTimer.stop();
        closeOnDeactivate = false;
        menuVisible = false;
        if (emitClosed && wasVisible && !actionEmitted) {
            postPayload(JSON.stringify({
                "event": "closed",
                "sequence": commandSequence
            }));
        }
        actionEmitted = false;
    }

    Component.onCompleted: {
        if (hostMode)
            return;

        root.raise();
        root.requestActivate();
        placementLogTimer.start();
        closeTimer.start();
    }

    Timer {
        id: commandPollTimer
        interval: 16
        repeat: true
        running: root.hostMode
        triggeredOnStart: true
        onTriggered: readCommand()
    }

    Timer {
        id: placementLogTimer
        interval: 75
        repeat: false
        onTriggered: logPlacement()
    }

    Timer {
        id: closeTimer
        interval: 150
        repeat: false
        onTriggered: root.closeOnDeactivate = true
    }

    Timer {
        id: deactivateCloseTimer
        interval: 180
        repeat: false
        onTriggered: {
            if (!root.active && !root.actionEmitted)
                root.closeMenu(true);
        }
    }

    Shortcut {
        sequences: [StandardKey.Cancel]
        onActivated: root.closeMenu(true)
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        enabled: root.menuVisible
        onPressed: root.closeMenu(true)
        onWheel: root.closeMenu(true)
    }

    Rectangle {
        x: root.contextMenuX() - root.x
        y: root.contextMenuY() - root.y
        width: root.menuWidth
        height: root.menuHeight
        visible: root.menuVisible
        radius: 4
        color: root.menuBg
        border.color: root.borderColor
        border.width: 1

        Flickable {
            anchors.fill: parent
            anchors.margins: 6
            contentWidth: width
            contentHeight: menuColumn.implicitHeight
            clip: true

            Column {
                id: menuColumn
                width: parent.width
                spacing: 3

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
                            if (root.emitAction({"action": "setLayout", "layout": index}, false))
                                root.activeLayout = index;
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
        radius: 3
        color: mouse.containsMouse && actionEnabled ? root.hoverBg : "transparent"
        opacity: actionEnabled ? 1 : 0.78

        Text {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            x: 8
            width: parent.width - 16
            elide: Text.ElideRight
            color: mouse.containsMouse && actionRoot.actionEnabled
                ? root.hoverTextColor
                : actionRoot.labelColor
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
        radius: 3
        color: mouse.containsMouse ? root.hoverBg : (checked ? root.checkedBg : "transparent")
        border.color: checked && !mouse.containsMouse ? accent : "transparent"
        border.width: checked && !mouse.containsMouse ? 1 : 0

        Text {
            id: label
            anchors.centerIn: parent
            width: parent.width - 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            color: mouse.containsMouse ? root.hoverTextColor : (checked ? pillRoot.accent : root.textColor)
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

        height: 90

        Rectangle {
            anchors.fill: parent
            radius: 3
            color: root.cardBg
            border.color: active ? row.accent : root.separatorColor
            border.width: active ? 1 : 0
        }

        MouseArea {
            id: rowMouse
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            cursorShape: Qt.PointingHandCursor
            onClicked: row.setActive(row.layoutIndex)
        }

        Rectangle {
            x: row.padding
            y: row.padding
            width: row.labelWidth
            height: parent.height - row.padding * 2
            radius: 3
            color: labelMouse.containsMouse ? root.hoverBg : "transparent"

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
                color: labelMouse.containsMouse ? root.hoverTextColor : root.textColor
                font.pixelSize: 12
                font.bold: row.active
            }

            Text {
                x: 3
                y: 36
                visible: row.active
                text: "Active"
                color: labelMouse.containsMouse ? root.hoverTextColor : row.accent
                font.pixelSize: 10
            }

            Row {
                x: 3
                y: parent.height - 18
                spacing: 12
                visible: !row.layout.is_built_in

                Text {
                    text: "Edit"
                    color: labelMouse.containsMouse ? root.hoverTextColor : (editMouse.containsMouse ? row.accent : root.subtleTextColor)
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
                    color: labelMouse.containsMouse ? root.hoverTextColor : (deleteMouse.containsMouse ? root.dangerColor : root.subtleTextColor)
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
            radius: 3
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
                    radius: 2
                    color: zoneMouse.containsMouse
                        ? root.hoverBg
                        : Qt.rgba(row.accent.r, row.accent.g, row.accent.b, root.darkMode ? 0.28 : 0.18)
                    border.color: zoneMouse.containsMouse
                        ? root.hoverBg
                        : Qt.rgba(row.accent.r, row.accent.g, row.accent.b, 0.42)
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
