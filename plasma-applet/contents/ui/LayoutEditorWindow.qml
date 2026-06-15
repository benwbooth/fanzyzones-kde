// Visual layout editor hosted by the plasmoid (runs in plasmashell's QML engine
// with the host Qt — the backend binary no longer needs Qt). A faithful port of
// the macOS FanzyZones editor (Sources/FanzyZones/Editor/EditorView.swift): a
// fixed-aspect canvas of normalized (0..1, top-left origin) zones, edited with a
// single drag gesture (body = move, bottom-right corner = resize), plus add /
// split / delete / name / snap-to-grid.
//
// IO is in-process QML: open(data) populates it; on Save it emits
// submitted({name, id, zones:[{x,y,width,height}]}); Cancel/close just hides.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

ApplicationWindow {
    id: win
    visible: false
    width: 900
    height: 640
    minimumWidth: 760
    minimumHeight: 560
    title: qsTr("FanzyZones — Layout Editor")

    // Emitted on Save with { name, id, zones:[{x,y,width,height}] }.
    signal submitted(var result)

    // ---- model -----------------------------------------------------------
    property string layoutName: "My Layout"
    property string layoutId: ""
    property bool snapToGrid: true
    property int selectedIndex: -1
    property real canvasAspect: Screen.height > 0 ? Screen.width / Screen.height : 1.6

    readonly property real gridDivisions: 24
    readonly property real minSize: 0.06
    readonly property real handleHit: 24   // px corner that starts a resize

    property var dragState: null

    ListModel { id: zonesModel }

    // Open the editor for `data` = { name, id, isBuiltIn, zones:[{x,y,width,height}] }.
    function open(data) {
        zonesModel.clear();
        win.selectedIndex = -1;
        win.dragState = null;
        win.snapToGrid = true;
        if (data) {
            win.layoutName = data.isBuiltIn ? (data.name + " Copy") : (data.name || "My Layout");
            win.layoutId = data.isBuiltIn ? "" : (data.id || "");
            const zs = data.zones || [];
            for (let k = 0; k < zs.length; k++)
                zonesModel.append(clampNormalized({ "x": zs[k].x, "y": zs[k].y, "width": zs[k].width, "height": zs[k].height }));
        } else {
            win.layoutName = "My Layout";
            win.layoutId = "";
        }
        if (zonesModel.count === 0) {
            zonesModel.append({ "x": 0.0, "y": 0.0, "width": 0.5, "height": 1.0 });
            zonesModel.append({ "x": 0.5, "y": 0.0, "width": 0.5, "height": 1.0 });
        }
        win.show();
        win.raise();
        win.requestActivate();
    }

    // ---- geometry helpers (ported from the Swift version) ----------------
    function clampNormalized(r) {
        // Cap size at the full canvas first; otherwise 1 - w goes negative and
        // the origin clamp below would push the pane off the top-left edge.
        const w0 = Math.min(1, Math.max(minSize, r.width));
        const h0 = Math.min(1, Math.max(minSize, r.height));
        const x = Math.min(Math.max(0, r.x), 1 - w0);
        const y = Math.min(Math.max(0, r.y), 1 - h0);
        const w = Math.min(w0, 1 - x);
        const h = Math.min(h0, 1 - y);
        return { "x": x, "y": y, "width": w, "height": h };
    }
    function snapOne(v) { return Math.round(v * gridDivisions) / gridDivisions; }
    function snapNormalized(r) {
        return clampNormalized({ "x": snapOne(r.x), "y": snapOne(r.y),
                                 "width": snapOne(r.width), "height": snapOne(r.height) });
    }
    function applyRect(index, r) {
        const c = win.snapToGrid ? snapNormalized(r) : clampNormalized(r);
        zonesModel.setProperty(index, "x", c.x);
        zonesModel.setProperty(index, "y", c.y);
        zonesModel.setProperty(index, "width", c.width);
        zonesModel.setProperty(index, "height", c.height);
    }
    // Topmost zone (selected first) under a canvas point; isResize in BR handle.
    function hitTest(px, py, cw, ch) {
        const order = [];
        for (let i = 0; i < zonesModel.count; i++) order.push(i);
        order.sort(function(a, b) {
            return (a === win.selectedIndex ? 1 : 0) - (b === win.selectedIndex ? 1 : 0);
        });
        for (let j = order.length - 1; j >= 0; j--) {
            const z = zonesModel.get(order[j]);
            const rx = z.x * cw, ry = z.y * ch, rw = z.width * cw, rh = z.height * ch;
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
        const i = win.selectedIndex;
        if (i < 0 || i >= zonesModel.count) return;
        const z = zonesModel.get(i);
        let a, b;
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
        const out = [];
        for (let i = 0; i < zonesModel.count; i++) {
            const z = zonesModel.get(i);
            out.push({ "x": z.x, "y": z.y, "width": z.width, "height": z.height });
        }
        out.sort(function(p, q) { return p.y !== q.y ? p.y - q.y : p.x - q.x; });
        win.submitted({ "name": win.layoutName.trim(), "id": win.layoutId, "zones": out });
        win.visible = false;
    }
    function cancel() { win.visible = false; }

    onClosing: win.visible = false

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
                    anchors.fill: parent
                    onPressed: function(mouse) {
                        const hit = win.hitTest(mouse.x, mouse.y, canvas.width, canvas.height);
                        if (!hit) { win.selectedIndex = -1; win.dragState = null; return; }
                        win.selectedIndex = hit.index;
                        const z = zonesModel.get(hit.index);
                        win.dragState = {
                            "index": hit.index, "isResize": hit.isResize,
                            "base": { "x": z.x, "y": z.y, "width": z.width, "height": z.height },
                            "startX": mouse.x, "startY": mouse.y
                        };
                    }
                    onPositionChanged: function(mouse) {
                        if (!win.dragState) return;
                        const lx = Math.min(Math.max(0, mouse.x), canvas.width);
                        const ly = Math.min(Math.max(0, mouse.y), canvas.height);
                        const tx = (lx - win.dragState.startX) / canvas.width;
                        const ty = (ly - win.dragState.startY) / canvas.height;
                        const b = win.dragState.base;
                        const r = win.dragState.isResize
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
