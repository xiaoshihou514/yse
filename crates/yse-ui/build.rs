fn main() {
    cxx_qt_build::CxxQtBuilder::new()
        .file("src/lib.rs")
        .cpp_file("src/widgets.cpp")
        .cpp_file("src/model.cpp")
        .qt_module("Gui")
        .qt_module("Widgets")
        .build();
}
