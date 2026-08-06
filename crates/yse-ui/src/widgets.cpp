// Implementation of the yse-ui C++ shim.

#include "yse-ui/src/widgets.h"

#include <QCoreApplication>
#include <QtCore/QSettings>
#include <QtGui/QKeySequence>
#include <QtCore/QMetaObject>
#include <QTimer>
#include <QtGui/QAction>
#include <QtWidgets/QApplication>
#include <QtWidgets/QBoxLayout>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QFileDialog>
#include <QItemSelectionModel>
#include <QtWidgets/QGridLayout>
#include <QtWidgets/QHBoxLayout>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QListView>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QMenu>
#include <QtWidgets/QMenuBar>
#include <QtWidgets/QMessageBox>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QToolBar>
#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

#include "yse-ui/src/lib.cxx.h"

namespace {

QApplication* ensure_app()
{
  static int argc = 1;
  static const char arg0[] = "yse";
  static char* argv[] = { const_cast<char*>(arg0), nullptr };
  static QApplication* app = new QApplication(argc, argv);
  return app;
}

yse_ui::Widget* new_child(QWidget* q, yse_ui::Widget* parent)
{
  if (parent != nullptr && parent->q != nullptr) {
    q->setParent(parent->q);
  }
  return new yse_ui::Widget{ q, false, true, nullptr };
}

} // namespace

namespace yse_ui {

void app_init()
{
  ensure_app();
}

int app_exec()
{
  return ensure_app()->exec();
}

void app_quit_after(int ms)
{
  QTimer::singleShot(ms, ensure_app(), &QCoreApplication::quit);
}

void app_schedule_gui(Void* task)
{
  QMetaObject::invokeMethod(
    ensure_app(),
    [task] { yse_ui::on_gui_scheduled(task); },
    Qt::QueuedConnection);
}

void app_invoke_after(int ms, Void* task)
{
  QTimer::singleShot(ms, ensure_app(), [task] { yse_ui::on_app_timer(task); });
}

Widget* widget_new_window()
{
  return new Widget{ new QMainWindow(), true, true, nullptr };
}

Widget* widget_new_label(rust::Str text, Widget* parent)
{
  return new_child(new QLabel(QString::fromUtf8(text.data(), text.size())), parent);
}

Widget* widget_new_button(rust::Str text, Widget* parent)
{
  return new_child(new QPushButton(QString::fromUtf8(text.data(), text.size())), parent);
}

Widget* widget_new_line_edit(rust::Str text, Widget* parent)
{
  return new_child(new QLineEdit(QString::fromUtf8(text.data(), text.size())), parent);
}

Widget* widget_new_checkbox(rust::Str text, Widget* parent)
{
  return new_child(new QCheckBox(QString::fromUtf8(text.data(), text.size())), parent);
}

Widget* widget_new_row(Widget* parent)
{
  auto* q = new QWidget();
  new QHBoxLayout(q);
  if (auto* mw = qobject_cast<QMainWindow*>(parent->q)) {
    if (mw->centralWidget() == nullptr) {
      mw->setCentralWidget(q);
      return new Widget{ q, false, true, nullptr };
    }
  }
  return new_child(q, parent);
}

Widget* widget_new_column(Widget* parent)
{
  auto* q = new QWidget();
  new QVBoxLayout(q);
  if (auto* mw = qobject_cast<QMainWindow*>(parent->q)) {
    if (mw->centralWidget() == nullptr) {
      mw->setCentralWidget(q);
      return new Widget{ q, false, true, nullptr };
    }
  }
  return new_child(q, parent);
}

Widget* widget_new_grid(Widget* parent)
{
  auto* q = new QWidget();
  new QGridLayout(q);
  if (auto* mw = qobject_cast<QMainWindow*>(parent->q)) {
    if (mw->centralWidget() == nullptr) {
      mw->setCentralWidget(q);
      return new Widget{ q, false, true, nullptr };
    }
  }
  return new_child(q, parent);
}

Widget* widget_new_menubar(Widget* window)
{
  auto* q = new QMenuBar(window->q);
  static_cast<QMainWindow*>(window->q)->setMenuBar(q);
  return new Widget{ q, false, true, nullptr };
}

Widget* widget_new_toolbar(Widget* window)
{
  auto* q = new QToolBar(window->q);
  static_cast<QMainWindow*>(window->q)->addToolBar(q);
  return new Widget{ q, false, true, nullptr };
}

Widget* menu_new(rust::Str title, Widget* menubar)
{
  auto* q = new QMenu(QString::fromUtf8(title.data(), title.size()), menubar->q);
  static_cast<QMenuBar*>(menubar->q)->addMenu(q);
  return new Widget{ q, false, true, nullptr };
}

void menu_add_separator(Widget* menu)
{
  if (menu->alive && menu->q != nullptr) {
    static_cast<QMenu*>(menu->q)->addSeparator();
  }
}

void toolbar_add_action(Widget* toolbar, Action* action)
{
  if (toolbar->alive && action->alive && toolbar->q != nullptr) {
    static_cast<QToolBar*>(toolbar->q)->addAction(action->q);
  }
}

void widget_drop(Widget* w)
{
  if (w == nullptr) {
    return;
  }
  // A wrapper owns exactly these connections. Never use a wildcard
  // disconnect here: embedders may have installed unrelated Qt connections.
  QObject::disconnect(w->destroyed_connection);
  QObject::disconnect(w->clicked_connection);
  QObject::disconnect(w->text_changed_connection);
  QObject::disconnect(w->toggled_connection);
  QObject::disconnect(w->selection_connection);
  w->destroyed_cb_data = nullptr;
  w->clicked_cb_data = nullptr;
  w->text_changed_cb_data = nullptr;
  w->toggled_cb_data = nullptr;
  w->selection_cb_data = nullptr;
  if (w->owned && w->alive) {
    // Windows own their QWidget; children are deleted by their Qt parent.
    delete w->q;
  }
  delete w;
}

void widget_show(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    w->q->show();
  }
}

void widget_set_visible(Widget* w, bool visible)
{
  if (w->alive && w->q != nullptr) {
    w->q->setVisible(visible);
  }
}

void widget_set_enabled(Widget* w, bool enabled)
{
  if (w->alive && w->q != nullptr) {
    w->q->setEnabled(enabled);
  }
}

void widget_set_title(Widget* w, rust::Str title)
{
  if (w->alive && w->q != nullptr) {
    w->q->setWindowTitle(QString::fromUtf8(title.data(), title.size()));
  }
}

void widget_resize(Widget* w, int width, int height)
{
  if (w->alive && w->q != nullptr) {
    w->q->resize(width, height);
  }
}

void layout_add(Widget* layout, Widget* child)
{
  if (layout->alive && child->alive && layout->q != nullptr && child->q != nullptr) {
    layout->q->layout()->addWidget(child->q);
  }
}

void layout_add_spacer(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    if (auto* box = qobject_cast<QBoxLayout*>(w->q->layout())) {
      box->addStretch();
    }
  }
}

void grid_add(Widget* grid, Widget* child, int row, int column)
{
  if (grid->alive && child->alive && grid->q != nullptr && child->q != nullptr) {
    qobject_cast<QGridLayout*>(grid->q->layout())->addWidget(child->q, row, column);
  }
}

void label_set_text(Widget* w, rust::Str text)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QLabel*>(w->q)->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

rust::String label_text(Widget* w)
{
  return rust::String(
    static_cast<QLabel*>(w->q)->text().toUtf8().constData());
}

void line_edit_set_text(Widget* w, rust::Str text)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QLineEdit*>(w->q)->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

rust::String line_edit_text(Widget* w)
{
  return rust::String(
    static_cast<QLineEdit*>(w->q)->text().toUtf8().constData());
}

void checkbox_set_checked(Widget* w, bool checked)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QCheckBox*>(w->q)->setChecked(checked);
  }
}

bool checkbox_checked(Widget* w)
{
  return static_cast<QCheckBox*>(w->q)->isChecked();
}

void button_click(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QPushButton*>(w->q)->click();
  }
}

Dialog* dialog_message_new(Widget* parent, rust::Str title, rust::Str text, int buttons)
{
  auto* q = new QMessageBox(parent != nullptr ? parent->q : nullptr);
  q->setWindowTitle(QString::fromUtf8(title.data(), title.size()));
  q->setText(QString::fromUtf8(text.data(), text.size()));
  switch (buttons) {
    case 1:
      q->setStandardButtons(QMessageBox::Ok | QMessageBox::Cancel);
      break;
    case 2:
      q->setStandardButtons(QMessageBox::Yes | QMessageBox::No);
      break;
    default:
      q->setStandardButtons(QMessageBox::Ok);
      break;
  }
  auto* dialog = new Dialog(q, true, 0, nullptr);
  dialog->destroyed_connection = QObject::connect(q, &QObject::destroyed, [dialog] {
    dialog->alive = false;
    dialog->q = nullptr;
    dialog->cb_data = nullptr;
  });
  return dialog;
}

void dialog_show(Dialog* d)
{
  if (d->alive && d->q != nullptr) {
    d->q->show();
  }
}

void dialog_accept(Dialog* d)
{
  if (d->alive && d->q != nullptr) {
    d->q->accept();
  }
}

void dialog_close(Dialog* d)
{
  if (d->alive && d->q != nullptr) {
    d->q->close();
  }
}

int dialog_last_result(Dialog* d)
{
  return d->last_result;
}

void dialog_set_finished_cb(Dialog* d, Void* data)
{
  QObject::disconnect(d->finished_connection);
  d->cb_data = static_cast<void*>(data);
  d->finished_connection = QObject::connect(
    d->q, &QMessageBox::finished, d->q, [d](int result) {
      d->last_result = result;
      auto* data = static_cast<yse_ui::Void*>(d->cb_data);
      auto* q = d->q;
      d->alive = false;
      d->cb_data = nullptr;
      if (data != nullptr) {
        yse_ui::on_dialog_finished(data);
      }
      d->q = nullptr;
      q->deleteLater();
    });
}

void dialog_drop(Dialog* d)
{
  if (d == nullptr) {
    return;
  }
  QObject::disconnect(d->finished_connection);
  QObject::disconnect(d->destroyed_connection);
  if (d->alive && d->q != nullptr) {
    delete d->q;
  }
  d->alive = false;
  d->cb_data = nullptr;
  delete d;
}

FileDialog* filedialog_open_new(Widget* parent, rust::Str title)
{
  auto* q = new QFileDialog(parent != nullptr ? parent->q : nullptr);
  q->setWindowTitle(QString::fromUtf8(title.data(), title.size()));
  q->setFileMode(QFileDialog::ExistingFile);
  auto* dialog = new FileDialog(q, true, nullptr);
  dialog->destroyed_connection = QObject::connect(q, &QObject::destroyed, [dialog] {
    dialog->alive = false;
    dialog->q = nullptr;
    dialog->cb_data = nullptr;
  });
  return dialog;
}

void filedialog_show(FileDialog* d)
{
  if (d->alive && d->q != nullptr) {
    d->q->show();
  }
}

void filedialog_accept(FileDialog* d)
{
  if (d->alive && d->q != nullptr) {
    // QFileDialog overrides accept()/done() as protected; invoking through
    // the meta-object triggers the same accepted signal.
    QMetaObject::invokeMethod(d->q, "accept", Qt::DirectConnection);
  }
}

void filedialog_close(FileDialog* d)
{
  if (d->alive && d->q != nullptr) {
    d->q->close();
  }
}

rust::String filedialog_selected_file(FileDialog* d)
{
  const QStringList files = d->q->selectedFiles();
  const QString first = files.isEmpty() ? QString() : files.first();
  return rust::String(first.toUtf8().constData());
}

void filedialog_set_finished_cb(FileDialog* d, Void* data)
{
  QObject::disconnect(d->finished_connection);
  d->cb_data = static_cast<void*>(data);
  d->finished_connection = QObject::connect(
    d->q, &QFileDialog::finished, d->q, [d](int /*result*/) {
      auto* data = static_cast<yse_ui::Void*>(d->cb_data);
      auto* q = d->q;
      d->alive = false;
      d->cb_data = nullptr;
      if (data != nullptr) {
        yse_ui::on_filedialog_finished(data);
      }
      d->q = nullptr;
      q->deleteLater();
    });
}

void filedialog_drop(FileDialog* d)
{
  if (d == nullptr) {
    return;
  }
  QObject::disconnect(d->finished_connection);
  QObject::disconnect(d->destroyed_connection);
  if (d->alive && d->q != nullptr) {
    delete d->q;
  }
  d->alive = false;
  d->cb_data = nullptr;
  delete d;
}

Settings* settings_new(rust::Str organization, rust::Str application)
{
  auto* q = new QSettings(
    QString::fromUtf8(organization.data(), organization.size()),
    QString::fromUtf8(application.data(), application.size()));
  return new Settings{ q, true, nullptr };
}

void settings_drop(Settings* s)
{
  if (s == nullptr) {
    return;
  }
  delete s->q;
  delete s;
}

rust::String settings_value(Settings* s, rust::Str key)
{
  const QVariant stored = s->q->value(
    QString::fromUtf8(key.data(), key.size()), QString());
  return rust::String(stored.toString().toUtf8().constData());
}

void settings_set(Settings* s, rust::Str key, rust::Str value)
{
  s->q->setValue(
    QString::fromUtf8(key.data(), key.size()),
    QString::fromUtf8(value.data(), value.size()));
}

void settings_remove(Settings* s, rust::Str key)
{
  s->q->remove(QString::fromUtf8(key.data(), key.size()));
}

bool settings_contains(Settings* s, rust::Str key)
{
  return s->q->contains(QString::fromUtf8(key.data(), key.size()));
}

void settings_sync(Settings* s)
{
  s->q->sync();
}

Action* action_new(rust::Str text, Widget* parent)
{
  auto* q = new QAction(
    QString::fromUtf8(text.data(), text.size()),
    parent != nullptr ? parent->q : nullptr);
  return new Action{ q, true, nullptr };
}

void action_drop(Action* a)
{
  if (a == nullptr) {
    return;
  }
  QObject::disconnect(a->destroyed_connection);
  QObject::disconnect(a->triggered_connection);
  a->alive = false;
  a->triggered_cb_data = nullptr;
  a->destroyed_cb_data = nullptr;
  delete a;
}

void action_set_text(Action* a, rust::Str text)
{
  if (a->alive && a->q != nullptr) {
    a->q->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

void action_set_enabled(Action* a, bool enabled)
{
  if (a->alive && a->q != nullptr) {
    a->q->setEnabled(enabled);
  }
}

void action_set_shortcut(Action* a, rust::Str shortcut)
{
  if (a->alive && a->q != nullptr) {
    a->q->setShortcut(
      QKeySequence(QString::fromUtf8(shortcut.data(), shortcut.size())));
  }
}

void action_trigger(Action* a)
{
  if (a->alive && a->q != nullptr) {
    a->q->trigger();
  }
}

void action_set_triggered_cb(Action* a, Void* data)
{
  QObject::disconnect(a->triggered_connection);
  a->triggered_cb_data = static_cast<void*>(data);
  a->triggered_connection = QObject::connect(a->q, &QAction::triggered, [a] {
    if (a->triggered_cb_data != nullptr) {
      yse_ui::on_action_triggered(
        static_cast<yse_ui::Void*>(a->triggered_cb_data));
    }
  });
}

void action_set_destroyed_cb(Action* a, Void* data)
{
  QObject::disconnect(a->destroyed_connection);
  a->destroyed_cb_data = static_cast<void*>(data);
  a->destroyed_connection = QObject::connect(a->q, &QObject::destroyed, [a] {
    auto* data = static_cast<yse_ui::Void*>(a->destroyed_cb_data);
    a->alive = false;
    a->q = nullptr;
    a->destroyed_cb_data = nullptr;
    if (data != nullptr) {
      yse_ui::on_action_destroyed(data);
    }
  });
}

void widget_set_destroyed_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->destroyed_connection);
  w->destroyed_cb_data = static_cast<void*>(data);
  w->destroyed_connection = QObject::connect(w->q, &QObject::destroyed, [w] {
    auto* data = static_cast<yse_ui::Void*>(w->destroyed_cb_data);
    w->alive = false;
    w->q = nullptr;
    w->destroyed_cb_data = nullptr;
    if (data != nullptr) {
      yse_ui::on_widget_destroyed(data);
    }
  });
}

void widget_set_clicked_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->clicked_connection);
  w->clicked_cb_data = static_cast<void*>(data);
  w->clicked_connection = QObject::connect(
    static_cast<QPushButton*>(w->q), &QPushButton::clicked, [w] {
    if (w->clicked_cb_data != nullptr) {
      yse_ui::on_widget_clicked(static_cast<yse_ui::Void*>(w->clicked_cb_data));
    }
  });
}

void widget_set_text_changed_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->text_changed_connection);
  w->text_changed_cb_data = static_cast<void*>(data);
  w->text_changed_connection = QObject::connect(
    static_cast<QLineEdit*>(w->q), &QLineEdit::textChanged, [w] {
    if (w->text_changed_cb_data != nullptr) {
      yse_ui::on_text_changed(
        static_cast<yse_ui::Void*>(w->text_changed_cb_data));
    }
  });
}

void widget_set_toggled_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->toggled_connection);
  w->toggled_cb_data = static_cast<void*>(data);
  w->toggled_connection = QObject::connect(
    static_cast<QCheckBox*>(w->q), &QCheckBox::toggled, [w] {
    if (w->toggled_cb_data != nullptr) {
      yse_ui::on_toggled(static_cast<yse_ui::Void*>(w->toggled_cb_data));
    }
  });
}

} // namespace yse_ui
