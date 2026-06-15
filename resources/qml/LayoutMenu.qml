import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    readonly property string actionPrefix: "FANZYZONES_ACTION "
    readonly property int menuMargin: 8
    property var backend: null
    property bool embeddedBackendRequested: false
    property string trayIconSource: parseArgument("--fanzyzones-tray-icon-source", "")
    readonly property bool backendMode: backend !== null
    readonly property bool trayMode: backendMode
    property int commandSequence: -1
    property bool menuVisible: !(trayMode || embeddedBackendRequested)
    readonly property bool idleMode: trayMode && !menuVisible
    property var settings: parseSettings()
    property var anchor: parseAnchor()
    property var placementAnchor: normalizeAnchor(anchor)
    property var lastTrayAnchor: invalidAnchor()
    property string integrationStatus: parseStatus()
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
    readonly property int menuWindowFlags: Qt.Popup | Qt.FramelessWindowHint
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

    visible: !idleMode
    opacity: 1
    width: menuWidth
    height: menuHeight
    x: contextMenuX()
    y: contextMenuY()
    color: "transparent"
    title: "FanzyZones"
    flags: menuWindowFlags

    onActiveChanged: {
        if (!menuVisible || !closeOnDeactivate || actionEmitted)
            return;
        if (active)
            deactivateCloseTimer.stop();
        else
            deactivateCloseTimer.restart();
    }

    onVisibleChanged: {
        if (trayMode && !visible && menuVisible && !actionEmitted)
            closeMenu(false);
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

    function anchorWithPosition(rawAnchor, x, y, scale, normalized) {
        return {
            "valid": rawAnchor.valid,
            "x": x,
            "y": y,
            "width": rawAnchor.width || 0,
            "height": rawAnchor.height || 0,
            "source": rawAnchor.source || "",
            "edge": rawAnchor.edge || "",
            "scale": scale,
            "normalized": normalized
        };
    }

    function normalizeAnchor(rawAnchor) {
        const scale = screenScale();
        if (!rawAnchor.valid || scale <= 1)
            return anchorWithPosition(rawAnchor, rawAnchor.x, rawAnchor.y, scale, false);

        const outsideLogicalScreen = rawAnchor.x > availableRight + menuMargin
            || rawAnchor.y > availableBottom + menuMargin
            || rawAnchor.x < availableLeft - menuMargin
            || rawAnchor.y < availableTop - menuMargin;
        if (!outsideLogicalScreen)
            return anchorWithPosition(rawAnchor, rawAnchor.x, rawAnchor.y, scale, false);

        return anchorWithPosition(rawAnchor, rawAnchor.x / scale, rawAnchor.y / scale, scale, true);
    }

    function nearestPanelEdge(point) {
        const screenRight = screenLeft + screenWidth;
        const screenBottom = screenTop + screenHeight;
        const candidates = [
            {"edge": "bottom", "thickness": Math.max(0, screenBottom - availableBottom), "distance": Math.abs(point.y - screenBottom)},
            {"edge": "top", "thickness": Math.max(0, availableTop - screenTop), "distance": Math.abs(point.y - screenTop)},
            {"edge": "right", "thickness": Math.max(0, screenRight - availableRight), "distance": Math.abs(point.x - screenRight)},
            {"edge": "left", "thickness": Math.max(0, availableLeft - screenLeft), "distance": Math.abs(point.x - screenLeft)}
        ];

        let best = candidates[0];
        for (let i = 1; i < candidates.length; i++) {
            const candidate = candidates[i];
            if (candidate.thickness > 0 && (best.thickness <= 0 || candidate.distance < best.distance))
                best = candidate;
        }
        if (best.thickness > 0)
            return best;

        for (let j = 1; j < candidates.length; j++) {
            if (candidates[j].distance < best.distance)
                best = candidates[j];
        }
        return best;
    }

    function trayAnchorFromClick(rawAnchor) {
        const point = normalizeAnchor(rawAnchor);
        if (!point.valid)
            return point;

        const panel = nearestPanelEdge(point);
        const iconSize = clamp(panel.thickness > 0 ? panel.thickness : 32, 24, 48);
        let x = point.x;
        let y = point.y;
        let width = iconSize;
        let height = iconSize;

        if (panel.edge === "bottom") {
            x = clamp(point.x - iconSize / 2, availableLeft, availableRight - iconSize);
            y = availableBottom;
            height = Math.max(panel.thickness, iconSize);
        } else if (panel.edge === "top") {
            x = clamp(point.x - iconSize / 2, availableLeft, availableRight - iconSize);
            y = availableTop;
            height = Math.max(panel.thickness, iconSize);
        } else if (panel.edge === "right") {
            x = availableRight;
            y = clamp(point.y - iconSize / 2, availableTop, availableBottom - iconSize);
            width = Math.max(panel.thickness, iconSize);
        } else {
            x = availableLeft;
            y = clamp(point.y - iconSize / 2, availableTop, availableBottom - iconSize);
            width = Math.max(panel.thickness, iconSize);
        }

        return {
            "valid": true,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "source": "trayGeometry",
            "edge": panel.edge,
            "clickX": point.x,
            "clickY": point.y,
            "scale": point.scale,
            "normalized": point.normalized
        };
    }

    function placementFromAnchor(rawAnchor) {
        if (!rawAnchor || !rawAnchor.valid)
            return invalidAnchor();
        if (rawAnchor.source === "trayClick")
            return trayAnchorFromClick(rawAnchor);
        return normalizeAnchor(rawAnchor);
    }

    function contextMenuX() {
        const minX = availableLeft + menuMargin;
        const maxX = availableRight - menuWidth - menuMargin;
        if (!placementAnchor.valid)
            return clamp(availableRight - menuWidth - menuMargin, minX, maxX);

        if (placementAnchor.source === "trayGeometry") {
            if (placementAnchor.edge === "right")
                return clamp(placementAnchor.x - menuWidth, minX, maxX);
            return clamp(placementAnchor.x, minX, maxX);
        }

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

        if (placementAnchor.source === "trayGeometry" && placementAnchor.edge === "bottom")
            return clamp(placementAnchor.y - menuHeight, minY, maxY);
        if (placementAnchor.source === "trayGeometry" && placementAnchor.edge === "top")
            return clamp(placementAnchor.y, minY, maxY);

        let proposed = placementAnchor.y;
        if (proposed + menuHeight > availableBottom - menuMargin)
            proposed = placementAnchor.y - menuHeight;
        return clamp(proposed, minY, maxY);
    }

    function activeLayoutName() {
        if (!settings.layouts || activeLayout < 0 || activeLayout >= settings.layouts.length)
            return "Unknown";
        return settings.layouts[activeLayout].name;
    }

    function trayToolTip() {
        return "Active layout: " + activeLayoutName() + "\n" + integrationStatus;
    }

    function modifierLabel() {
        const labels = {
            "shift": "Shift",
            "control": "Ctrl",
            "alt": "Alt",
            "meta": "Meta"
        };
        const modifiers = settings.modifiers || ["shift"];
        const names = [];
        for (let i = 0; i < modifiers.length; i++) {
            const key = String(modifiers[i]).toLowerCase();
            names.push(labels[key] || modifiers[i]);
        }
        return names.length > 0 ? names.join("+") : "Shift";
    }

    function showMenuFromAnchor(rawAnchor) {
        applyBackendState();
        anchor = rawAnchor && rawAnchor.valid ? rawAnchor : invalidAnchor();
        placementAnchor = placementFromAnchor(anchor);
        lastTrayAnchor = placementAnchor;
        actionEmitted = false;
        closeOnDeactivate = false;
        menuVisible = true;
        raise();
        requestActivate();
        placementLogTimer.restart();
        closeTimer.restart();
    }

    function showMenuFromTray() {
        showMenuFromAnchor(lastTrayAnchor);
    }

    function toggleTrayMenuAt(rawAnchor) {
        if (menuVisible)
            closeMenu(true);
        else
            showMenuFromAnchor(rawAnchor);
    }

    function toggleTrayMenu() {
        toggleTrayMenuAt(lastTrayAnchor);
    }

    function orderedLayoutIndexes() {
        // Order top-to-bottom: least-recently-used at the top, most-recently
        // -used (the active layout) at the bottom, nearest the panel.
        const layouts = settings.layouts || [];
        const mru = settings.layout_mru || [];
        function rank(i) {
            const layout = layouts[i];
            const r = layout ? mru.indexOf(layout.id) : -1;
            return r >= 0 ? r : mru.length + (layouts.length - i);
        }
        const indexes = [];
        for (let i = 0; i < layouts.length; i++)
            indexes.push(i);
        const bottomUp = settings.layout_menu_bottom_up !== false;
        indexes.sort((a, b) => bottomUp ? rank(b) - rank(a) : rank(a) - rank(b));
        return indexes;
    }

    function emitAction(action, closeAfter) {
        const shouldClose = closeAfter !== false || !trayMode;
        if (shouldClose) {
            actionEmitted = true;
            deactivateCloseTimer.stop();
        } else {
            action.closeMenu = false;
        }
        const payload = JSON.stringify(action);
        let posted = false;
        if (backendMode)
            posted = backend.invoke_action(payload);
        else {
            print(actionPrefix + payload);
            posted = true;
        }
        if (posted && backendMode)
            applyBackendState();
        if (!shouldClose)
            return posted;
        if (backendMode && action.action === "quit")
            Qt.quit();
        else if (trayMode)
            closeMenu(false);
        else
            Qt.quit();
        return posted;
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
            "trayGeometry": lastTrayAnchor,
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
        if (backendMode)
            backend.invoke_action(payload);
        else
            console.log("FANZYZONES_PLACEMENT " + payload);
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
            placementAnchor = placementFromAnchor(anchor);
            lastTrayAnchor = placementAnchor;
        }
        actionEmitted = false;
        closeOnDeactivate = false;
        menuVisible = true;
        raise();
        requestActivate();
        placementLogTimer.restart();
        closeTimer.restart();
    }

    function applyTrayCommand(commandJson) {
        if (!commandJson || commandJson.length === 0)
            return;

        try {
            const command = JSON.parse(commandJson);
            if (command.sequence !== undefined && command.sequence === commandSequence)
                return;
            if (command.sequence !== undefined)
                commandSequence = command.sequence;
            if (command.action === "toggleTrayMenu")
                toggleTrayMenuAt(command.anchor);
            else
                applyCommand(command);
        } catch (error) {
        }
    }

    function closeMenu(emitClosed) {
        if (!trayMode) {
            Qt.quit();
            return;
        }

        deactivateCloseTimer.stop();
        closeTimer.stop();
        closeOnDeactivate = false;
        menuVisible = false;
        actionEmitted = false;
    }

    function applyBackendState() {
        if (!backendMode)
            return;

        try {
            if (backend.settings_json !== undefined && backend.settings_json.length > 0) {
                settings = JSON.parse(backend.settings_json);
                activeLayout = activeLayoutFromSettings(settings);
            }
        } catch (error) {
        }
        if (backend.status !== undefined && backend.status.length > 0)
            integrationStatus = backend.status;
        if (backend.tray_icon_source !== undefined && backend.tray_icon_source.length > 0)
            trayIconSource = backend.tray_icon_source;
    }

    Component.onCompleted: {
        applyBackendState();
        if (trayMode)
            return;

        root.raise();
        root.requestActivate();
        placementLogTimer.start();
        closeTimer.start();
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

    Connections {
        target: root.backend
        ignoreUnknownSignals: true

        function onSettings_jsonChanged() {
            root.applyBackendState();
        }

        function onStatusChanged() {
            root.applyBackendState();
        }

        function onTray_icon_sourceChanged() {
            root.applyBackendState();
        }

        function onTray_command_jsonChanged() {
            root.applyTrayCommand(root.backend.tray_command_json);
        }
    }

    Rectangle {
        x: 0
        y: 0
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
                        text: settings.snap_mode === "modifier" ? "Hold " + root.modifierLabel() + " and drag" : "Use " + root.modifierLabel() + " drag"
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
