fn main() {
    let mut builder = cxx_qt_build::CxxQtBuilder::new()
        .file("src/bridge.rs")
        .file("src/qt_object.rs")
        .cpp_file("src/widgets.cpp")
        .cpp_file("src/model.cpp")
        .qt_module("Gui")
        .qt_module("Widgets");
    // MSVC assumes the system code page for narrow literals; force UTF-8 so
    // non-ASCII text in shim sources compiles identically everywhere.
    unsafe {
        builder = builder.cc_builder(|cc| {
            cc.flag_if_supported("/utf-8");
        });
    }
    builder.build();
}
