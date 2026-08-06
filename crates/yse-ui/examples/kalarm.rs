//! A KAlarm-inspired personal alarm desk with persistent alarm rows.

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::{Application, Settings, StringTableModel, Window, clone};

fn refresh(table: &StringTableModel, alarms: &[Vec<String>]) {
    table.replace_all(alarms.iter().cloned());
}

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("KAlarm — Yse edition");
    window.set_size(760, 490);
    let settings = Settings::new("Yse", "KAlarmExample");
    let message = Var::new(String::new());
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
    let (message_edit, when_edit, recurrence_edit, add, remove, toggle, status_label) =
        window.ui().column(|ui| {
            ui.label("KAlarm");
            ui.label("Schedule a focused message alarm. State is saved when you change the list.");
            let (message_edit, when_edit) = ui.row(|ui| {
                (
                    ui.line_edit("Alarm message"),
                    ui.date_time_edit("2026-08-07T09:00:00"),
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
            ui.table_view(&table);
            let status_label = ui.label("");
            (
                message_edit,
                when_edit,
                recurrence_edit,
                add,
                remove,
                toggle,
                status_label,
            )
        });
    message_edit.bind_text_two_way(&message);
    recurrence_edit.bind_text_two_way(&recurrence);
    status_label.bind_text(&status.signal());
    add.on_click(clone!(message, when_edit, recurrence, alarms, table, status, settings => move |_| {
        if message.value().trim().is_empty() { status.set(String::from("Enter an alarm message first.")); return; }
        alarms.borrow_mut().push(vec![message.value().to_string(), when_edit.value(), recurrence.value().to_string(), String::from("Active")]);
        refresh(&table, &alarms.borrow());
        settings.set("alarm_count", &alarms.borrow().len().to_string());
        settings.sync();
        message.set(String::new());
        status.set(String::from("Alarm saved"));
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
