use std::cell::RefCell;
use std::rc::Rc;
use yse_ui::{Application, FileDialog, Window};

#[test]
fn file_and_directory_dialogs_report_no_selection_on_close() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    for dialog in [
        FileDialog::open(&window, "Open"),
        FileDialog::directory(&window, "Choose folder"),
    ] {
        let seen: Rc<RefCell<Vec<Option<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let seen_rc = seen.clone();
        let _sub = dialog
            .result()
            .observe(move |path| seen_rc.borrow_mut().push(path.clone()));

        dialog.show();
        dialog.close();
        assert_eq!(*seen.borrow(), vec![None]);
    }
}
