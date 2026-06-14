use cxx::UniquePtr;
use cxx_qt_lib::QString;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "C++" {
        include!("src/qt_app.h");

        type QApplication;
        type FanzyBackendHolder;
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        fn fanzy_new_qapplication() -> UniquePtr<QApplication>;
        fn fanzy_new_backend() -> UniquePtr<FanzyBackendHolder>;
        fn fanzy_qapplication_set_application_name(app: Pin<&mut QApplication>, name: &QString);
        fn fanzy_qapplication_set_application_version(
            app: Pin<&mut QApplication>,
            version: &QString,
        );
        fn fanzy_qapplication_set_desktop_file_name(name: &QString);
        fn fanzy_qapplication_exec(app: Pin<&mut QApplication>) -> i32;
        fn fanzy_qml_engine_set_backend(
            engine: Pin<&mut QQmlApplicationEngine>,
            backend: Pin<&mut FanzyBackendHolder>,
        );
        fn fanzy_qml_engine_root_count(engine: Pin<&mut QQmlApplicationEngine>) -> i32;
    }
}

pub(crate) struct QApplication {
    inner: UniquePtr<ffi::QApplication>,
}

impl QApplication {
    pub(crate) fn new() -> Self {
        Self {
            inner: ffi::fanzy_new_qapplication(),
        }
    }

    pub(crate) fn set_application_name(&mut self, name: &QString) {
        ffi::fanzy_qapplication_set_application_name(self.inner.pin_mut(), name);
    }

    pub(crate) fn set_application_version(&mut self, version: &QString) {
        ffi::fanzy_qapplication_set_application_version(self.inner.pin_mut(), version);
    }

    pub(crate) fn set_desktop_file_name(name: &QString) {
        ffi::fanzy_qapplication_set_desktop_file_name(name);
    }

    pub(crate) fn exec(&mut self) -> i32 {
        ffi::fanzy_qapplication_exec(self.inner.pin_mut())
    }
}

pub(crate) struct Backend {
    inner: UniquePtr<ffi::FanzyBackendHolder>,
}

impl Backend {
    pub(crate) fn new() -> Self {
        Self {
            inner: ffi::fanzy_new_backend(),
        }
    }
}

pub(crate) fn set_engine_backend(
    engine: std::pin::Pin<&mut ffi::QQmlApplicationEngine>,
    backend: &mut Backend,
) {
    ffi::fanzy_qml_engine_set_backend(engine, backend.inner.pin_mut());
}

pub(crate) fn engine_root_count(engine: std::pin::Pin<&mut ffi::QQmlApplicationEngine>) -> i32 {
    ffi::fanzy_qml_engine_root_count(engine)
}
