import QtQuick
import QtQuick.Layouts
import QtQuick.Window

import org.kde.kirigami as Kirigami
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as P5Support

PlasmoidItem {
    id: main

    // The daemon is gone: the applet runs the fanzyzones-kde CLI on demand.
    readonly property string cli: "$HOME/.local/share/fanzyzones-kde/fanzyzones-kde"

    property var settings: ({ "active_layout": 0, "layouts": [] })
    property int activeLayout: activeLayoutFromSettings(settings)
    property string integrationStatus: "Starting FanzyZones..."
    property int cliNonce: 0
    property var executableCallbacks: ({})

    switchWidth: Kirigami.Units.gridUnit * 18
    switchHeight: Kirigami.Units.gridUnit * 26
    Layout.minimumWidth: Kirigami.Units.iconSizes.smallMedium
    Layout.minimumHeight: Kirigami.Units.iconSizes.smallMedium

    Plasmoid.icon: "fanzyzones-kde"
    Plasmoid.status: PlasmaCore.Types.ActiveStatus
    toolTipMainText: "FanzyZones"
    toolTipSubText: "Active layout: " + activeLayoutName() + "\n" + integrationStatus

    function activeLayoutFromSettings(value) {
        if (value && value.active_layout !== undefined)
            return value.active_layout;
        if (value && value.activeLayout !== undefined)
            return value.activeLayout;
        return 0;
    }

    function activeLayoutName() {
        if (!settings.layouts || activeLayout < 0 || activeLayout >= settings.layouts.length)
            return "Unknown";
        return settings.layouts[activeLayout].name;
    }

    function applyStateJson(stateJson) {
        const state = JSON.parse(stateJson);
        if (state.settings !== undefined)
            settings = state.settings;
        activeLayout = activeLayoutFromSettings(settings);
        if (state.status !== undefined)
            integrationStatus = state.status;
    }

    P5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            const cb = main.executableCallbacks[source];
            delete main.executableCallbacks[source];
            executable.disconnectSource(source);
            if (cb)
                cb((data["stdout"] || "").trim());
        }
    }

    function shellQuote(text) {
        return "'" + String(text).replace(/'/g, "'\\''") + "'";
    }

    function runCli(commandSuffix, onResult) {
        // Unique trailing shell comment so identical commands map to distinct sources.
        const source = main.cli + " " + commandSuffix + " #" + (main.cliNonce++);
        main.executableCallbacks[source] = onResult || function() {};
        executable.connectSource(source);
    }

    function refreshState() {
        runCli("state-json", function(out) {
            try {
                applyStateJson(out);
            } catch (error) {
                integrationStatus = "Error: " + error;
            }
        });
    }

    function invokeAction(action, closeAfter) {
        const shouldClose = closeAfter !== false;
        if (!shouldClose)
            action.closeMenu = false;
        runCli("invoke-action " + shellQuote(JSON.stringify(action)), function(out) {
            try {
                applyStateJson(out);
            } catch (error) {
                // keep prior state on parse failure
            }
            if (shouldClose)
                main.closeMenu();
        });
        return true;
    }

    Component.onCompleted: refreshState()

    preferredRepresentation: compactRepresentation

    // We manage our own popup (see menuPopup) instead of the System Tray's
    // shared expanded popup, which Plasma locks to a fixed grid-unit size for
    // every docked applet. This lets the menu size itself to its content.
    function toggleMenu() {
        if (menuPopup.visible) {
            menuPopup.visible = false;
        } else {
            refreshState();
            menuPopup.visible = true;
            menuPopup.requestActivate();
        }
    }

    function closeMenu() {
        menuPopup.visible = false;
    }

    compactRepresentation: MouseArea {
        acceptedButtons: Qt.LeftButton
        hoverEnabled: true
        onClicked: main.toggleMenu()

        Kirigami.Icon {
            anchors.centerIn: parent
            width: Math.max(Kirigami.Units.iconSizes.smallMedium, Math.min(parent.width, parent.height) - 2)
            height: width
            source: Qt.resolvedUrl("../images/fanzyzones-kde.svg")
            isMask: true
        }
    }

    // Placeholder only: the System Tray requires a fullRepresentation to render
    // the applet's icon, but we never expand it. The real (full-height) menu is
    // shown by menuPopup below, which the System Tray cannot size-clamp.
    fullRepresentation: Item {
        implicitWidth: 1
        implicitHeight: 1
    }

    PlasmaCore.PopupPlasmaWindow {
        id: menuPopup

        // Anchored to the tray icon (visualParent); PopupPlasmaWindow computes
        // the icon's screen coordinates and panel edge for us, exactly like
        // every native Plasma tray popup. No manual positioning.
        visualParent: main.compactRepresentationItem
        visible: false
        width: dialog.implicitWidth
        height: dialog.implicitHeight
        margin: Kirigami.Units.smallSpacing
        popupDirection: {
            switch (Plasmoid.location) {
            case PlasmaCore.Types.TopEdge:
                return Qt.BottomEdge;
            case PlasmaCore.Types.LeftEdge:
                return Qt.RightEdge;
            case PlasmaCore.Types.RightEdge:
                return Qt.LeftEdge;
            default:
                return Qt.TopEdge;
            }
        }

        onActiveChanged: {
            if (!active)
                menuPopup.visible = false;
        }

        mainItem: Item {
        id: dialog

        readonly property int menuWidth: 346
        // Grow to fit the whole menu without scrolling; only cap at the available screen height.
        readonly property int maxMenuHeight: Math.max(260, Screen.desktopAvailableHeight - 80)
        readonly property int menuHeight: Math.min(Math.max(menuColumn.implicitHeight + 12, 260), maxMenuHeight)
        readonly property color accent: Qt.rgba(
            main.settings.highlight_color ? main.settings.highlight_color.red : 0.18,
            main.settings.highlight_color ? main.settings.highlight_color.green : 0.48,
            main.settings.highlight_color ? main.settings.highlight_color.blue : 0.96,
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

        width: menuWidth
        height: menuHeight
        implicitWidth: menuWidth
        implicitHeight: menuHeight
        focus: true

        SystemPalette {
            id: systemPalette
            colorGroup: SystemPalette.Active
        }

        Keys.onEscapePressed: main.closeMenu()

        function modifierLabel() {
            const labels = {
                "shift": "Shift",
                "control": "Ctrl",
                "alt": "Alt",
                "meta": "Meta"
            };
            const modifiers = main.settings.modifiers || ["shift"];
            const names = [];
            for (let i = 0; i < modifiers.length; i++) {
                const key = String(modifiers[i]).toLowerCase();
                names.push(labels[key] || modifiers[i]);
            }
            return names.length > 0 ? names.join("+") : "Shift";
        }

        function orderedLayoutIndexes() {
            // Order top-to-bottom: least-recently-used at the top, most-recently
            // -used (the active layout) at the bottom, nearest the panel.
            const layouts = main.settings.layouts || [];
            const mru = main.settings.layout_mru || [];
            function rank(i) {
                const layout = layouts[i];
                const r = layout ? mru.indexOf(layout.id) : -1;
                // Not in MRU => least recent; keep natural order above the MRU items.
                return r >= 0 ? r : mru.length + (layouts.length - i);
            }
            const indexes = [];
            for (let i = 0; i < layouts.length; i++)
                indexes.push(i);
            // Descending rank puts the active layout last (bottom); reverse for top.
            const bottomUp = main.settings.layout_menu_bottom_up !== false;
            indexes.sort((a, b) => bottomUp ? rank(b) - rank(a) : rank(a) - rank(b));
            return indexes;
        }

        function zoneRect(zone, area) {
            return Qt.rect(
                area.x + zone.x * area.width,
                area.y + zone.y * area.height,
                Math.max(3, zone.width * area.width),
                Math.max(3, zone.height * area.height)
            );
        }

        Rectangle {
            anchors.fill: parent
            radius: 4
            color: dialog.menuBg
            border.color: dialog.borderColor
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
                        model: dialog.orderedLayoutIndexes()

                        delegate: LayoutRow {
                            required property int modelData

                            width: menuColumn.width
                            layoutIndex: modelData
                            layout: main.settings.layouts[modelData]
                            active: modelData === main.activeLayout
                            accent: dialog.accent

                            onSetActive: function(index) {
                                if (main.invokeAction({"action": "setLayout", "layout": index}, false))
                                    main.activeLayout = index;
                            }

                            onSnapZone: function(index, zone) {
                                main.invokeAction({"action": "snap", "layout": index, "zone": zone}, false);
                            }

                            onEditLayout: function(index) {
                                main.closeMenu();
                                main.invokeAction({"action": "editLayout", "layout": index});
                            }

                            onDeleteLayout: function(index) {
                                main.invokeAction({"action": "deleteLayout", "layout": index});
                            }
                        }
                    }

                    Separator { width: parent.width }

                    MenuAction {
                        width: parent.width
                        text: "New Custom Layout…"
                        onClicked: {
                            main.closeMenu();
                            main.invokeAction({"action": "createLayout"});
                        }
                    }

                    Separator { width: parent.width }

                    MenuAction {
                        width: parent.width
                        text: "Settings..."
                        onClicked: {
                            main.closeMenu();
                            Plasmoid.internalAction("configure").trigger();
                        }
                    }

                    Separator { width: parent.width }

                    MenuAction {
                        width: parent.width
                        text: "Quit FanzyZones"
                        onClicked: main.invokeAction({"action": "quit"})
                    }
                }
            }
        }

        component Separator: Rectangle {
            height: 1
            color: dialog.separatorColor
        }

        component SectionLabel: Text {
            leftPadding: 6
            rightPadding: 6
            text: ""
            color: dialog.subtleTextColor
            font.pixelSize: 11
            font.bold: true
        }

        component MenuAction: Rectangle {
            id: actionRoot

            signal clicked()
            property alias text: label.text
            property bool actionEnabled: true
            property color labelColor: dialog.textColor

            height: 30
            radius: 3
            color: mouse.containsMouse && actionEnabled ? dialog.hoverBg : "transparent"
            opacity: actionEnabled ? 1 : 0.78

            Text {
                id: label
                anchors.verticalCenter: parent.verticalCenter
                x: 8
                width: parent.width - 16
                elide: Text.ElideRight
                color: mouse.containsMouse && actionRoot.actionEnabled
                    ? dialog.hoverTextColor
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
            color: mouse.containsMouse ? dialog.hoverBg : (checked ? dialog.checkedBg : "transparent")
            border.color: checked && !mouse.containsMouse ? accent : "transparent"
            border.width: checked && !mouse.containsMouse ? 1 : 0

            Text {
                id: label
                anchors.centerIn: parent
                width: parent.width - 12
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                color: mouse.containsMouse ? dialog.hoverTextColor : (checked ? pillRoot.accent : dialog.textColor)
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
                color: dialog.cardBg
                border.color: active ? row.accent : dialog.separatorColor
                border.width: active ? 1 : 0
            }

            MouseArea {
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
                color: labelMouse.containsMouse ? dialog.hoverBg : "transparent"

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
                    color: labelMouse.containsMouse ? dialog.hoverTextColor : dialog.textColor
                    font.pixelSize: 12
                    font.bold: row.active
                }

                Text {
                    x: 3
                    y: 36
                    visible: row.active
                    text: "Active"
                    color: labelMouse.containsMouse ? dialog.hoverTextColor : row.accent
                    font.pixelSize: 10
                }

                Row {
                    x: 3
                    y: parent.height - 18
                    spacing: 12
                    visible: !row.layout.is_built_in

                    Text {
                        text: "Edit"
                        color: labelMouse.containsMouse ? dialog.hoverTextColor : (editMouse.containsMouse ? row.accent : dialog.subtleTextColor)
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
                        color: labelMouse.containsMouse ? dialog.hoverTextColor : (deleteMouse.containsMouse ? dialog.dangerColor : dialog.subtleTextColor)
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
                color: dialog.darkMode ? Qt.rgba(1, 1, 1, 0.035) : Qt.rgba(0, 0, 0, 0.035)
                border.color: row.active ? row.accent : dialog.borderColor
                border.width: row.active ? 2 : 1

                Repeater {
                    model: row.layout.zones

                    delegate: Rectangle {
                        required property var modelData
                        required property int index

                        readonly property rect zr: dialog.zoneRect(modelData, Qt.rect(0, 0, diagram.width, diagram.height))
                        x: zr.x + 2
                        y: zr.y + 2
                        width: Math.max(4, zr.width - 4)
                        height: Math.max(4, zr.height - 4)
                        radius: 2
                        color: zoneMouse.containsMouse
                            ? dialog.hoverBg
                            : Qt.rgba(row.accent.r, row.accent.g, row.accent.b, dialog.darkMode ? 0.28 : 0.18)
                        border.color: zoneMouse.containsMouse
                            ? dialog.hoverBg
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
    }
}
