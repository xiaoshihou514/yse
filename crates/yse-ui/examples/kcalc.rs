//! A compact KCalc-inspired scientific calculator: reactive display, history,
//! keyboard-friendly actions, and a deliberately small expression evaluator.

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::{Application, StringListModel, Window, clone};

fn evaluate(expression: &str) -> Result<f64, String> {
    let mut values = Vec::<f64>::new();
    let mut operators = Vec::<char>::new();
    let apply = |values: &mut Vec<f64>, op: char| -> Result<(), String> {
        let right = values.pop().ok_or_else(|| String::from("missing number"))?;
        let left = values.pop().ok_or_else(|| String::from("missing number"))?;
        let value = match op {
            '+' => left + right,
            '-' => left - right,
            '*' => left * right,
            '/' if right != 0.0 => left / right,
            '/' => return Err(String::from("division by zero")),
            _ => return Err(String::from("unsupported operator")),
        };
        values.push(value);
        Ok(())
    };
    let mut number = String::new();
    for character in expression.chars().chain(std::iter::once('+')) {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
            continue;
        }
        if character.is_whitespace() {
            continue;
        }
        if number.is_empty() {
            return Err(String::from("enter a number before an operator"));
        }
        values.push(number.parse().map_err(|_| String::from("invalid number"))?);
        number.clear();
        while let Some(operator) = operators.pop() {
            apply(&mut values, operator)?;
        }
        if character != '+' || !expression.ends_with('+') {
            operators.push(character);
        }
    }
    values
        .pop()
        .ok_or_else(|| String::from("enter an expression"))
}

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("KCalc — Yse edition");
    window.set_size(420, 530);

    let display = Var::new(String::from("0"));
    let status = Var::new(String::from("Ready · basic arithmetic"));
    let history = StringListModel::new();
    let entries = Rc::new(RefCell::new(Vec::<String>::new()));

    let (display_edit, clear, backspace, equals, keys, status_label) = window.ui().column(|ui| {
        ui.label("KCalc");
        ui.label("A small, reactive calculator with a recallable result stack.");
        let display_edit = ui.line_edit("");
        let (clear, backspace, equals) =
            ui.row(|ui| (ui.button("Clear"), ui.button("⌫"), ui.button("=")));
        let first = ui.row(|ui| {
            (
                ui.button("7"),
                ui.button("8"),
                ui.button("9"),
                ui.button("÷"),
            )
        });
        let second = ui.row(|ui| {
            (
                ui.button("4"),
                ui.button("5"),
                ui.button("6"),
                ui.button("×"),
            )
        });
        let third = ui.row(|ui| {
            (
                ui.button("1"),
                ui.button("2"),
                ui.button("3"),
                ui.button("−"),
            )
        });
        let fourth = ui.row(|ui| (ui.button("0"), ui.button("."), ui.button("+")));
        ui.label("Result stack");
        ui.list_view(&history);
        let status_label = ui.label("");
        (
            display_edit,
            clear,
            backspace,
            equals,
            (first, second, third, fourth),
            status_label,
        )
    });
    display_edit.bind_text_two_way(&display);
    status_label.bind_text(&status.signal());
    clear.on_click(clone!(display, status => move |_| { display.set(String::from("0")); status.set(String::from("Cleared")); }));
    backspace.on_click(clone!(display => move |_| {
        let mut value = display.value().to_string();
        value.pop();
        display.set(if value.is_empty() { String::from("0") } else { value });
    }));
    equals.on_click(
        clone!(display, status, history, entries => move |_| match evaluate(&display.value()) {
            Ok(value) => {
                let result = format!("{} = {value}", display.value());
                entries.borrow_mut().insert(0, result.clone());
                history.replace_all(entries.borrow().iter().cloned());
                display.set(value.to_string());
                status.set(String::from("Calculated · select history to recall"));
            }
            Err(error) => status.set(format!("Error: {error}")),
        }),
    );
    for (key, text) in [
        (keys.0.0, "7"),
        (keys.0.1, "8"),
        (keys.0.2, "9"),
        (keys.0.3, "/"),
        (keys.1.0, "4"),
        (keys.1.1, "5"),
        (keys.1.2, "6"),
        (keys.1.3, "*"),
        (keys.2.0, "1"),
        (keys.2.1, "2"),
        (keys.2.2, "3"),
        (keys.2.3, "-"),
        (keys.3.0, "0"),
        (keys.3.1, "."),
        (keys.3.2, "+"),
    ] {
        key.on_click(clone!(display => move |_| {
            let current = display.value().to_string();
            display.set(if current == "0" { String::from(text) } else { format!("{current}{text}") });
        }));
    }

    if std::env::var("YSE_SMOKE").is_ok() {
        display.set(String::from("12.5 * 4"));
        equals.click();
        app.quit_after(100);
    }
    window.show();
    std::process::exit(app.exec());
}
