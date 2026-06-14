#pragma once

#include <QtCore/QString>
#include <QtCore/QVariant>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>
#include <QtWidgets/QApplication>
#include <fanzyzones-kde/src/backend.cxxqt.h>
#include <memory>

class FanzyBackendHolder
{
public:
  FanzyBackendHolder();
  FanzyBackend& backend();

private:
  std::unique_ptr<FanzyBackend> backend_;
};

std::unique_ptr<QApplication> fanzy_new_qapplication();
std::unique_ptr<FanzyBackendHolder> fanzy_new_backend();
void fanzy_qapplication_set_application_name(QApplication& app, const QString& name);
void fanzy_qapplication_set_application_version(QApplication& app, const QString& version);
void fanzy_qapplication_set_desktop_file_name(const QString& name);
int fanzy_qapplication_exec(QApplication& app);
void fanzy_qml_engine_set_backend(QQmlApplicationEngine& engine, FanzyBackendHolder& backend);
int fanzy_qml_engine_root_count(QQmlApplicationEngine& engine);
