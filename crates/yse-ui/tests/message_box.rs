use std::cell::RefCell;
use std::rc::Rc;
use yse_ui::{Application, MessageBox, MessageBoxButtons, MessageBoxResult, Window};

#[test]
fn message_box_reports_result() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let box_ = MessageBox::new(&window, "Title", "Hello", MessageBoxButtons::OkCancel);
    let seen: Rc<RefCell<Vec<MessageBoxResult>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = box_
        .result()
        .observe(move |result| seen_rc.borrow_mut().push(*result));

    box_.show();
    box_.accept();
    assert_eq!(*seen.borrow(), vec![MessageBoxResult::Ok]);
}
