//! A native Qt/FFmpeg media converter. No command-line programs are invoked:
//! probing, decoding, filtering, encoding, and muxing use the FFmpeg C API.

use std::cell::{Cell, RefCell};
use std::f32::consts::TAU;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use yse_model::{Subscription, Task, Var, spawn_task};
use yse_ui::{
    Application, FileDialog, MediaInfo, MediaPreset, QtGuiScheduler, Window, button, clone, column,
    convert_media, label, line_edit, probe_media, row, spacer,
};

type TaskSlot<T> = Rc<RefCell<Option<Task<Result<T, String>>>>>;

fn start_probe(
    path: String,
    details: Var<String>,
    tasks: TaskSlot<MediaInfo>,
    subscriptions: Rc<RefCell<Vec<Subscription>>>,
) {
    details.set(String::from("Inspecting…"));
    let task = spawn_task(Arc::new(QtGuiScheduler), move |_| probe_media(&path));
    subscriptions.borrow_mut().push(task.results().observe(
        clone!(details => move |result| match result {
            Ok(info) => details.set(info.summary.clone()),
            Err(error) => details.set(format!("Could not inspect input: {error}")),
        }),
    ));
    *tasks.borrow_mut() = Some(task);
}

fn write_test_wav(path: &str) -> std::io::Result<()> {
    let sample_rate = 48_000u32;
    let samples = sample_rate / 4;
    let data_size = samples * 2;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_size).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&(sample_rate * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;
    for index in 0..samples {
        let phase = index as f32 * 440.0 * TAU / sample_rate as f32;
        let sample = (phase.sin() * i16::MAX as f32 * 0.2) as i16;
        file.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("Yse Media Converter");
    window.set_size(680, 300);

    let input = Var::new(String::new());
    let output = Var::new(String::new());
    let details = Var::new(String::from("Choose an audio or video file."));
    let status = Var::new(String::from("Ready"));
    let selected = Rc::new(Cell::new(MediaPreset::Mp4));
    let probe_task = Rc::new(RefCell::new(None));
    let convert_task = Rc::new(RefCell::new(None));
    let subscriptions = Rc::new(RefCell::new(Vec::<Subscription>::new()));

    let browse_dialog = FileDialog::open(&window, "Choose media file");
    subscriptions
        .borrow_mut()
        .push(browse_dialog.result().observe({
            let input = input.clone();
            let output = output.clone();
            let details = details.clone();
            let probe_task = probe_task.clone();
            let subscriptions = subscriptions.clone();
            move |selection| {
                if let Some(path) = selection {
                    input.set(path.clone());
                    output.set(format!("{path}.converted.mp4"));
                    start_probe(
                        path.clone(),
                        details.clone(),
                        probe_task.clone(),
                        subscriptions.clone(),
                    );
                }
            }
        }));

    let (_, (_, browse), _, _, _, (mp4, webm, mp3, flac), (_, convert), _) =
        window.mount(column((
            label("Input"),
            row((line_edit("").controlled(input.clone()), button("Browse…"))),
            label(details.signal()),
            label("Output"),
            line_edit("").controlled(output.clone()),
            row((
                button("MP4 · H.264/AAC"),
                button("WebM · VP9/Opus"),
                button("MP3"),
                button("FLAC"),
            )),
            row((spacer(), button("Convert"))),
            label(status.signal()),
        )));

    browse.on_click(clone!(browse_dialog => move |_| browse_dialog.show()));
    for (button, preset, extension, name) in [
        (mp4, MediaPreset::Mp4, "mp4", "MP4 · H.264/AAC"),
        (webm, MediaPreset::WebM, "webm", "WebM · VP9/Opus"),
        (mp3, MediaPreset::Mp3, "mp3", "MP3"),
        (flac, MediaPreset::Flac, "flac", "FLAC"),
    ] {
        button.on_click({
            let selected = selected.clone();
            let input = input.clone();
            let output = output.clone();
            let status = status.clone();
            move |_| {
                selected.set(preset);
                if !input.value().is_empty() {
                    output.set(format!("{}.converted.{extension}", input.value()));
                }
                status.set(format!("Selected {name}"));
            }
        });
    }
    convert.on_click({
        let input = input.clone();
        let output = output.clone();
        let selected = selected.clone();
        let status = status.clone();
        let convert_task = convert_task.clone();
        let subscriptions = subscriptions.clone();
        move |_| {
            let source = input.value().to_string();
            let destination = output.value().to_string();
            if source.is_empty() || destination.is_empty() {
                status.set(String::from("Choose input and output files first."));
                return;
            }
            status.set(String::from("Converting in the background…"));
            let preset = selected.get();
            let task = spawn_task(Arc::new(QtGuiScheduler), move |_| {
                convert_media(&source, &destination, preset)
            });
            subscriptions.borrow_mut().push(task.results().observe(
                clone!(status => move |result| match result {
                    Ok(()) => status.set(String::from("Conversion complete.")),
                    Err(error) => status.set(format!("Conversion failed: {error}")),
                }),
            ));
            *convert_task.borrow_mut() = Some(task);
        }
    });

    if std::env::var("YSE_SMOKE").is_ok() {
        let source = "/tmp/yse-media-smoke.wav";
        let destination = "/tmp/yse-media-smoke.flac";
        if let Err(error) = std::fs::remove_file(destination)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            panic!("remove old smoke-test output: {error}");
        }
        write_test_wav(source).expect("create smoke-test input");
        input.set(String::from(source));
        output.set(String::from(destination));
        selected.set(MediaPreset::Flac);
        convert.click();
        app.quit_after(1800);
    }

    window.show();
    let code = app.exec();
    if std::env::var("YSE_SMOKE").is_ok() {
        let converted = probe_media("/tmp/yse-media-smoke.flac")
            .expect("inspect the converted file through libavformat");
        assert!(converted.has_audio);
        assert!(!converted.has_video);
        println!("[media_converter] output={}", converted.summary);
    }
    println!("[media_converter] {}", status.value());
    std::process::exit(code);
}
