// SPDX-License-Identifier: MIT
// Test-only input injection. Never built into or installed by the application.
#include <input.h>
#include <pointer_input.h>
#include <window.h>
#include <effect/quickeffect.h>
#include <effect/effecthandler.h>
#include <KDecoration3/Decoration>
#include <KDecoration3/DecorationButton>
#include <QCoreApplication>
#include <QDBusConnection>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlExtensionPlugin>
#include <QQuickItem>
#include <qqml.h>
#include <chrono>

class Driver : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "com.benwbooth.FanzyZonesTest")
public:
    explicit Driver(QObject *parent = nullptr) : QObject(parent)
    {
        const bool isolated = QCoreApplication::arguments().contains("--virtual")
            && qEnvironmentVariable("WAYLAND_DISPLAY").startsWith("fanzyzones-test-")
            && qEnvironmentVariable("XDG_RUNTIME_DIR").startsWith("/tmp/fanzyzones-parity.");
        if (!isolated || QCoreApplication::applicationVersion() != KWIN_VERSION_STRING)
            qFatal("FanzyZones test input requires the matching isolated virtual compositor");
        auto bus = QDBusConnection::sessionBus();
        if (!bus.registerService("com.benwbooth.FanzyZonesTest")
            || !bus.registerObject("/Input", this, QDBusConnection::ExportAllSlots))
            qFatal("Cannot register test input service");
    }
    static std::chrono::microseconds now()
    {
        return std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now().time_since_epoch());
    }
public Q_SLOTS:
    QString Scenes()
    {
        QJsonArray result;
        auto *effect = qobject_cast<KWin::QuickSceneEffect *>(parent());
        if (!effect) return QString::fromLatin1(parent() ? parent()->metaObject()->className() : "no parent");
        for (auto *screen : KWin::effects->screens()) {
            auto *view = effect->viewForScreen(screen);
            if (!view) continue;
            auto *root = view->rootItem();
            result.append(QJsonObject{{"width", root->width()}, {"height", root->height()},
                {"x", view->geometry().x()}, {"y", view->geometry().y()},
                {"originX", root->property("originX").toDouble()},
                {"originY", root->property("originY").toDouble()},
                {"children", int(root->childItems().size())}, {"dirty", view->isDirty()}});
        }
        return QString::fromUtf8(QJsonDocument(result).toJson(QJsonDocument::Compact));
    }
    void PinchBegin(int fingers) { KWin::input()->pointer()->processPinchGestureBegin(fingers, now()); }
    void PinchUpdate(double scale) { KWin::input()->pointer()->processPinchGestureUpdate(scale, 0, {}, now()); }
    void PinchEnd(bool cancel)
    {
        if (cancel) KWin::input()->pointer()->processPinchGestureCancelled(now());
        else KWin::input()->pointer()->processPinchGestureEnd(now());
    }
    QString DecorationAtPointer()
    {
        QJsonObject result;
        auto *window = KWin::input()->findToplevel(KWin::input()->globalPointer());
        if (!window) return "{}";
        result["id"] = window->internalId().toString();
        auto *decoration = window->decoration();
        if (decoration) {
            auto box = [window](QRectF r) {
                r.translate(window->pos());
                return QJsonObject{{"x", r.x()}, {"y", r.y()}, {"width", r.width()}, {"height", r.height()}};
            };
            result["title"] = box(decoration->titleBar());
            QJsonArray buttons;
            for (auto *button : decoration->findChildren<KDecoration3::DecorationButton *>()) {
                if (!button->isVisible()) continue;
                auto entry = box(button->geometry());
                entry["type"] = int(button->type());
                entry["maximize"] = button->type() == KDecoration3::DecorationButtonType::Maximize;
                buttons.append(entry);
            }
            result["buttons"] = buttons;
        }
        return QString::fromUtf8(QJsonDocument(result).toJson(QJsonDocument::Compact));
    }
};

class TestPlugin : public QQmlExtensionPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)
public:
    void registerTypes(const char *uri) override { qmlRegisterType<Driver>(uri, 1, 0, "Driver"); }
};
#include "Driver.moc"
