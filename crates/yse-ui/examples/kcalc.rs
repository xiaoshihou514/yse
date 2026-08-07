//! A compact KCalc-inspired calculator: one display and a precise keypad.

#[path = "support/example_style.rs"]
mod example_style;

use yse_model::Var;
use yse_ui::{Application, Window, clone};

fn precedence(operator: char) -> u8 {
    match operator {
        '+' | '-' => 1,
        '*' | '/' => 2,
        _ => 0,
    }
}

fn apply(values: &mut Vec<f64>, operator: char) -> Result<(), String> {
    let right = values.pop().ok_or_else(|| String::from("missing number"))?;
    let left = values.pop().ok_or_else(|| String::from("missing number"))?;
    values.push(match operator {
        '+' => left + right,
        '-' => left - right,
        '*' => left * right,
        '/' if right != 0.0 => left / right,
        '/' => return Err(String::from("division by zero")),
        _ => return Err(String::from("unsupported operator")),
    });
    Ok(())
}

fn evaluate(expression: &str) -> Result<f64, String> {
    let mut values = Vec::new();
    let mut operators = Vec::new();
    let mut number = String::new();
    for character in expression
        .chars()
        .filter(|character| !character.is_whitespace())
    {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
            continue;
        }
        if !matches!(character, '+' | '-' | '*' | '/') || number.is_empty() {
            return Err(String::from("incomplete expression"));
        }
        values.push(number.parse().map_err(|_| String::from("invalid number"))?);
        number.clear();
        while operators
            .last()
            .is_some_and(|op| precedence(*op) >= precedence(character))
        {
            apply(&mut values, operators.pop().unwrap())?;
        }
        operators.push(character);
    }
    if number.is_empty() {
        return Err(String::from("incomplete expression"));
    }
    values.push(number.parse().map_err(|_| String::from("invalid number"))?);
    while let Some(operator) = operators.pop() {
        apply(&mut values, operator)?;
    }
    values
        .pop()
        .ok_or_else(|| String::from("enter an expression"))
}

fn append(display: &Var<String>, token: &str) {
    let value = display.value();
    display.set(if value.as_ref() == "0" && token != "." {
        String::from(token)
    } else {
        format!("{value}{token}")
    });
}

fn main() {
    let app = Application::init();
    example_style::apply(&app);
    let window = Window::new();
    window.set_title("KCalc — Yse edition");
    window.set_size(470, 510);

    let menu_bar = window.menu_bar();
    let clear_action = menu_bar.menu_with("Calculator", |menu| {
        menu.action("Clear").icon("edit-clear").shortcut("Esc")
    });
    let history_action = menu_bar.menu_with("History", |menu| menu.action("Show history"));

    let display = Var::new(String::from("0"));
    let status = Var::new(String::new());
    let (display_edit, first, second, third, fourth, fifth, status_label) =
        window.ui().column(|ui| {
            let display_edit = ui.line_edit("0");
            let first = ui.row(|ui| {
                (
                    ui.button("C"),
                    ui.button("⌫"),
                    ui.button("÷"),
                    ui.button("×"),
                )
            });
            let second = ui.row(|ui| {
                (
                    ui.button("7"),
                    ui.button("8"),
                    ui.button("9"),
                    ui.button("−"),
                )
            });
            let third = ui.row(|ui| {
                (
                    ui.button("4"),
                    ui.button("5"),
                    ui.button("6"),
                    ui.button("+"),
                )
            });
            let fourth = ui.row(|ui| {
                (
                    ui.button("1"),
                    ui.button("2"),
                    ui.button("3"),
                    ui.button("="),
                )
            });
            let fifth = ui.row(|ui| (ui.button("0"), ui.button(".")));
            let status_label = ui.label("");
            (
                display_edit,
                first,
                second,
                third,
                fourth,
                fifth,
                status_label,
            )
        });
    let (clear, backspace, divide, multiply) = first;
    let (seven, eight, nine, minus) = second;
    let (four, five, six, plus) = third;
    let (one, two, three, equals) = fourth;
    let (zero, decimal) = fifth;
    display_edit.set_style_class("display");
    for key in [
        &clear, &backspace, &seven, &eight, &nine, &four, &five, &six, &one, &two, &three, &zero,
        &decimal,
    ] {
        key.set_style_class("key");
    }
    for key in [&divide, &multiply, &minus, &plus] {
        key.set_style_class("operator");
    }
    equals.set_style_class("accent");
    status_label.set_style_class("muted");

    display_edit.bind_text_two_way(&display);
    status_label.bind_text(&status.signal());
    clear.on_click(clone!(display, status => move |_| { display.set(String::from("0")); status.set(String::new()); }));
    clear_action.on_trigger(clone!(display, status => move |_| { display.set(String::from("0")); status.set(String::new()); }));
    backspace.on_click(clone!(display => move |_| { let mut value = display.value().to_string(); value.pop(); display.set(if value.is_empty() { String::from("0") } else { value }); }));
    for (key, token) in [
        (&divide, "/"),
        (&multiply, "*"),
        (&seven, "7"),
        (&eight, "8"),
        (&nine, "9"),
        (&minus, "-"),
        (&four, "4"),
        (&five, "5"),
        (&six, "6"),
        (&plus, "+"),
        (&one, "1"),
        (&two, "2"),
        (&three, "3"),
        (&zero, "0"),
        (&decimal, "."),
    ] {
        key.on_click(clone!(display => move |_| append(&display, token)));
    }
    equals.on_click(
        clone!(display, status => move |_| match evaluate(&display.value()) {
            Ok(value) => { display.set(value.to_string()); status.set(String::new()); }
            Err(error) => status.set(error),
        }),
    );
    history_action.on_trigger(
        clone!(status => move |_| status.set(String::from("No calculations in this session yet."))),
    );

    if std::env::var("YSE_SMOKE").is_ok() {
        display.set(String::from("12.5 * 4 + 2"));
        equals.click();
        app.quit_after(100);
    }
    window.show();
    std::process::exit(app.exec());
}
