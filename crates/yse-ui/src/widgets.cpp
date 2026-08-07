// Implementation of the yse-ui C++ shim.

#include "yse-ui/src/widgets.h"
#include "yse-ui/src/qt_object.cxxqt.h"

// CXX-Qt generates its Rust glue below `yse_ui::rust`; the existing CXX
// widget shim predates that namespace and uses Rust's shared CXX types.
namespace yse_ui::rust {
using ::rust::Str;
using ::rust::String;
template <typename T>
using Vec = ::rust::Vec<T>;
}

#include <QCoreApplication>
#include <QPointer>
#include <QtCore/QSettings>
#include <QtGui/QKeySequence>
#include <QtGui/QIcon>
#include <QtGui/QStyleHints>
#include <QtCore/QMetaObject>
#include <QtGui/QFont>
#include <QtGui/QPainter>
#include <QtGui/QPainterPath>
#include <QtCore/QVector>
#include <QTimer>
#include <QtGui/QAction>
#include <QtWidgets/QApplication>
#include <QtWidgets/QBoxLayout>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QDateTimeEdit>
#include <QtWidgets/QDateEdit>
#include <QtWidgets/QFileDialog>
#include <QtWidgets/QHeaderView>
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
#include <QtWidgets/QProgressBar>
#include <QtWidgets/QSlider>
#include <QtWidgets/QSpinBox>
#include <QtWidgets/QSizePolicy>
#include <QtWidgets/QStyle>
#include <QtWidgets/QTabWidget>
#include <QtWidgets/QToolBar>
#include <QtWidgets/QTreeView>
#include <QtWidgets/QTimeEdit>
#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

#include <numeric>

#include "yse-ui/src/bridge.cxx.h"

namespace {

class DiskMapWidget final : public QWidget {
public:
  struct Segment { QString label; double share; };

  explicit DiskMapWidget(QWidget* parent) : QWidget(parent) {
    setMinimumHeight(210);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
  }

  void setSegments(rust::Vec<rust::String> labels, rust::Vec<double> shares) {
    segments_.clear();
    const auto count = std::min(labels.size(), shares.size());
    for (size_t index = 0; index < count; ++index) {
      if (shares[index] > 0.0) {
        segments_.push_back({QString::fromUtf8(labels[index].data(), labels[index].size()), shares[index]});
      }
    }
    update();
  }

protected:
  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QRectF bounds = rect().adjusted(18, 12, -18, -12);
    const qreal legendWidth = std::min<qreal>(220, bounds.width() * .38);
    const QRectF chartBounds(bounds.left(), bounds.top(), bounds.width() - legendWidth - 18, bounds.height());
    const qreal diameter = std::min(chartBounds.width(), chartBounds.height());
    const QRectF circle(chartBounds.left() + (chartBounds.width() - diameter) / 2.0,
                        chartBounds.top() + (chartBounds.height() - diameter) / 2.0,
                        diameter, diameter);
    const QRectF legendBounds(bounds.right() - legendWidth, bounds.top(), legendWidth, bounds.height());
    const QVector<QColor> colors = {QColor("#3daee9"), QColor("#8e7cc3"), QColor("#f6c344"), QColor("#ef6c6c"), QColor("#42b883"), QColor("#e67e22")};
    const double total = std::accumulate(segments_.cbegin(), segments_.cend(), 0.0,
      [](double value, const Segment& segment) { return value + segment.share; });
    if (total <= 0.0) {
      const QSize iconSize(64, 64);
      const QRect iconRect(bounds.center().x() - iconSize.width() / 2,
                           bounds.center().y() - 64,
                           iconSize.width(), iconSize.height());
      QIcon::fromTheme("folder-open").paint(&painter, iconRect);
      painter.setPen(palette().color(QPalette::Text));
      QFont title = painter.font();
      title.setPointSizeF(title.pointSizeF() + 2.0);
      title.setWeight(QFont::DemiBold);
      painter.setFont(title);
      painter.drawText(QRectF(bounds.left(), iconRect.bottom() + 12, bounds.width(), 30),
                       Qt::AlignHCenter | Qt::AlignTop, "Choose a folder to scan");
      painter.setFont(font());
      painter.setPen(palette().color(QPalette::PlaceholderText));
      painter.drawText(QRectF(bounds.left(), iconRect.bottom() + 42, bounds.width(), 30),
                       Qt::AlignHCenter | Qt::AlignTop,
                       "Filelight will show the folders using the most space.");
      return;
    }
    int start = 90 * 16;
    for (int index = 0; index < segments_.size(); ++index) {
      const int span = -qRound(segments_[index].share / total * 360.0 * 16.0);
      painter.setPen(QPen(palette().color(QPalette::Window), 2));
      painter.setBrush(colors[index % colors.size()]);
      painter.drawPie(circle, start, span);
      start += span;
    }
    painter.setPen(Qt::NoPen);
    painter.setBrush(palette().color(QPalette::Window));
    painter.drawEllipse(circle.adjusted(diameter * .27, diameter * .27, -diameter * .27, -diameter * .27));
    const auto largest = std::max_element(segments_.cbegin(), segments_.cend(),
      [](const Segment& left, const Segment& right) { return left.share < right.share; });
    painter.setPen(palette().color(QPalette::Text));
    painter.drawText(circle, Qt::AlignCenter,
      QString("%1%\n%2").arg(largest->share / total * 100.0, 0, 'f', 1).arg(largest->label));

    const QFontMetrics metrics(painter.font());
    constexpr int rowHeight = 26;
    const int visibleSegments = std::min(static_cast<int>(segments_.size()), 6);
    for (int index = 0; index < visibleSegments; ++index) {
      const int y = static_cast<int>(legendBounds.top()) + index * rowHeight;
      painter.setPen(Qt::NoPen);
      painter.setBrush(colors[index % colors.size()]);
      painter.drawRoundedRect(QRectF(legendBounds.left(), y + 5, 11, 11), 2, 2);
      painter.setPen(palette().color(QPalette::Text));
      const auto label = metrics.elidedText(segments_[index].label, Qt::ElideRight,
                                            static_cast<int>(legendBounds.width()) - 62);
      painter.drawText(QRectF(legendBounds.left() + 18, y, legendBounds.width() - 62, rowHeight),
                       Qt::AlignVCenter | Qt::AlignLeft, label);
      painter.drawText(QRectF(legendBounds.right() - 42, y, 42, rowHeight),
                       Qt::AlignVCenter | Qt::AlignRight,
                       QString::number(segments_[index].share / total * 100.0, 'f', 1) + "%");
    }
    if (segments_.size() > visibleSegments) {
      painter.setPen(palette().color(QPalette::PlaceholderText));
      painter.drawText(QRectF(legendBounds.left(), legendBounds.top() + visibleSegments * rowHeight,
                              legendBounds.width(), rowHeight), Qt::AlignVCenter | Qt::AlignLeft,
                       QString("+%1 more").arg(segments_.size() - visibleSegments));
    }
  }

private:
  QVector<Segment> segments_;
};

class LineChartWidget final : public QWidget {
public:
  explicit LineChartWidget(QWidget* parent) : QWidget(parent) {
    setMinimumHeight(140);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
  }

  void setSeries(rust::Vec<double> points) {
    series_.clear();
    if (!points.empty()) {
      QVector<double> single;
      single.reserve(points.size());
      for (double point : points) {
        single.push_back(point);
      }
      series_.push_back(single);
    }
    update();
  }

  void setSeriesMulti(rust::Vec<double> points, size_t series) {
    series_.clear();
    if (series == 0) {
      update();
      return;
    }
    const size_t chunk = points.size() / series;
    for (size_t index = 0; index < series; ++index) {
      QVector<double> single;
      single.reserve(chunk);
      for (size_t offset = index * chunk; offset < (index + 1) * chunk; ++offset) {
        const double point = points[offset];
        single.push_back(point);
      }
      series_.push_back(single);
    }
    update();
  }

protected:
  void paintEvent(QPaintEvent*) override {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const QRectF bounds = rect().adjusted(6, 6, -6, -6);
    painter.setPen(palette().color(QPalette::Mid));
    painter.drawRect(bounds);

    double minValue = 0.0;
    double maxValue = 100.0;
    for (const auto& points : series_) {
      for (double point : points) {
        minValue = std::min(minValue, point);
        maxValue = std::max(maxValue, point);
      }
    }
    if (maxValue - minValue < 1.0) {
      maxValue = minValue + 1.0;
    }

    const QColor palette_[8] = {
      QColor("#3daee9"), QColor("#8e7cc3"), QColor("#f6c344"), QColor("#ef6c6c"),
      QColor("#42b883"), QColor("#e67e22"), QColor("#b48ead"), QColor("#7f8c8d"),
    };
    painter.setClipping(true);
    painter.setClipRect(bounds.adjusted(1, 1, -1, -1));
    for (int index = 0; index < series_.size(); ++index) {
      const auto& points = series_[index];
      if (points.size() < 2) {
        continue;
      }
      painter.setPen(QPen(palette_[index % 8], 2));
      QPainterPath path;
      for (int point = 0; point < points.size(); ++point) {
        const double x = bounds.left() + bounds.width() * point / (points.size() - 1);
        const double y = bounds.bottom() -
          (points[point] - minValue) / (maxValue - minValue) * bounds.height();
        if (point == 0) {
          path.moveTo(x, y);
        } else {
          path.lineTo(x, y);
        }
      }
      painter.drawPath(path);
    }
  }

private:
  QVector<QVector<double>> series_;
};

QApplication* ensure_app()
{
  static int argc = 1;
  static const char arg0[] = "yse";
  static char* argv[] = { const_cast<char*>(arg0), nullptr };
  static QApplication* app = new QApplication(argc, argv);
  return app;
}

yse_ui::Widget* new_widget(QWidget* q, bool owned)
{
  auto* widget = new yse_ui::Widget{ q, owned, true, nullptr };
  auto* state = new yse_ui::WidgetState(q);
  widget->state = state;
  QObject::connect(state, &yse_ui::WidgetState::enabledChanged, q, [state, q] {
    q->setEnabled(state->getEnabled());
  });
  QObject::connect(state, &yse_ui::WidgetState::visibleChanged, q, [state, q] {
    q->setVisible(state->getVisible());
  });
  QObject::connect(state, &yse_ui::WidgetState::titleChanged, q, [state, q] {
    q->setWindowTitle(state->getTitle());
  });
  return widget;
}

yse_ui::Widget* new_child(QWidget* q, yse_ui::Widget* parent)
{
  if (parent != nullptr && parent->q != nullptr) {
    q->setParent(parent->q);
  }
  return new_widget(q, false);
}

} // namespace

namespace yse_ui {

Widget* widget_wrap_child(QWidget* q, Widget* parent)
{
  return new_child(q, parent);
}

void app_init()
{
  ensure_app();
}

bool app_system_color_scheme_dark()
{
  QApplication* app = ensure_app();
  const auto scheme = app->styleHints()->colorScheme();
  if (scheme == Qt::ColorScheme::Dark) {
    return true;
  }
  if (scheme == Qt::ColorScheme::Light) {
    return false;
  }
  // Unknown scheme: fall back to the palette's window luminance.
  return app->palette().color(QPalette::Window).lightness() < 128;
}

void app_apply_color_scheme(bool dark)
{
  QApplication* app = ensure_app();
  app->setStyle(QStringLiteral("Fusion"));
  if (!dark) {
    app->setPalette(app->style()->standardPalette());
    return;
  }
  QPalette palette;
  const QColor window(53, 53, 53);
  const QColor base(35, 35, 35);
  const QColor text(220, 220, 220);
  const QColor accent(58, 174, 233);
  palette.setColor(QPalette::Window, window);
  palette.setColor(QPalette::WindowText, text);
  palette.setColor(QPalette::Base, base);
  palette.setColor(QPalette::AlternateBase, window);
  palette.setColor(QPalette::ToolTipBase, base);
  palette.setColor(QPalette::ToolTipText, text);
  palette.setColor(QPalette::Text, text);
  palette.setColor(QPalette::Button, window);
  palette.setColor(QPalette::ButtonText, text);
  palette.setColor(QPalette::BrightText, QColor(255, 0, 0));
  palette.setColor(QPalette::Link, accent);
  palette.setColor(QPalette::Highlight, accent);
  palette.setColor(QPalette::HighlightedText, QColor(10, 10, 10));
  palette.setColor(QPalette::PlaceholderText, QColor(150, 150, 150));
  palette.setColor(QPalette::Disabled, QPalette::Text, QColor(120, 120, 120));
  palette.setColor(QPalette::Disabled, QPalette::ButtonText, QColor(120, 120, 120));
  app->setPalette(palette);
}

void app_set_style_sheet(rust::Str style_sheet)
{
  ensure_app()->setStyleSheet(QString::fromUtf8(style_sheet.data(), style_sheet.size()));
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
  return new_widget(new QMainWindow(), true);
}

Widget* widget_new_label(rust::Str text, Widget* parent)
{
  auto* label = new QLabel(parent != nullptr ? parent->q : nullptr);
  auto* widget = new_child(label, parent);
  auto* state = new TextState(label);
  widget->text_state = state;
  QObject::connect(state, &TextState::textChanged, label, [state, label] {
    label->setText(state->getText());
  });
  state->setText(QString::fromUtf8(text.data(), text.size()));
  return widget;
}

Widget* widget_new_button(rust::Str text, Widget* parent)
{
  return new_child(new QPushButton(QString::fromUtf8(text.data(), text.size())), parent);
}

Widget* widget_new_line_edit(rust::Str text, Widget* parent)
{
  auto* edit = new QLineEdit(parent != nullptr ? parent->q : nullptr);
  auto* widget = new_child(edit, parent);
  auto* state = new TextState(edit);
  widget->text_state = state;
  QObject::connect(state, &TextState::textChanged, edit, [state, edit] {
    edit->setText(state->getText());
  });
  QObject::connect(edit, &QLineEdit::textChanged, state, [state](const QString& value) {
    state->setText(value);
  });
  state->setText(QString::fromUtf8(text.data(), text.size()));
  return widget;
}

Widget* widget_new_datetime_edit(rust::Str iso_datetime, Widget* parent)
{
  auto* edit = new QDateTimeEdit(parent != nullptr ? parent->q : nullptr);
  edit->setCalendarPopup(true);
  edit->setDisplayFormat("ddd, d MMM yyyy  HH:mm");
  const auto value = QDateTime::fromString(QString::fromUtf8(iso_datetime.data(), iso_datetime.size()), Qt::ISODate);
  edit->setDateTime(value.isValid() ? value : QDateTime::currentDateTime());
  auto* widget = new_child(edit, parent);
  widget->value_kind = 5;
  return widget;
}

Widget* widget_new_date_edit(rust::Str iso_date, Widget* parent)
{
  auto* edit = new QDateEdit(parent != nullptr ? parent->q : nullptr);
  edit->setCalendarPopup(true);
  edit->setDisplayFormat("ddd, d MMM yyyy");
  const auto value = QDate::fromString(QString::fromUtf8(iso_date.data(), iso_date.size()), Qt::ISODate);
  edit->setDate(value.isValid() ? value : QDate::currentDate());
  auto* widget = new_child(edit, parent);
  widget->value_kind = 6;
  return widget;
}

Widget* widget_new_time_edit(rust::Str iso_time, Widget* parent)
{
  auto* edit = new QTimeEdit(parent != nullptr ? parent->q : nullptr);
  edit->setDisplayFormat("HH:mm");
  edit->setWrapping(true);
  const auto value = QTime::fromString(QString::fromUtf8(iso_time.data(), iso_time.size()), "HH:mm");
  edit->setTime(value.isValid() ? value : QTime::currentTime());
  auto* widget = new_child(edit, parent);
  widget->value_kind = 7;
  return widget;
}

Widget* widget_new_disk_map(Widget* parent)
{
  return new_child(new DiskMapWidget(parent != nullptr ? parent->q : nullptr), parent);
}

Widget* widget_new_checkbox(rust::Str text, Widget* parent)
{
  auto* checkbox = new QCheckBox(QString::fromUtf8(text.data(), text.size()),
                                 parent != nullptr ? parent->q : nullptr);
  auto* widget = new_child(checkbox, parent);
  auto* state = new ToggleState(checkbox);
  widget->toggle_state = state;
  QObject::connect(state, &ToggleState::checkedChanged, checkbox, [state, checkbox] {
    checkbox->setChecked(state->getChecked());
  });
  QObject::connect(checkbox, &QCheckBox::toggled, state, [state](bool checked) {
    state->setChecked(checked);
  });
  return widget;
}

Widget* widget_new_combo(rust::Vec<rust::String> items, Widget* parent)
{
  auto* combo = new QComboBox(parent != nullptr ? parent->q : nullptr);
  auto* widget = new_child(combo, parent);
  widget->value_kind = 1;
  for (const auto& item : items) {
    combo->addItem(QString::fromUtf8(item.data(), item.size()));
  }
  return widget;
}

Widget* widget_new_spin_box(int min, int max, int value, Widget* parent)
{
  auto* spin = new QSpinBox(parent != nullptr ? parent->q : nullptr);
  spin->setRange(min, max);
  spin->setValue(value);
  auto* widget = new_child(spin, parent);
  widget->value_kind = 2;
  return widget;
}

Widget* widget_new_slider(int min, int max, int value, Widget* parent)
{
  auto* slider = new QSlider(Qt::Horizontal, parent != nullptr ? parent->q : nullptr);
  slider->setRange(min, max);
  slider->setValue(value);
  auto* widget = new_child(slider, parent);
  widget->value_kind = 3;
  return widget;
}

Widget* widget_new_progress_bar(int min, int max, int value, Widget* parent)
{
  auto* bar = new QProgressBar(parent != nullptr ? parent->q : nullptr);
  bar->setRange(min, max);
  bar->setValue(value);
  auto* widget = new_child(bar, parent);
  widget->value_kind = 4;
  return widget;
}

Widget* widget_new_tab_widget(Widget* parent)
{
  return new_child(new QTabWidget(parent != nullptr ? parent->q : nullptr), parent);
}

Widget* tab_widget_add_page(Widget* tabs, rust::Str label)
{
  // `addTab` reparents `page` into the QTabWidget's internal stack. Wrap it
  // WITHOUT calling setParent again: re-parenting the page back onto the tab
  // widget would yank it out of the tab stack and hide it.
  auto* page = new QWidget();
  auto* layout = new QVBoxLayout(page);
  layout->setContentsMargins(6, 6, 6, 6);
  static_cast<QTabWidget*>(tabs->q)->addTab(
    page, QString::fromUtf8(label.data(), label.size()));
  return new_widget(page, false);
}

void tab_widget_set_current(Widget* tabs, int index)
{
  if (tabs->alive && tabs->q != nullptr) {
    static_cast<QTabWidget*>(tabs->q)->setCurrentIndex(index);
  }
}

int tab_widget_count(Widget* tabs)
{
  if (tabs->alive && tabs->q != nullptr) {
    return static_cast<QTabWidget*>(tabs->q)->count();
  }
  return 0;
}

Widget* widget_new_line_chart(Widget* parent)
{
  return new_child(new LineChartWidget(parent != nullptr ? parent->q : nullptr), parent);
}

Widget* widget_new_row(Widget* parent)
{
  auto* q = new QWidget();
  new QHBoxLayout(q);
  if (auto* mw = qobject_cast<QMainWindow*>(parent->q)) {
    if (mw->centralWidget() == nullptr) {
      mw->setCentralWidget(q);
      return new_widget(q, false);
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
      return new_widget(q, false);
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
      return new_widget(q, false);
    }
  }
  return new_child(q, parent);
}

Widget* widget_new_menubar(Widget* window)
{
  auto* q = new QMenuBar(window->q);
  static_cast<QMainWindow*>(window->q)->setMenuBar(q);
  return new_widget(q, false);
}

Widget* widget_new_toolbar(Widget* window)
{
  auto* q = new QToolBar(window->q);
  static_cast<QMainWindow*>(window->q)->addToolBar(q);
  return new_widget(q, false);
}

Widget* menu_new(rust::Str title, Widget* menubar)
{
  // QMenuBar owns menus created through this overload and configures them as
  // popup menus.  Constructing a QMenu as an ordinary QWidget child prevents
  // Wayland from granting the popup mouse grab required to open it.
  auto* q = static_cast<QMenuBar*>(menubar->q)->addMenu(
    QString::fromUtf8(title.data(), title.size()));
  return new_widget(q, false);
}

Widget* window_menu_new(rust::Str title, Widget* window)
{
  auto* q = new QMenu(QString::fromUtf8(title.data(), title.size()), window->q);
  return new_widget(q, false);
}

void menu_popup(Widget* menu)
{
  if (menu->alive && menu->q != nullptr) {
    static_cast<QMenu*>(menu->q)->popup(QCursor::pos());
  }
}

Widget* menu_add_submenu(Widget* menu, rust::Str title)
{
  auto* submenu = static_cast<QMenu*>(menu->q)->addMenu(
    QString::fromUtf8(title.data(), title.size()));
  return new_widget(submenu, false);
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
  QObject::disconnect(w->value_connection);
  QObject::disconnect(w->selection_connection);
  QObject::disconnect(w->header_connection);
  w->destroyed_cb_data = nullptr;
  w->clicked_cb_data = nullptr;
  w->text_changed_cb_data = nullptr;
  w->toggled_cb_data = nullptr;
  w->value_cb_data = nullptr;
  w->selection_cb_data = nullptr;
  w->header_cb_data = nullptr;
  w->context_cb_data = nullptr;
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
  if (w->alive && w->state != nullptr) {
    w->state->setVisible(true);
  }
}

void widget_set_visible(Widget* w, bool visible)
{
  if (w->alive && w->q != nullptr) {
    // A generated property's initial value can already equal `visible`, in
    // which case Qt emits no change signal.  Apply the native property too so
    // `set_visible(false)` reliably hides a freshly-created widget.
    w->q->setVisible(visible);
  }
  if (w->alive && w->state != nullptr) {
    w->state->setVisible(visible);
  }
}

bool widget_is_visible(Widget* w)
{
  return w->alive && w->q != nullptr && w->q->isVisible();
}

int widget_width(Widget* w)
{
  return w->alive && w->q != nullptr ? w->q->width() : 0;
}

int widget_height(Widget* w)
{
  return w->alive && w->q != nullptr ? w->q->height() : 0;
}

void widget_set_enabled(Widget* w, bool enabled)
{
  if (w->alive && w->state != nullptr) {
    w->state->setEnabled(enabled);
  }
}

void widget_set_title(Widget* w, rust::Str title)
{
  if (w->alive && w->state != nullptr) {
    w->state->setTitle(QString::fromUtf8(title.data(), title.size()));
  }
}

void widget_set_style_class(Widget* w, rust::Str style_class)
{
  if (w != nullptr && w->alive && w->q != nullptr) {
    w->q->setProperty("yseClass", QString::fromUtf8(style_class.data(), style_class.size()));
    w->q->style()->unpolish(w->q);
    w->q->style()->polish(w->q);
    w->q->update();
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
    auto* layout = qobject_cast<QGridLayout*>(grid->q->layout());
    child->q->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    layout->setColumnStretch(column, 1);
    layout->setRowStretch(row, 1);
    layout->addWidget(child->q, row, column);
    grid->q->updateGeometry();
  }
}

void label_set_text(Widget* w, rust::Str text)
{
  if (w->alive && w->text_state != nullptr) {
    w->text_state->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

rust::String label_text(Widget* w)
{
  if (w->alive && w->text_state != nullptr) {
    return rust::String(w->text_state->getText().toUtf8().constData());
  }
  return rust::String();
}

void line_edit_set_text(Widget* w, rust::Str text)
{
  if (w->alive && w->text_state != nullptr) {
    w->text_state->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

rust::String line_edit_text(Widget* w)
{
  if (w->alive && w->text_state != nullptr) {
    return rust::String(w->text_state->getText().toUtf8().constData());
  }
  return rust::String();
}

void datetime_edit_set_value(Widget* w, rust::Str iso_datetime)
{
  if (w->alive && w->q != nullptr) {
    const auto value = QDateTime::fromString(QString::fromUtf8(iso_datetime.data(), iso_datetime.size()), Qt::ISODate);
    if (value.isValid()) static_cast<QDateTimeEdit*>(w->q)->setDateTime(value);
  }
}

rust::String datetime_edit_value(Widget* w)
{
  return rust::String(static_cast<QDateTimeEdit*>(w->q)->dateTime().toString(Qt::ISODate).toUtf8().constData());
}

void date_edit_set_value(Widget* w, rust::Str iso_date)
{
  if (w->alive && w->q != nullptr) {
    const auto value = QDate::fromString(QString::fromUtf8(iso_date.data(), iso_date.size()), Qt::ISODate);
    if (value.isValid()) static_cast<QDateEdit*>(w->q)->setDate(value);
  }
}

rust::String date_edit_value(Widget* w)
{
  return rust::String(static_cast<QDateEdit*>(w->q)->date().toString(Qt::ISODate).toUtf8().constData());
}

void time_edit_set_value(Widget* w, rust::Str iso_time)
{
  if (w->alive && w->q != nullptr) {
    const auto value = QTime::fromString(QString::fromUtf8(iso_time.data(), iso_time.size()), "HH:mm");
    if (value.isValid()) static_cast<QTimeEdit*>(w->q)->setTime(value);
  }
}

rust::String time_edit_value(Widget* w)
{
  return rust::String(static_cast<QTimeEdit*>(w->q)->time().toString("HH:mm").toUtf8().constData());
}

void disk_map_set_segments(Widget* w, rust::Vec<rust::String> labels, rust::Vec<double> shares)
{
  if (w->alive && w->q != nullptr) {
    static_cast<DiskMapWidget*>(w->q)->setSegments(std::move(labels), std::move(shares));
  }
}

void line_chart_set_series(Widget* w, rust::Vec<double> points)
{
  if (w->alive && w->q != nullptr) {
    static_cast<LineChartWidget*>(w->q)->setSeries(std::move(points));
  }
}

void line_chart_set_series_multi(Widget* w, rust::Vec<double> points, size_t series)
{
  if (w->alive && w->q != nullptr) {
    static_cast<LineChartWidget*>(w->q)->setSeriesMulti(std::move(points), series);
  }
}

void button_set_icon(Widget* w, rust::Str theme_name)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QPushButton*>(w->q)->setIcon(
      QIcon::fromTheme(QString::fromUtf8(theme_name.data(), theme_name.size())));
  }
}

void checkbox_set_checked(Widget* w, bool checked)
{
  if (w->alive && w->toggle_state != nullptr) {
    w->toggle_state->setChecked(checked);
  }
}

bool checkbox_checked(Widget* w)
{
  return w->alive && w->toggle_state != nullptr && w->toggle_state->getChecked();
}

void combo_set_items(Widget* w, rust::Vec<rust::String> items)
{
  if (w->alive && w->q != nullptr && w->value_kind == 1) {
    auto* combo = static_cast<QComboBox*>(w->q);
    combo->clear();
    for (const auto& item : items) {
      combo->addItem(QString::fromUtf8(item.data(), item.size()));
    }
  }
}

rust::String combo_current_text(Widget* w)
{
  if (w->alive && w->q != nullptr && w->value_kind == 1) {
    return rust::String(static_cast<QComboBox*>(w->q)->currentText().toUtf8().constData());
  }
  return rust::String();
}

void combo_set_current_text(Widget* w, rust::Str text)
{
  if (w->alive && w->q != nullptr && w->value_kind == 1) {
    auto* combo = static_cast<QComboBox*>(w->q);
    const int index = combo->findText(QString::fromUtf8(text.data(), text.size()));
    if (index >= 0) {
      combo->setCurrentIndex(index);
    }
  }
}

int widget_value(Widget* w)
{
  if (w == nullptr || !w->alive || w->q == nullptr) {
    return 0;
  }
  switch (w->value_kind) {
    case 1:
      return static_cast<QComboBox*>(w->q)->currentIndex();
    case 2:
      return static_cast<QSpinBox*>(w->q)->value();
    case 3:
      return static_cast<QSlider*>(w->q)->value();
    case 4:
      return static_cast<QProgressBar*>(w->q)->value();
    default:
      return 0;
  }
}

void widget_set_value(Widget* w, int value)
{
  if (w == nullptr || !w->alive || w->q == nullptr) {
    return;
  }
  switch (w->value_kind) {
    case 1:
      static_cast<QComboBox*>(w->q)->setCurrentIndex(value);
      break;
    case 2:
      static_cast<QSpinBox*>(w->q)->setValue(value);
      break;
    case 3:
      static_cast<QSlider*>(w->q)->setValue(value);
      break;
    case 4:
      static_cast<QProgressBar*>(w->q)->setValue(value);
      break;
    default:
      break;
  }
}

void widget_set_value_changed_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->value_connection);
  w->value_cb_data = static_cast<void*>(data);
  const auto fire = [w] {
    if (w->value_cb_data != nullptr) {
      yse_ui::on_value_changed(static_cast<yse_ui::Void*>(w->value_cb_data));
    }
  };
  switch (w->value_kind) {
    case 1:
      w->value_connection = QObject::connect(
        static_cast<QComboBox*>(w->q),
        QOverload<int>::of(&QComboBox::currentIndexChanged),
        w->q,
        fire);
      break;
    case 2:
      w->value_connection = QObject::connect(
        static_cast<QSpinBox*>(w->q),
        QOverload<int>::of(&QSpinBox::valueChanged),
        w->q,
        fire);
      break;
    case 3:
      w->value_connection = QObject::connect(
        static_cast<QSlider*>(w->q),
        &QSlider::valueChanged,
        w->q,
        fire);
      break;
    default:
      break;
  }
}

void widget_set_date_value_changed_cb(Widget* w, Void* data)
{
  QObject::disconnect(w->value_connection);
  w->value_cb_data = static_cast<void*>(data);
  const auto fire = [w] {
    if (w->value_cb_data != nullptr) {
      yse_ui::on_date_value_changed(static_cast<yse_ui::Void*>(w->value_cb_data));
    }
  };
  switch (w->value_kind) {
    case 5:
      w->value_connection = QObject::connect(
        static_cast<QDateTimeEdit*>(w->q),
        &QDateTimeEdit::dateTimeChanged,
        w->q,
        fire);
      break;
    case 6:
      w->value_connection = QObject::connect(
        static_cast<QDateEdit*>(w->q),
        &QDateEdit::dateChanged,
        w->q,
        fire);
      break;
    case 7:
      w->value_connection = QObject::connect(
        static_cast<QTimeEdit*>(w->q),
        &QTimeEdit::timeChanged,
        w->q,
        fire);
      break;
    default:
      break;
  }
}

rust::String widget_value_text(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    switch (w->value_kind) {
      case 5:
        return rust::String(
          static_cast<QDateTimeEdit*>(w->q)->dateTime().toString(Qt::ISODate).toUtf8().constData());
      case 6:
        return rust::String(
          static_cast<QDateEdit*>(w->q)->date().toString(Qt::ISODate).toUtf8().constData());
      case 7:
        return rust::String(
          static_cast<QTimeEdit*>(w->q)->time().toString("HH:mm").toUtf8().constData());
      default:
        break;
    }
  }
  return rust::String();
}

void widget_set_value_text(Widget* w, rust::Str text)
{
  if (w->alive && w->q != nullptr) {
    const QString value = QString::fromUtf8(text.data(), text.size());
    switch (w->value_kind) {
      case 5:
        static_cast<QDateTimeEdit*>(w->q)->setDateTime(
          QDateTime::fromString(value, Qt::ISODate));
        break;
      case 6:
        static_cast<QDateEdit*>(w->q)->setDate(QDate::fromString(value, Qt::ISODate));
        break;
      case 7:
        static_cast<QTimeEdit*>(w->q)->setTime(QTime::fromString(value, "HH:mm"));
        break;
      default:
        break;
    }
  }
}

void spin_box_set_range(Widget* w, int min, int max)
{
  if (w->alive && w->q != nullptr && w->value_kind == 2) {
    static_cast<QSpinBox*>(w->q)->setRange(min, max);
  }
}

void slider_set_range(Widget* w, int min, int max)
{
  if (w->alive && w->q != nullptr && w->value_kind == 3) {
    static_cast<QSlider*>(w->q)->setRange(min, max);
  }
}

void progress_set_range(Widget* w, int min, int max)
{
  if (w->alive && w->q != nullptr && w->value_kind == 4) {
    static_cast<QProgressBar*>(w->q)->setRange(min, max);
  }
}

void button_click(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QPushButton*>(w->q)->click();
  }
}

void button_set_text(Widget* w, rust::Str text)
{
  if (w->alive && w->q != nullptr) {
    static_cast<QPushButton*>(w->q)->setText(
      QString::fromUtf8(text.data(), text.size()));
  }
}

rust::String button_text(Widget* w)
{
  if (w->alive && w->q != nullptr) {
    return rust::String(static_cast<QPushButton*>(w->q)->text().toUtf8().constData());
  }
  return rust::String();
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
      QPointer<QMessageBox> q = d->q;
      d->alive = false;
      d->cb_data = nullptr;
      if (data != nullptr) {
        yse_ui::on_dialog_finished(data);
      }
      if (q != nullptr) {
        q->deleteLater();
      }
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

FileDialog* filedialog_directory_new(Widget* parent, rust::Str title)
{
  auto* q = new QFileDialog(parent != nullptr ? parent->q : nullptr);
  q->setWindowTitle(QString::fromUtf8(title.data(), title.size()));
  q->setFileMode(QFileDialog::Directory);
  q->setOption(QFileDialog::ShowDirsOnly, true);
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

int filedialog_last_result(FileDialog* d)
{
  return d != nullptr ? d->last_result : 0;
}

void filedialog_set_finished_cb(FileDialog* d, Void* data)
{
  QObject::disconnect(d->finished_connection);
    d->cb_data = static_cast<void*>(data);
    d->finished_connection = QObject::connect(
    d->q, &QFileDialog::finished, d->q, [d](int result) {
      auto* data = static_cast<yse_ui::Void*>(d->cb_data);
      QPointer<QFileDialog> q = d->q;
      d->last_result = result;
      d->alive = false;
      d->cb_data = nullptr;
      if (data != nullptr) {
        yse_ui::on_filedialog_finished(data);
      }
      if (q != nullptr) {
        q->deleteLater();
      }
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
  auto* q = new QAction(parent != nullptr ? parent->q : nullptr);
  auto* action = new Action{ q, true, nullptr };
  auto* state = new ActionState(q);
  action->state = state;
  QObject::connect(state, &ActionState::textChanged, q, [state, q] {
    q->setText(state->getText());
  });
  QObject::connect(state, &ActionState::enabledChanged, q, [state, q] {
    q->setEnabled(state->getEnabled());
  });
  state->setText(QString::fromUtf8(text.data(), text.size()));
  return action;
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
  if (a->alive && a->state != nullptr) {
    a->state->setText(QString::fromUtf8(text.data(), text.size()));
  }
}

void action_set_icon(Action* a, rust::Str theme_name)
{
  if (a->alive && a->q != nullptr) {
    a->q->setIcon(QIcon::fromTheme(QString::fromUtf8(theme_name.data(), theme_name.size())));
  }
}

void action_set_enabled(Action* a, bool enabled)
{
  if (a->alive && a->state != nullptr) {
    a->state->setEnabled(enabled);
  }
}

void action_set_shortcut(Action* a, rust::Str shortcut)
{
  if (a->alive && a->q != nullptr) {
    a->q->setShortcut(
      QKeySequence(QString::fromUtf8(shortcut.data(), shortcut.size())));
  }
}

void action_set_checkable(Action* a, bool checkable)
{
  if (a->alive && a->q != nullptr) {
    a->q->setCheckable(checkable);
  }
}

void action_set_checked(Action* a, bool checked)
{
  if (a->alive && a->q != nullptr) {
    a->q->setChecked(checked);
  }
}

bool action_checked(Action* a)
{
  return a->alive && a->q != nullptr && a->q->isChecked();
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
