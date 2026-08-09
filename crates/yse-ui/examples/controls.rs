//! Phase 4 demonstration: value widgets (combo box, spin box, slider,
//! progress bar) in a declarative tree, with one-way signal bindings
//! composed into two-way behavior through value-change handlers.

use yse_model::Var;
use yse_ui::{
    Application, Window, button, clone, column, combo_box, label, progress_bar, slider, spin_box,
};

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("Yse controls");
    window.set_size(420, 260);

    // State and derived signals.
    let profile = Var::new(0i32);
    let quantity = Var::new(3i32);
    let level = Var::new(40i32);
    let status = Var::new(String::from("idle"));

    // Declarative tree: `Var`s are controlled by the widgets (two-way
    // bindings); the progress bar derives from the same state as the slider.
    let (combo, spin, slider, bar, reset, status_label) = window.mount(column((
        combo_box(["Starter", "Balanced", "Power"])
            .value(profile.clone())
            .on_value_change(clone!(status => move |index| {
                status.set(format!("profile: {}", ["Starter", "Balanced", "Power"][*index as usize]));
            })),
        spin_box(quantity.clone())
            .range(1, 10)
            .on_value_change(clone!(status => move |value| {
                status.set(format!("quantity: {value}"));
            })),
        slider(level.clone()).range(0, 100),
        progress_bar(level.signal()).range(0, 100),
        button("Reset").on_click(clone!(profile, quantity, level, status => move |_| {
            profile.set(0);
            quantity.set(3);
            level.set(40);
            status.set(String::from("reset"));
        })),
        label(status.signal()),
    )));

    // Headless smoke run: drive every control, then quit.
    let smoke_ok = Var::new(true);
    let smoke = std::env::var("YSE_SMOKE").is_ok();
    if smoke {
        app.quit_after(1500);
        app.after(
            150,
            clone!(combo, spin, slider, level, bar, reset, smoke_ok => move || {
                combo.set_current_index(2);
                app.after(120, clone!(spin, slider, level, bar, reset, smoke_ok => move || {
                    spin.set_value(7);
                    app.after(120, clone!(slider, level, bar, reset, smoke_ok => move || {
                        slider.set_value(80);
                        // The slider writes `level`, which drives the progress bar.
                        if *level.value() != 80 || bar.value() != 80 {
                            smoke_ok.set(false);
                        }
                        app.after(120, clone!(reset => move || {
                            reset.click();
                            app.after(150, || {});
                        }));
                    }));
                }));
            }),
        );
    }

    window.show();
    let code = app.exec();
    println!(
        "[controls] profile={} quantity={} level={} progress={} status={:?}",
        combo.current_text(),
        spin.value(),
        slider.value(),
        bar.value(),
        status_label.text(),
    );
    if smoke {
        assert!(*smoke_ok.value(), "controls smoke checks failed");
        assert_eq!(combo.current_index(), 0, "reset restores the profile");
        assert_eq!(spin.value(), 3, "reset restores the quantity");
        assert_eq!(slider.value(), 40, "reset restores the level");
    }
    std::process::exit(code);
}
