// Hand-written C++ shim for yse-ui: the entire Qt Widgets surface.
//
// Rust owns the reactive state (yse-model) and the component lifecycle; this
// shim owns the raw widgets. The expansion pattern is one factory per widget
// family plus small property/signal accessors.

#pragma once

#include <cstddef>
#include <cstdint>
#include "rust/cxx.h"

class QWidget;
class QAction;
class QMessageBox;
class QFileDialog;
class QSettings;
class RustListModel;
class RustTableModel;

namespace yse_ui {

/// Opaque handle used as callback data by the cxx bridge.
struct Void;
struct Row;

/// Opaque wrapper around a QWidget. `owned` means the wrapper deletes the
/// widget when dropped (windows); child widgets are owned by their Qt parent.
struct Widget {
  QWidget* q;
  bool owned;
  bool alive;
  void* cb_data;
};

/// Opaque wrapper around a QAction.
struct Action {
  QAction* q;
  bool alive;
  void* cb_data;
};

/// Opaque wrapper around a RustListModel (QAbstractListModel mirror).
struct Model {
  RustListModel* q;
  bool alive;
  void* cb_data;
};

/// Opaque wrapper around a RustTableModel.
struct TableModel {
  RustTableModel* q;
  bool alive;
  void* cb_data;
};

/// Opaque wrapper around a QMessageBox.
struct Dialog {
  QMessageBox* q;
  bool alive;
  int last_result;
  void* cb_data;
};

/// Opaque wrapper around a QFileDialog.
struct FileDialog {
  QFileDialog* q;
  bool alive;
  void* cb_data;
};

/// Opaque wrapper around a QSettings store.
struct Settings {
  QSettings* q;
  bool alive;
  void* cb_data;
};

// Application ---------------------------------------------------------------

void app_init();
int app_exec();
void app_quit_after(int ms);
void app_schedule_gui(Void* task);
void app_invoke_after(int ms, Void* task);

// Widget factories -----------------------------------------------------------

Widget* widget_new_window();
Widget* widget_new_label(rust::Str text, Widget* parent);
Widget* widget_new_button(rust::Str text, Widget* parent);
Widget* widget_new_line_edit(rust::Str text, Widget* parent);
Widget* widget_new_checkbox(rust::Str text, Widget* parent);
Widget* widget_new_row(Widget* parent);
Widget* widget_new_column(Widget* parent);
Widget* widget_new_grid(Widget* parent);
Widget* widget_new_menubar(Widget* window);
Widget* widget_new_toolbar(Widget* window);
Widget* menu_new(rust::Str title, Widget* menubar);
void menu_add_separator(Widget* menu);
void toolbar_add_action(Widget* toolbar, Action* action);
Widget* widget_new_list_view(Model* model, Widget* parent);
Widget* widget_new_table_view(TableModel* model, Widget* parent);

// Lifecycle / structure ------------------------------------------------------

void widget_drop(Widget* w);
void widget_show(Widget* w);
void widget_set_visible(Widget* w, bool visible);
void widget_set_enabled(Widget* w, bool enabled);
void widget_set_title(Widget* w, rust::Str title);
void widget_resize(Widget* w, int width, int height);
void layout_add(Widget* layout, Widget* child);
void layout_add_spacer(Widget* w);
void grid_add(Widget* grid, Widget* child, int row, int column);

// Dialogs --------------------------------------------------------------------

Dialog* dialog_message_new(Widget* parent, rust::Str title, rust::Str text, int buttons);
void dialog_show(Dialog* d);
void dialog_accept(Dialog* d);
void dialog_close(Dialog* d);
int dialog_last_result(Dialog* d);
void dialog_set_finished_cb(Dialog* d, Void* data);
void dialog_drop(Dialog* d);

FileDialog* filedialog_open_new(Widget* parent, rust::Str title);
void filedialog_show(FileDialog* d);
void filedialog_accept(FileDialog* d);
void filedialog_close(FileDialog* d);
rust::String filedialog_selected_file(FileDialog* d);
void filedialog_set_finished_cb(FileDialog* d, Void* data);
void filedialog_drop(FileDialog* d);

// Settings --------------------------------------------------------------------

Settings* settings_new(rust::Str organization, rust::Str application);
void settings_drop(Settings* s);
rust::String settings_value(Settings* s, rust::Str key);
void settings_set(Settings* s, rust::Str key, rust::Str value);
void settings_remove(Settings* s, rust::Str key);
bool settings_contains(Settings* s, rust::Str key);
void settings_sync(Settings* s);

// List view selection --------------------------------------------------------

void view_set_selection_cb(Widget* view, Void* data);
rust::Vec<int32_t> view_selected_rows(Widget* view);
void view_select_row(Widget* view, int row);
void view_clear_selection(Widget* view);

// Properties -----------------------------------------------------------------

void label_set_text(Widget* w, rust::Str text);
rust::String label_text(Widget* w);
void line_edit_set_text(Widget* w, rust::Str text);
rust::String line_edit_text(Widget* w);
void checkbox_set_checked(Widget* w, bool checked);
bool checkbox_checked(Widget* w);
void button_click(Widget* w);

// Actions --------------------------------------------------------------------

Action* action_new(rust::Str text, Widget* parent);
void action_drop(Action* a);
void action_set_text(Action* a, rust::Str text);
void action_set_enabled(Action* a, bool enabled);
void action_set_shortcut(Action* a, rust::Str shortcut);
void action_trigger(Action* a);
void action_set_triggered_cb(Action* a, Void* data);
void action_set_destroyed_cb(Action* a, Void* data);

// List models -----------------------------------------------------------------

Model* model_new();
void model_drop(Model* m);
void model_insert_rows(Model* m, int row, rust::Vec<rust::String> items);
void model_remove_rows(Model* m, int row, int count);
void model_update_rows(Model* m, int row, rust::Vec<rust::String> items);
void model_reset(Model* m, rust::Vec<rust::String> items);
int model_row_count(Model* m);
rust::String model_text(Model* m, int row);

// Table models ----------------------------------------------------------------

TableModel* table_model_new(int columns);
void table_model_drop(TableModel* m);
void table_set_headers(TableModel* m, rust::Vec<rust::String> headers);
void table_insert_rows(TableModel* m, int row, rust::Vec<Row> rows);
void table_remove_rows(TableModel* m, int row, int count);
void table_update_rows(TableModel* m, int row, rust::Vec<Row> rows);
void table_reset(TableModel* m, rust::Vec<Row> rows);
int table_row_count(TableModel* m);
int table_column_count(TableModel* m);
rust::String table_text(TableModel* m, int row, int column);

// Signal wiring: data is an opaque pointer handed back to Rust ----------------

void widget_set_destroyed_cb(Widget* w, Void* data);
void widget_set_clicked_cb(Widget* w, Void* data);
void widget_set_text_changed_cb(Widget* w, Void* data);
void widget_set_toggled_cb(Widget* w, Void* data);

} // namespace yse_ui
