//! A KAlarm-inspired personal alarm desk with explicit calendar and time controls.

#[path = "support/example_style.rs"]
mod example_style;

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::{Application, Settings, StringTableModel, Window, clone};

fn refresh(table: &StringTableModel, alarms: &[Vec<String>]) {
    table.replace_all(alarms.iter().cloned());
}

fn main() {
    let app = Application::init();
    example_style::apply(&app);
    let window = Window::new();
    window.set_title("KAlarm — Yse edition");
    window.set_size(820, 540);
    let menubar = window.menu_bar();
    let new_alarm = menubar.menu_with("Alarm", |menu| {
        menu.action("New alarm")
            .icon("appointment-new")
            .shortcut("Ctrl+N")
    });
    let toolbar = window.toolbar();
    toolbar.add(&new_alarm);
    let settings = Settings::new("Yse", "KAlarmExample");
    let message = Var::new(String::new());
    let date = Var::new(String::from("2026-08-07"));
    let time = Var::new(String::from("09:00"));
    let recurrence = Var::new(String::from("Once"));
    let status = Var::new(String::from("Ready"));
    let table = StringTableModel::new(
        4,
        [
            String::from("Alarm"),
            String::from("When"),
            String::from("Repeat"),
            String::from("State"),
        ],
    );
    let alarms = Rc::new(RefCell::new(vec![vec![
        String::from("Stand up"),
        String::from("Weekdays 10:30"),
        String::from("Weekdays"),
        String::from("Active"),
    ]]));
    refresh(&table, &alarms.borrow());
    let (
        _title,
        description,
        message_edit,
        date_edit,
        time_edit,
        recurrence_edit,
        add,
        remove,
        toggle,
        _table_view,
        status_label,
    ) = window.ui().column(|ui| {
        let title = ui.label("New alarm");
        let description = ui.label("Schedule a message for a specific date and time.");
        let message_edit = ui.line_edit("Alarm message");
        let (date_edit, time_edit) = ui.row(|ui| {
            (
                ui.date_edit(date.value().as_str()),
                ui.time_edit(time.value().as_str()),
            )
        });
        let (recurrence_edit, add, remove, toggle) = ui.row(|ui| {
            (
                ui.line_edit(""),
                ui.button("Add alarm"),
                ui.button("Remove selected"),
                ui.button("Enable / disable"),
            )
        });
        let table_view = ui.table_view(&table);
        let status_label = ui.label("");
        (
            title,
            description,
            message_edit,
            date_edit,
            time_edit,
            recurrence_edit,
            add,
            remove,
            toggle,
            table_view,
            status_label,
        )
    });
    description.set_style_class("muted");
    date_edit.set_style_class("schedule");
    time_edit.set_style_class("schedule");
    add.set_style_class("accent");
    remove.set_style_class("quiet");
    toggle.set_style_class("quiet");
    status_label.set_style_class("muted");
    message_edit.bind_text_two_way(&message);
    date_edit.bind_value_two_way(&date);
    time_edit.bind_value_two_way(&time);
    recurrence_edit.bind_text_two_way(&recurrence);
    status_label.bind_text(&status.signal());
    add.on_click(clone!(message, date, time, recurrence, alarms, table, status, settings => move |_| {
        if message.value().trim().is_empty() { status.set(String::from("Enter an alarm message first.")); return; }
        let when = format!("{} {}", date.value(), time.value());
        alarms.borrow_mut().push(vec![message.value().to_string(), when, recurrence.value().to_string(), String::from("Active")]);
        refresh(&table, &alarms.borrow());
        settings.set("alarm_count", &alarms.borrow().len().to_string());
        settings.sync();
        message.set(String::new());
        status.set(String::from("Alarm saved"));
    }));
    new_alarm.on_trigger(clone!(message, status => move |_| {
        message.set(String::new());
        status.set(String::from("New alarm ready — choose a date and time."));
    }));
    remove.on_click(clone!(alarms, table, status => move |_| {
        if let Some(index) = table.row_count().checked_sub(1) { alarms.borrow_mut().remove(index); refresh(&table, &alarms.borrow()); status.set(String::from("Removed last alarm")); }
    }));
    toggle.on_click(clone!(alarms, table, status => move |_| {
        if let Some(alarm) = alarms.borrow_mut().last_mut() { alarm[3] = if alarm[3] == "Active" { String::from("Paused") } else { String::from("Active") }; refresh(&table, &alarms.borrow()); status.set(String::from("Updated alarm state")); }
    }));
    if std::env::var("YSE_SMOKE").is_ok() {
        message.set(String::from("Smoke-test alarm"));
        add.click();
        app.quit_after(80);
    }
    window.show();
    std::process::exit(app.exec());
}
