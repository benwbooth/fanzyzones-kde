// SPDX-License-Identifier: MIT
// Build against the exact KWin version in the session: InputEventFilter is a
// private compositor API. The QML module is optional on portable Rust builds.
#include <input.h>
#include <input_event.h>
#include <window.h>
#include <KDecoration3/Decoration>
#include <KDecoration3/DecorationButton>
#include <QCoreApplication>
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QPointer>
#include <QQmlExtensionPlugin>
#include <qqml.h>
#include <QStyleHints>
#include <QIcon>
#include <QImage>
#include <QVariant>
#include <cmath>
#include <algorithm>

class InputBridge : public QObject, public KWin::InputEventFilter
{
    Q_OBJECT
    Q_PROPERTY(QObject *controller READ controller WRITE setController)
    Q_PROPERTY(bool available READ available CONSTANT)
public:
    explicit InputBridge(QObject *parent = nullptr)
        : QObject(parent), InputEventFilter(KWin::InputFilterOrder::GlobalShortcut)
    {
        m_available = KWin::input() && QCoreApplication::applicationVersion() == KWIN_VERSION_STRING;
        if (m_available) KWin::input()->installInputEventFilter(this);
    }
    QObject *controller() const { return m_controller; }
    void setController(QObject *value) { m_controller = value; }
    bool available() const { return m_available; }
    Q_INVOKABLE bool hasDecoration(QObject *object) const
    {
        auto *window = qobject_cast<KWin::Window *>(object);
        return window && window->decoration();
    }
    Q_INVOKABLE QString iconColor(const QVariant &value) const {
        const QImage image = value.value<QIcon>().pixmap(32, 32).toImage();
        if (image.isNull()) return {};
        int buckets[4096] = {};
        int best = 0;
        for (int y = 0; y < image.height(); ++y) {
            for (int x = 0; x < image.width(); ++x) {
                const QColor c = image.pixelColor(x, y);
                if (c.alpha() < 128 || c.saturation() < 50 || c.value() < 40) continue;
                const int bucket = ((c.red() >> 4) << 8) | ((c.green() >> 4) << 4) | (c.blue() >> 4);
                buckets[bucket] += c.saturation();
                if (buckets[bucket] > buckets[best]) best = bucket;
            }
        }
        if (!buckets[best]) return {};
        const QColor dominant(((best >> 8) & 15) * 17, ((best >> 4) & 15) * 17, (best & 15) * 17);
        return dominant.darker(210).name();
    }
    bool enabled() const { return m_controller && m_controller->property("enabled").toBool(); }
    bool desktop() const
    {
        QVariant result;
        if (enabled()) QMetaObject::invokeMethod(m_controller, "nativeDesktopAt", Q_RETURN_ARG(QVariant, result));
        return result.toBool();
    }
    void call(const char *method) { if (m_controller) QMetaObject::invokeMethod(m_controller, method); }
    void call(const char *method, QVariant arg) { if (m_controller) QMetaObject::invokeMethod(m_controller, method, Q_ARG(QVariant, arg)); }
    bool managed(KWin::Window *window) const
    {
        QVariant result;
        if (enabled() && window) QMetaObject::invokeMethod(m_controller, "nativeWindowManaged",
            Q_RETURN_ARG(QVariant, result), Q_ARG(QVariant, window->internalId().toString()));
        return result.toBool();
    }
    void windowAction(KWin::Window *window, const QString &action)
    {
        if (managed(window)) QMetaObject::invokeMethod(m_controller, "nativeWindowAction",
            Q_ARG(QVariant, window->internalId().toString()), Q_ARG(QVariant, action));
    }
    bool pinchGestureBegin(KWin::PointerPinchGestureBeginEvent *event) override
    {
        m_pinch = enabled() && event->fingerCount == 2
            && (KWin::input()->keyboardModifiers().testFlag(Qt::AltModifier) || desktop());
        m_pinchCancelled = false;
        if (m_pinch) call("beginPinch");
        return m_pinch;
    }
    bool pinchGestureUpdate(KWin::PointerPinchGestureUpdateEvent *event) override
    {
        if (!m_pinch) return false;
        if (!enabled() || m_pinchCancelled) return true;
        call("updatePinch", event->scale);
        return true;
    }
    bool pinchGestureEnd(KWin::PointerPinchGestureEndEvent *) override
    {
        if (!m_pinch) return false;
        m_pinch = false;
        call("finishPinch", m_pinchCancelled);
        return true;
    }
    bool pinchGestureCancelled(KWin::PointerPinchGestureCancelEvent *) override
    {
        if (!m_pinch) return false;
        m_pinch = false;
        call("cancelGesture");
        return true;
    }
    bool pointerButton(KWin::PointerButtonEvent *event) override
    {
        if (event->button != Qt::LeftButton) return false;
        if (m_buttonRelease && event->state == KWin::PointerButtonState::Released) {
            m_buttonRelease = false;
            if (m_buttonWindow && m_buttonRect.translated(m_buttonWindow->pos()).contains(event->position))
                windowAction(m_buttonWindow, "toggle-float");
            m_buttonWindow.clear();
            return true;
        }
        if (m_frame && event->state == KWin::PointerButtonState::Released) {
            m_frame = false;
            call("endNativeFrame", true);
            return true;
        }
        if (!enabled() || event->state != KWin::PointerButtonState::Pressed) return false;
        if (event->modifiers.testFlag(Qt::MetaModifier) && event->modifiers.testFlag(Qt::ShiftModifier)) {
            m_frame = true;
            call("beginNativeFrame");
            return true;
        }
        auto *window = KWin::input()->findToplevel(event->position);
        auto *decoration = window && managed(window) ? window->decoration() : nullptr;
        bool title = false;
        if (decoration && event->modifiers == Qt::NoModifier) {
            const QPointF local = event->position - window->pos();
            title = decoration->titleBar().contains(local);
            for (auto *button : decoration->findChildren<KDecoration3::DecorationButton *>()) {
                if (!button->isVisible() || !button->geometry().contains(local)) continue;
                title = false;
                if (button->isEnabled() && button->type() == KDecoration3::DecorationButtonType::Maximize) {
                    m_buttonWindow = window;
                    m_buttonRect = button->geometry();
                    m_buttonRelease = true;
                    m_click.invalidate();
                    return true;
                }
            }
        }
        if ((title || desktop()) && event->modifiers == Qt::NoModifier) {
            const auto hints = QGuiApplication::styleHints();
            const QString target = title ? window->internalId().toString() : QStringLiteral("desktop");
            if (m_click.isValid() && m_click.elapsed() <= hints->mouseDoubleClickInterval()
                && target == m_clickTarget
                && (event->position - m_clickPosition).manhattanLength() < hints->startDragDistance()) {
                if (title) {
                    windowAction(window, "focus");
                } else call("desktopDoubleClick");
                m_buttonRelease = true;
                m_click.invalidate();
                return true;
            } else { m_click.start(); m_clickPosition = event->position; m_clickTarget = target; }
        } else m_click.invalidate();
        return false;
    }
    bool pointerMotion(KWin::PointerMotionEvent *) override
    {
        if (!m_frame) return false;
        call("updateNativeFrame");
        return true;
    }
    bool keyboardKey(KWin::KeyboardKeyEvent *event) override
    {
        if (event->key != Qt::Key_Escape || (!m_frame && !m_pinch)) return false;
        if (event->state == KWin::KeyboardKeyState::Pressed) {
            if (m_frame) call("endNativeFrame", false);
            if (m_pinch) { m_pinchCancelled = true; call("cancelGesture"); }
            // Keep consuming the paired release and the rest of the gesture.
        }
        return true;
    }
private:
    QPointer<QObject> m_controller;
    bool m_available = false;
    bool m_pinch = false;
    bool m_pinchCancelled = false;
    bool m_frame = false;
    bool m_buttonRelease = false;
    QPointer<KWin::Window> m_buttonWindow;
    QRectF m_buttonRect;
    QElapsedTimer m_click;
    QPointF m_clickPosition;
    QString m_clickTarget;
};

class InputPlugin : public QQmlExtensionPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)
public:
    void registerTypes(const char *uri) override { qmlRegisterType<InputBridge>(uri, 1, 0, "InputBridge"); }
};
#include "InputBridge.moc"
