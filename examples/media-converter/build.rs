fn main() {
    let flags = std::process::Command::new("pkg-config")
        .args(["--cflags", "libavfilter"])
        .output()
        .expect("failed to run pkg-config for FFmpeg");
    assert!(
        flags.status.success(),
        "FFmpeg development libraries not found via pkg-config"
    );
    let flags = String::from_utf8(flags.stdout).expect("pkg-config returned non-UTF-8 flags");
    let mut build = cxx_qt_build::CxxQtBuilder::new()
        .file("src/main.rs")
        .cpp_file("cpp/media.cpp");
    unsafe {
        build = build.cc_builder(|cc| {
            cc.flag_if_supported("/utf-8");
            for flag in flags.split_whitespace() {
                cc.flag(flag);
            }
        });
    }
    build.build();
    for library in ["avformat", "avcodec", "avfilter", "avutil"] {
        println!("cargo:rustc-link-lib={library}");
    }
}
