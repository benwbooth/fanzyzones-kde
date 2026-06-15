import QtQuick
import org.kde.plasma.configuration

ConfigModel {
    ConfigCategory {
        name: i18n("General")
        icon: "preferences-system-windows-actions"
        source: "ConfigGeneral.qml"
    }
    ConfigCategory {
        name: i18n("FanzyZones Shortcuts")
        icon: "configure-shortcuts"
        source: "ConfigShortcuts.qml"
    }
}
