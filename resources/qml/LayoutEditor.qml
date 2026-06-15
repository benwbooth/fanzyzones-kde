// Standalone visual layout editor, launched by the CLI via the `qml` runtime.
// A faithful port of the macOS FanzyZones editor (Sources/FanzyZones/Editor/
// EditorView.swift): a fixed-aspect canvas of normalized (0..1, top-left origin)
// zones, edited with a single drag gesture (body = move, bottom-right corner =
// resize), plus add / split / delete / name / snap-to-grid.
//
// IO runs through the in-process EditorBridge QObject (no external qml/IPC):
//   input : bridge.inputJson
//           { "name": str, "id": str|null, "isBuiltIn": bool,
//             "zones": [ { "x":r, "y":r, "width":r, "height":r }, ... ] }
//   output: on Save, bridge.submit(<json>)  ({name, id, zones:[{x,y,width,height}]})
//           then Qt.quit(); Cancel/close just Qt.quit() with no result.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import FanzyZones 1.0

ApplicationWindow {
    id: win
    visible: true
    width: 900
    height: 640
    minimumWidth: 760
    minimumHeight: 560
    title: qsTr("FanzyZones — Layout Editor")

    // ---- model -----------------------------------------------------------
    property string layoutName: "My Layout"
    property string layoutId: ""
    property bool snapToGrid: true
    property int selectedIndex: -1
    property real canvasAspect: Screen.height > 0 ? Screen.width / Screen.height : 1.6

    readonly property real gridDivisions: 24
    readonly property real minSize: 0.06
    readonly property real handleHit: 24   // px corner that starts a resize

    // drag session (null when idle)
    property var dragState: null

    ListModel { id: zonesModel }

    EditorBridge { id: bridge }

    Component.onCompleted: loadInput()

    function loadInput() {
        var data = null;
        try {
            if (bridge.inputJson && bridge.inputJson.length > 0)
                data = JSON.parse(bridge.inputJson);
        } catch (e) {
            console.log("FZEDITOR input parse failed: " + e);
        }
        if (data) {
            win.layoutName = data.isBuiltIn ? (data.name + " Copy") : (data.name || "My Layout");
            win.layoutId = data.isBuiltIn ? "" : (data.id || "");
            var zs = data.zones || [];
            for (var k = 0; k < zs.length; k++)
                zonesModel.append(clampNormalized({ "x": zs[k].x, "y": zs[k].y, "width": zs[k].width, "height": zs[k].height }));
        }
        if (zonesModel.count === 0) {
            zonesModel.append({ "x": 0.0, "y": 0.0, "width": 0.5, "height": 1.0 });
            zonesModel.append({ "x": 0.5, "y": 0.0, "width": 0.5, "height": 1.0 });
        }
    }

    // ---- geometry helpers (ported from the Swift version) ----------------
    function clampNormalized(r) {
        // Cap size at the full canvas first; otherwise 1 - w goes negative and
        // the origin clamp below would push the pane off the top-left edge.
        var w = Math.min(1, Math.max(minSize, r.width));
        var h = Math.min(1, Math.max(minSize, r.height));
        var x = Math.min(Math.max(0, r.x), 1 - w);
        var y = Math.min(Math.max(0, r.y), 1 - h);
        w = Math.min(w, 1 - x);
        h = Math.min(h, 1 - y);
        return { "x": x, "y": y, "width": w, "height": h };
    }
    function snapOne(v) { return Math.round(v * gridDivisions) / gridDivisions; }
    function snapNormalized(r) {
        return clampNormalized({ "x": snapOne(r.x), "y": snapOne(r.y),
                                 "width": snapOne(r.width), "height": snapOne(r.height) });
    }

    function applyRect(index, r) {
        var c = win.snapToGrid ? snapNormalized(r) : clampNormalized(r);
        zonesModel.setProperty(index, "x", c.x);
        zonesModel.setProperty(index, "y", c.y);
        zonesModel.setProperty(index, "width", c.width);
        zonesModel.setProperty(index, "height", c.height);
    }

    // Topmost zone (selected first) under a canvas-space point; isResize when in
    // its bottom-right handle.
    function hitTest(px, py, cw, ch) {
        var order = [];
        for (var i = 0; i < zonesModel.count; i++) order.push(i);
        order.sort(function(a, b) {
            return (a === win.selectedIndex ? 1 : 0) - (b === win.selectedIndex ? 1 : 0);
        });
        for (var j = order.length - 1; j >= 0; j--) {
            var z = zonesModel.get(order[j]);
            var rx = z.x * cw, ry = z.y * ch, rw = z.width * cw, rh = z.height * ch;
            if (px >= rx + rw - handleHit && px <= rx + rw && py >= ry + rh - handleHit && py <= ry + rh)
                return { "index": order[j], "isResize": true };
            if (px >= rx && px <= rx + rw && py >= ry && py <= ry + rh)
                return { "index": order[j], "isResize": false };
        }
        return null;
    }

    // ---- mutations -------------------------------------------------------
    function addZone() {
        zonesModel.append({ "x": 0.3, "y": 0.3, "width": 0.4, "height": 0.4 });
        win.selectedIndex = zonesModel.count - 1;
    }
    function deleteSelected() {
        if (win.selectedIndex < 0 || win.selectedIndex >= zonesModel.count) return;
        zonesModel.remove(win.selectedIndex);
        win.selectedIndex = -1;
    }
    function splitSelected(horizontal) {
        var i = win.selectedIndex;
        if (i < 0 || i >= zonesModel.count) return;
        var z = zonesModel.get(i);
        var a, b;
        if (horizontal) {
            a = { "x": z.x, "y": z.y, "width": z.width, "height": z.height / 2 };
            b = { "x": z.x, "y": z.y + z.height / 2, "width": z.width, "height": z.height / 2 };
        } else {
            a = { "x": z.x, "y": z.y, "width": z.width / 2, "height": z.height };
            b = { "x": z.x + z.width / 2, "y": z.y, "width": z.width / 2, "height": z.height };
        }
        zonesModel.set(i, a);
        zonesModel.insert(i + 1, b);
        win.selectedIndex = i;
    }

    function save() {
        // Sort by (y, x), reindex; emit minimal {x,y,width,height}.
        var out = [];
        for (var i = 0; i < zonesModel.count; i++) {
            var z = zonesModel.get(i);
            out.push({ "x": z.x, "y": z.y, "width": z.width, "height": z.height });
        }
        out.sort(function(p, q) { return p.y !== q.y ? p.y - q.y : p.x - q.x; });
        var result = { "name": win.layoutName.trim(), "id": win.layoutId, "zones": out };
        bridge.submit(JSON.stringify(result));
        Qt.quit();
    }
    function cancel() { Qt.quit(); }

    onClosing: Qt.quit()

    // ---- UI --------------------------------------------------------------
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            TextField {
                id: nameField
                Layout.preferredWidth: 220
                placeholderText: qsTr("Layout name")
                text: win.layoutName
                onTextEdited: win.layoutName = text
            }
            Button { text: qsTr("Add Pane"); onClicked: win.addZone() }
            Button { text: qsTr("Split L|R"); enabled: win.selectedIndex >= 0; onClicked: win.splitSelected(false) }
            Button { text: qsTr("Split T/B"); enabled: win.selectedIndex >= 0; onClicked: win.splitSelected(true) }
            Button { text: qsTr("Delete"); enabled: win.selectedIndex >= 0; onClicked: win.deleteSelected() }
            Item { Layout.fillWidth: true }
            CheckBox { text: qsTr("Snap to grid"); checked: win.snapToGrid; onToggled: win.snapToGrid = checked }
        }

        // Fixed-aspect canvas centered in the available area.
        Item {
            id: canvasArea
            Layout.fillWidth: true
            Layout.fillHeight: true

            Rectangle {
                id: canvas
                anchors.centerIn: parent
                width: Math.min(canvasArea.width, canvasArea.height * win.canvasAspect)
                height: width / win.canvasAspect
                radius: 8
                color: Qt.rgba(0.16, 0.17, 0.19, 1)
                border.color: Qt.rgba(1, 1, 1, 0.25)
                border.width: 1

                Repeater {
                    model: zonesModel
                    delegate: Rectangle {
                        required property int index
                        required property var model
                        readonly property bool selected: index === win.selectedIndex
                        // Bind Item geometry to the model's normalized roles. We
                        // must NOT redeclare x/y/width/height as properties (they
                        // are FINAL on Item); just assign the bindings.
                        x: model.x * canvas.width
                        y: model.y * canvas.height
                        width: model.width * canvas.width
                        height: model.height * canvas.height
                        radius: 6
                        color: Qt.rgba(0.20, 0.50, 0.95, selected ? 0.34 : 0.17)
                        border.color: Qt.rgba(0.36, 0.62, 1.0, selected ? 0.95 : 0.55)
                        border.width: selected ? 3 : 1

                        Text {
                            anchors.centerIn: parent
                            text: index + 1
                            color: "white"
                            font.pixelSize: 24
                            font.bold: true
                        }
                        // resize handle
                        Rectangle {
                            width: 16; height: 16; radius: 8
                            x: parent.width - width - 3
                            y: parent.height - height - 3
                            color: Qt.rgba(0.36, 0.62, 1.0, 1)
                            border.color: Qt.rgba(1, 1, 1, 0.85)
                            border.width: 1.5
                        }
                    }
                }

                MouseArea {
                    id: canvasMouse
                    anchors.fill: parent
                    onPressed: function(mouse) {
                        var hit = win.hitTest(mouse.x, mouse.y, canvas.width, canvas.height);
                        if (!hit) { win.selectedIndex = -1; win.dragState = null; return; }
                        win.selectedIndex = hit.index;
                        var z = zonesModel.get(hit.index);
                        win.dragState = {
                            "index": hit.index, "isResize": hit.isResize,
                            "base": { "x": z.x, "y": z.y, "width": z.width, "height": z.height },
                            "startX": mouse.x, "startY": mouse.y
                        };
                    }
                    onPositionChanged: function(mouse) {
                        if (!win.dragState) return;
                        var lx = Math.min(Math.max(0, mouse.x), canvas.width);
                        var ly = Math.min(Math.max(0, mouse.y), canvas.height);
                        var tx = (lx - win.dragState.startX) / canvas.width;
                        var ty = (ly - win.dragState.startY) / canvas.height;
                        var b = win.dragState.base;
                        var r = win.dragState.isResize
                            ? { "x": b.x, "y": b.y, "width": b.width + tx, "height": b.height + ty }
                            : { "x": b.x + tx, "y": b.y + ty, "width": b.width, "height": b.height };
                        win.applyRect(win.dragState.index, r);
                    }
                    onReleased: win.dragState = null
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Label {
                text: zonesModel.count + (zonesModel.count === 1 ? qsTr(" pane") : qsTr(" panes"))
                opacity: 0.7
            }
            Item { Layout.fillWidth: true }
            Button { text: qsTr("Cancel"); onClicked: win.cancel() }
            Button {
                text: qsTr("Save Layout")
                highlighted: true
                enabled: zonesModel.count > 0 && nameField.text.trim().length > 0
                onClicked: win.save()
            }
        }
    }
}
