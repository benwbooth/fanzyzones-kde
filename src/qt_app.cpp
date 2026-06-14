#include "qt_app.h"

FanzyBackendHolder::FanzyBackendHolder()
  : backend_(std::make_unique<FanzyBackend>())
{
}

FanzyBackend& FanzyBackendHolder::backend()
{
  return *backend_;
}

std::unique_ptr<QApplication> fanzy_new_qapplication()
{
  static int argc = 1;
  static char appName[] = "fanzyzones-kde";
  static char* argv[] = { appName, nullptr };

  return std::make_unique<QApplication>(argc, argv);
}

std::unique_ptr<FanzyBackendHolder> fanzy_new_backend()
{
  return std::make_unique<FanzyBackendHolder>();
}

void fanzy_qapplication_set_application_name(QApplication& app, const QString& name)
{
  app.setApplicationName(name);
}

void fanzy_qapplication_set_application_version(QApplication& app, const QString& version)
{
  app.setApplicationVersion(version);
}

void fanzy_qapplication_set_desktop_file_name(const QString& name)
{
  QGuiApplication::setDesktopFileName(name);
}

int fanzy_qapplication_exec(QApplication& app)
{
  return app.exec();
}

void fanzy_qml_engine_set_backend(QQmlApplicationEngine& engine, FanzyBackendHolder& backend)
{
  QVariantMap properties;
  properties.insert(QStringLiteral("backend"), QVariant::fromValue(static_cast<QObject*>(&backend.backend())));
  engine.setInitialProperties(properties);
}

int fanzy_qml_engine_root_count(QQmlApplicationEngine& engine)
{
  return engine.rootObjects().size();
}
