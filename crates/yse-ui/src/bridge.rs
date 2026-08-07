#[cxx::bridge(namespace = "yse_ui")]
#[allow(clippy::module_inception)]
mod bridge {
    /// A table row, passed to the C++ shim as a flat string list.
    struct Row {
        cells: Vec<String>,
    }

    /// One row of a tree model: `parent` is the flat row of the parent
    /// (-1 for a root/group row), `cells` the display text, `icon` a file
    /// path whose icon is resolved by the C++ side.
    struct TreeRow {
        parent: i32,
        cells: Vec<String>,
        icon: String,
    }

    unsafe extern "C++" {
        include!("yse-ui/src/widgets.h");

        type Widget;
        type Void;
        type Action;
        type Model;
        type TableModel;
        type TreeModel;
        type Dialog;
        type FileDialog;
        type Settings;

        fn app_init();
        fn app_system_color_scheme_dark() -> bool;
        fn app_apply_color_scheme(dark: bool);
        fn app_set_style_sheet(style_sheet: &str);
        fn app_exec() -> i32;
        fn app_quit_after(ms: i32);
        unsafe fn app_schedule_gui(task: *mut Void);
        unsafe fn app_invoke_after(ms: i32, task: *mut Void);

        unsafe fn widget_new_window() -> *mut Widget;
        unsafe fn widget_new_label(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_button(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_line_edit(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_datetime_edit(iso_datetime: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_date_edit(iso_date: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_time_edit(iso_time: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_disk_map(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_checkbox(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_combo(items: Vec<String>, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_spin_box(
            min: i32,
            max: i32,
            value: i32,
            parent: *mut Widget,
        ) -> *mut Widget;
        unsafe fn widget_new_slider(
            min: i32,
            max: i32,
            value: i32,
            parent: *mut Widget,
        ) -> *mut Widget;
        unsafe fn widget_new_progress_bar(
            min: i32,
            max: i32,
            value: i32,
            parent: *mut Widget,
        ) -> *mut Widget;
        unsafe fn widget_new_tab_widget(parent: *mut Widget) -> *mut Widget;
        unsafe fn tab_widget_add_page(tabs: *mut Widget, label: &str) -> *mut Widget;
        unsafe fn tab_widget_set_current(tabs: *mut Widget, index: i32);
        unsafe fn tab_widget_count(tabs: *mut Widget) -> i32;
        unsafe fn widget_new_line_chart(parent: *mut Widget) -> *mut Widget;
        unsafe fn line_chart_set_series(w: *mut Widget, points: Vec<f64>);
        unsafe fn line_chart_set_series_multi(w: *mut Widget, points: Vec<f64>, series: usize);
        unsafe fn widget_new_row(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_column(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_grid(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_menubar(window: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_toolbar(window: *mut Widget) -> *mut Widget;
        unsafe fn menu_new(title: &str, menubar: *mut Widget) -> *mut Widget;
        unsafe fn window_menu_new(title: &str, window: *mut Widget) -> *mut Widget;
        unsafe fn menu_add_submenu(menu: *mut Widget, title: &str) -> *mut Widget;
        unsafe fn menu_popup(menu: *mut Widget);
        unsafe fn menu_add_separator(menu: *mut Widget);
        unsafe fn toolbar_add_action(toolbar: *mut Widget, action: *mut Action);
        unsafe fn widget_drop(w: *mut Widget);
        unsafe fn widget_show(w: *mut Widget);
        unsafe fn widget_set_visible(w: *mut Widget, visible: bool);
        unsafe fn widget_is_visible(w: *mut Widget) -> bool;
        unsafe fn widget_width(w: *mut Widget) -> i32;
        unsafe fn widget_height(w: *mut Widget) -> i32;
        unsafe fn widget_set_enabled(w: *mut Widget, enabled: bool);
        unsafe fn widget_set_title(w: *mut Widget, title: &str);
        unsafe fn widget_set_style_class(w: *mut Widget, style_class: &str);
        unsafe fn widget_resize(w: *mut Widget, width: i32, height: i32);
        unsafe fn layout_add(layout: *mut Widget, child: *mut Widget);
        unsafe fn layout_add_spacer(w: *mut Widget);
        unsafe fn grid_add(grid: *mut Widget, child: *mut Widget, row: i32, column: i32);
        unsafe fn label_set_text(w: *mut Widget, text: &str);
        unsafe fn label_text(w: *mut Widget) -> String;
        unsafe fn line_edit_set_text(w: *mut Widget, text: &str);
        unsafe fn line_edit_text(w: *mut Widget) -> String;
        unsafe fn datetime_edit_set_value(w: *mut Widget, iso_datetime: &str);
        unsafe fn datetime_edit_value(w: *mut Widget) -> String;
        unsafe fn date_edit_set_value(w: *mut Widget, iso_date: &str);
        unsafe fn date_edit_value(w: *mut Widget) -> String;
        unsafe fn time_edit_set_value(w: *mut Widget, iso_time: &str);
        unsafe fn time_edit_value(w: *mut Widget) -> String;
        unsafe fn disk_map_set_segments(w: *mut Widget, labels: Vec<String>, shares: Vec<f64>);
        unsafe fn button_set_icon(w: *mut Widget, theme_name: &str);
        unsafe fn checkbox_set_checked(w: *mut Widget, checked: bool);
        unsafe fn checkbox_checked(w: *mut Widget) -> bool;
        unsafe fn combo_set_items(w: *mut Widget, items: Vec<String>);
        unsafe fn combo_current_text(w: *mut Widget) -> String;
        unsafe fn combo_set_current_text(w: *mut Widget, text: &str);
        unsafe fn widget_value(w: *mut Widget) -> i32;
        unsafe fn widget_set_value(w: *mut Widget, value: i32);
        unsafe fn widget_set_value_changed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_date_value_changed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_value_text(w: *mut Widget) -> String;
        unsafe fn widget_set_value_text(w: *mut Widget, text: &str);
        unsafe fn spin_box_set_range(w: *mut Widget, min: i32, max: i32);
        unsafe fn slider_set_range(w: *mut Widget, min: i32, max: i32);
        unsafe fn progress_set_range(w: *mut Widget, min: i32, max: i32);
        unsafe fn button_click(w: *mut Widget);
        unsafe fn button_set_text(w: *mut Widget, text: &str);
        unsafe fn button_text(w: *mut Widget) -> String;
        unsafe fn action_new(text: &str, parent: *mut Widget) -> *mut Action;
        unsafe fn action_drop(a: *mut Action);
        unsafe fn action_set_text(a: *mut Action, text: &str);
        unsafe fn action_set_icon(a: *mut Action, theme_name: &str);
        unsafe fn action_set_enabled(a: *mut Action, enabled: bool);
        unsafe fn action_set_shortcut(a: *mut Action, shortcut: &str);
        unsafe fn action_set_checkable(a: *mut Action, checkable: bool);
        unsafe fn action_set_checked(a: *mut Action, checked: bool);
        unsafe fn action_checked(a: *mut Action) -> bool;
        unsafe fn action_trigger(a: *mut Action);
        unsafe fn action_set_triggered_cb(a: *mut Action, data: *mut Void);
        unsafe fn action_set_destroyed_cb(a: *mut Action, data: *mut Void);
        unsafe fn model_new() -> *mut Model;
        unsafe fn model_drop(m: *mut Model);
        unsafe fn model_insert_rows(m: *mut Model, row: i32, items: Vec<String>);
        unsafe fn model_remove_rows(m: *mut Model, row: i32, count: i32);
        unsafe fn model_update_rows(m: *mut Model, row: i32, items: Vec<String>);
        unsafe fn model_reset(m: *mut Model, items: Vec<String>);
        unsafe fn model_row_count(m: *mut Model) -> i32;
        unsafe fn model_text(m: *mut Model, row: i32) -> String;
        unsafe fn widget_new_list_view(model: *mut Model, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_table_view(model: *mut TableModel, parent: *mut Widget)
        -> *mut Widget;
        unsafe fn widget_new_tree_view(model: *mut TreeModel, parent: *mut Widget) -> *mut Widget;
        unsafe fn table_model_new(columns: i32) -> *mut TableModel;
        unsafe fn table_model_drop(m: *mut TableModel);
        unsafe fn table_set_headers(m: *mut TableModel, headers: Vec<String>);
        unsafe fn table_insert_rows(m: *mut TableModel, row: i32, rows: Vec<Row>);
        unsafe fn table_remove_rows(m: *mut TableModel, row: i32, count: i32);
        unsafe fn table_update_rows(m: *mut TableModel, row: i32, rows: Vec<Row>);
        unsafe fn table_reset(m: *mut TableModel, rows: Vec<Row>);
        unsafe fn table_model_set_row_icons(m: *mut TableModel, paths: Vec<String>);
        unsafe fn table_model_row_icon_count(m: *mut TableModel) -> i32;
        unsafe fn table_model_set_heat(m: *mut TableModel, column: i32, values: Vec<f64>);
        unsafe fn tree_model_new(columns: i32) -> *mut TreeModel;
        unsafe fn tree_model_drop(m: *mut TreeModel);
        unsafe fn tree_model_reset(m: *mut TreeModel, rows: Vec<TreeRow>);
        unsafe fn tree_model_set_headers(m: *mut TreeModel, headers: Vec<String>);
        unsafe fn tree_model_set_heat(m: *mut TreeModel, column: i32, values: Vec<f64>);
        unsafe fn table_row_count(m: *mut TableModel) -> i32;
        unsafe fn table_column_count(m: *mut TableModel) -> i32;
        unsafe fn table_text(m: *mut TableModel, row: i32, column: i32) -> String;
        unsafe fn view_set_selection_cb(view: *mut Widget, data: *mut Void);
        unsafe fn view_set_header_clicked_cb(view: *mut Widget, data: *mut Void);
        unsafe fn view_set_context_menu_cb(view: *mut Widget, data: *mut Void);
        unsafe fn view_click_header(view: *mut Widget, section: i32);
        unsafe fn view_emit_context_menu(view: *mut Widget, row: i32);
        unsafe fn view_set_column_width(view: *mut Widget, column: i32, width: i32);
        unsafe fn view_stretch_last_section(view: *mut Widget, stretch: bool);
        unsafe fn view_set_select_rows(view: *mut Widget, on: bool);
        unsafe fn view_set_alternating_row_colors(view: *mut Widget, on: bool);
        unsafe fn view_set_heat_delegate(view: *mut Widget);
        unsafe fn view_set_column_hidden(view: *mut Widget, column: i32, hidden: bool);
        unsafe fn tree_view_expand_all(view: *mut Widget, expand: bool);
        unsafe fn tree_view_set_column_width(view: *mut Widget, column: i32, width: i32);
        unsafe fn tree_view_stretch_last_section(view: *mut Widget, stretch: bool);
        unsafe fn tree_view_set_header_clicked_cb(view: *mut Widget, data: *mut Void);
        unsafe fn tree_view_click_header(view: *mut Widget, section: i32);
        unsafe fn tree_view_select_flat(view: *mut Widget, flat: i32);
        unsafe fn tree_model_flat_row_count(m: *mut TreeModel) -> i32;
        unsafe fn view_selected_rows(view: *mut Widget) -> Vec<i32>;
        unsafe fn view_select_row(view: *mut Widget, row: i32);
        unsafe fn view_clear_selection(view: *mut Widget);
        unsafe fn dialog_message_new(
            parent: *mut Widget,
            title: &str,
            text: &str,
            buttons: i32,
        ) -> *mut Dialog;
        unsafe fn dialog_show(d: *mut Dialog);
        unsafe fn dialog_accept(d: *mut Dialog);
        unsafe fn dialog_close(d: *mut Dialog);
        unsafe fn dialog_last_result(d: *mut Dialog) -> i32;
        unsafe fn dialog_set_finished_cb(d: *mut Dialog, data: *mut Void);
        unsafe fn dialog_drop(d: *mut Dialog);
        unsafe fn filedialog_open_new(parent: *mut Widget, title: &str) -> *mut FileDialog;
        unsafe fn filedialog_directory_new(parent: *mut Widget, title: &str) -> *mut FileDialog;
        unsafe fn filedialog_show(d: *mut FileDialog);
        unsafe fn filedialog_accept(d: *mut FileDialog);
        unsafe fn filedialog_close(d: *mut FileDialog);
        unsafe fn filedialog_selected_file(d: *mut FileDialog) -> String;
        unsafe fn filedialog_last_result(d: *mut FileDialog) -> i32;
        unsafe fn filedialog_set_finished_cb(d: *mut FileDialog, data: *mut Void);
        unsafe fn filedialog_drop(d: *mut FileDialog);
        unsafe fn settings_new(organization: &str, application: &str) -> *mut Settings;
        unsafe fn settings_drop(s: *mut Settings);
        unsafe fn settings_value(s: *mut Settings, key: &str) -> String;
        unsafe fn settings_set(s: *mut Settings, key: &str, value: &str);
        unsafe fn settings_remove(s: *mut Settings, key: &str);
        unsafe fn settings_contains(s: *mut Settings, key: &str) -> bool;
        unsafe fn settings_sync(s: *mut Settings);
        unsafe fn widget_set_destroyed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_clicked_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_text_changed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_toggled_cb(w: *mut Widget, data: *mut Void);

    }

    extern "Rust" {
        unsafe fn on_widget_clicked(data: *mut Void);
        unsafe fn on_text_changed(data: *mut Void);
        unsafe fn on_toggled(data: *mut Void);
        unsafe fn on_widget_destroyed(data: *mut Void);
        unsafe fn on_value_changed(data: *mut Void);
        unsafe fn on_date_value_changed(data: *mut Void);
        unsafe fn on_header_clicked(data: *mut Void, section: i32);
        unsafe fn on_view_context_menu(data: *mut Void, row: i32);
        unsafe fn on_gui_scheduled(task: *mut Void);
        unsafe fn on_app_timer(task: *mut Void);
        unsafe fn on_action_triggered(data: *mut Void);
        unsafe fn on_action_destroyed(data: *mut Void);
        unsafe fn on_selection_changed(data: *mut Void);
        unsafe fn on_dialog_finished(data: *mut Void);
        unsafe fn on_filedialog_finished(data: *mut Void);
    }
}

unsafe fn on_widget_clicked(data: *mut bridge::Void) {
    catch_callback("widget clicked", || unsafe {
        crate::callback::clicked(data)
    });
}

unsafe fn on_text_changed(data: *mut bridge::Void) {
    catch_callback("text changed", || unsafe {
        crate::callback::text_changed(data)
    });
}

unsafe fn on_toggled(data: *mut bridge::Void) {
    catch_callback("toggled", || unsafe { crate::callback::toggled(data) });
}

unsafe fn on_widget_destroyed(data: *mut bridge::Void) {
    catch_callback("widget destroyed", || unsafe {
        crate::callback::destroyed(data)
    });
}

unsafe fn on_value_changed(data: *mut bridge::Void) {
    catch_callback("value changed", || unsafe {
        crate::callback::value_changed(data)
    });
}

unsafe fn on_date_value_changed(data: *mut bridge::Void) {
    catch_callback("date value changed", || unsafe {
        crate::callback::date_value_changed(data)
    });
}

unsafe fn on_header_clicked(data: *mut bridge::Void, section: i32) {
    catch_callback("header clicked", || unsafe {
        crate::callback::header_clicked(data, section)
    });
}

unsafe fn on_view_context_menu(data: *mut bridge::Void, row: i32) {
    catch_callback("view context menu", || unsafe {
        crate::callback::view_context_menu(data, row)
    });
}

unsafe fn on_gui_scheduled(task: *mut bridge::Void) {
    catch_callback("GUI-scheduled task", || unsafe {
        let task: Box<Box<dyn FnOnce() + Send>> =
            Box::from_raw(task as *mut Box<dyn FnOnce() + Send>);
        task();
    });
}

unsafe fn on_app_timer(task: *mut bridge::Void) {
    catch_callback("app timer", || unsafe {
        let task: Box<Box<dyn FnOnce()>> = Box::from_raw(task as *mut Box<dyn FnOnce()>);
        task();
    });
}

unsafe fn on_action_triggered(data: *mut bridge::Void) {
    catch_callback("action triggered", || unsafe {
        crate::callback::action_triggered(data)
    });
}

unsafe fn on_action_destroyed(data: *mut bridge::Void) {
    catch_callback("action destroyed", || unsafe {
        crate::callback::action_destroyed(data)
    });
}

unsafe fn on_selection_changed(data: *mut bridge::Void) {
    catch_callback("selection changed", || unsafe {
        crate::callback::selection_changed(data)
    });
}

unsafe fn on_dialog_finished(data: *mut bridge::Void) {
    catch_callback("dialog finished", || unsafe {
        crate::callback::dialog_finished(data)
    });
}

unsafe fn on_filedialog_finished(data: *mut bridge::Void) {
    catch_callback("file dialog finished", || unsafe {
        crate::callback::filedialog_finished(data)
    });
}

/// Run a callback that C++ invoked across the FFI boundary, containing any
/// panic so it never unwinds through the C ABI. An uncontained panic would be
/// converted by CXX into a `rust::Error` C++ exception; uncaught in the Qt
/// event loop, that terminates the application. We log the panic instead.
fn catch_callback<F>(name: &'static str, f: F)
where
    F: FnOnce(),
{
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    if let Err(payload) = result {
        let message = if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            String::from("<non-string panic payload>")
        };
        yse_model::log_error(format!("panic in {name} callback: {message}"));
        // Always surface the panic, even when no logger is installed: silently
        // swallowing it would hide bugs during development.
        eprintln!("yse: panic contained in {name} callback: {message}");
    }
}

pub use bridge::*;
