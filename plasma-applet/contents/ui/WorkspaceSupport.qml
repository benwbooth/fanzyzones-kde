import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: support
    width: 660
    height: reportMode ? 600 : 300
    minimumWidth: 500
    minimumHeight: reportMode ? 400 : 260
    visible: false
    color: "#202428"
    property bool reportMode: false
    property bool busy: false
    property string message: ""
    property string link: ""
    property string draft: ""
    function open(report) {
        reportMode = report; busy = true; message = report ? "Preparing diagnostics…" : "Checking releases…";
        link = ""; draft = ""; title = report ? "FanzyZones — Report a problem" : "FanzyZones — Updates";
        show(); raise(); requestActivate();
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 14
        Label { text: support.message; wrapMode: Text.Wrap; Layout.fillWidth: true }
        Label {
            visible: support.reportMode && !support.busy
            text: "Review and edit this draft before opening GitHub. Window titles, application names, file paths, and logs are omitted."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        ScrollView {
            visible: support.reportMode
            Layout.fillWidth: true
            Layout.fillHeight: true
            TextArea { id: report; text: support.draft; wrapMode: TextEdit.Wrap; selectByMouse: true }
        }
        Item { visible: !support.reportMode; Layout.fillHeight: true }
        RowLayout {
            Layout.fillWidth: true
            Button { text: "Close"; onClicked: support.close() }
            Item { Layout.fillWidth: true }
            Button {
                visible: support.reportMode
                enabled: !support.busy && support.draft !== ""
                text: "Copy draft"
                onClicked: { report.selectAll(); report.copy(); report.deselect(); }
            }
            Button {
                enabled: !support.busy && support.link !== ""
                text: support.reportMode ? "Open issue draft…" : "View release…"
                onClicked: Qt.openUrlExternally(support.link + (support.reportMode
                    ? "?title=" + encodeURIComponent("Workspace problem") + "&body=" + encodeURIComponent(report.text) : ""))
            }
        }
    }
}
