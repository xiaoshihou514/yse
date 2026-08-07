// Implementation of the RustListModel adapter and model-facing shim
// functions.

#include "yse-ui/src/model.h"

#include <QContextMenuEvent>
#include <QItemSelectionModel>
#include <QHeaderView>
#include <QFileIconProvider>
#include <QFileInfo>
#include <QMenu>
#include <QPainter>
#include <QSet>
#include <QStyledItemDelegate>
#include <QtWidgets/QAbstractItemView>
#include <QtWidgets/QListView>
#include <QtWidgets/QTableView>

#include "yse-ui/src/bridge.cxx.h"
#include "yse-ui/src/widgets.h"

namespace yse_ui {

// Paints a translucent accent wash over cells whose model exposes a positive
// `kHeatRole` value (normalized 0..1), producing the resource heat-map look.
class HeatDelegate final : public QStyledItemDelegate {
public:
  using QStyledItemDelegate::QStyledItemDelegate;

  void paint(QPainter* painter, const QStyleOptionViewItem& option,
             const QModelIndex& index) const override {
    QStyledItemDelegate::paint(painter, option, index);
    const QVariant heat = index.data(kHeatRole);
    if (!heat.isValid()) {
      return;
    }
    const double intensity = qBound(0.0, heat.toDouble(), 1.0);
    if (intensity <= 0.0) {
      return;
    }
    QColor accent = option.palette.color(QPalette::Highlight);
    accent.setAlphaF(0.18 * intensity);
    painter->fillRect(option.rect.adjusted(2, 2, -2, -2), accent);
  }
};

// Reports the row under the cursor on right-click back to Rust, where the
// application composes its own context menu. Owned by the view (parented
// QObject), so it dies with the widget.
class ContextMenuFilter final : public QObject {
public:
  ContextMenuFilter(QTableView* view, Widget* widget, QObject* parent)
    : QObject(parent), view_(view), widget_(widget) {}

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (event->type() == QEvent::ContextMenu) {
      auto* context = static_cast<QContextMenuEvent*>(event);
      const QModelIndex index = view_->indexAt(context->pos());
      if (index.isValid()) {
        if (widget_->alive && widget_->context_cb_data != nullptr) {
          yse_ui::on_view_context_menu(
            static_cast<yse_ui::Void*>(widget_->context_cb_data), index.row());
        }
      }
      return true;
    }
    return QObject::eventFilter(watched, event);
  }

private:
  QTableView* view_;
  Widget* widget_;
};

Model* model_new()
{
  return new Model{ new RustListModel(), true, nullptr };
}

void model_drop(Model* m)
{
  if (m == nullptr) {
    return;
  }
  m->alive = false;
  delete m->q;
  delete m;
}

static QVector<QString> to_qstrings(const rust::Vec<rust::String>& items)
{
  QVector<QString> result;
  result.reserve(items.size());
  for (const rust::String& item : items) {
    result.append(QString::fromUtf8(item.data(), item.size()));
  }
  return result;
}

static QVector<QStringList> to_qstringlists(const rust::Vec<yse_ui::Row>& rows)
{
  QVector<QStringList> result;
  result.reserve(rows.size());
  for (const yse_ui::Row& row : rows) {
    result.append(to_qstrings(row.cells));
  }
  return result;
}

void model_insert_rows(Model* m, int row, rust::Vec<rust::String> items)
{
  if (m->alive) {
    m->q->rustInsert(row, to_qstrings(items));
  }
}

void model_remove_rows(Model* m, int row, int count)
{
  if (m->alive) {
    m->q->rustRemove(row, count);
  }
}

void model_update_rows(Model* m, int row, rust::Vec<rust::String> items)
{
  if (m->alive) {
    m->q->rustUpdate(row, to_qstrings(items));
  }
}

void model_reset(Model* m, rust::Vec<rust::String> items)
{
  if (m->alive) {
    m->q->rustReset(to_qstrings(items));
  }
}

int model_row_count(Model* m)
{
  return m->alive ? m->q->rustRowCount() : 0;
}

rust::String model_text(Model* m, int row)
{
  return rust::String(m->q->rustText(row).toUtf8().constData());
}

Widget* widget_new_list_view(Model* m, Widget* parent)
{
  auto* q = new QListView(parent->q);
  q->setModel(m->q);
  return widget_wrap_child(q, parent);
}

TableModel* table_model_new(int columns)
{
  return new TableModel{ new RustTableModel(columns), true, nullptr };
}

void table_model_drop(TableModel* m)
{
  if (m == nullptr) {
    return;
  }
  m->alive = false;
  delete m->q;
  delete m;
}

void table_set_headers(TableModel* m, rust::Vec<rust::String> headers)
{
  if (m->alive) {
    m->q->rustSetHeaders(to_qstrings(headers));
  }
}

void table_insert_rows(TableModel* m, int row, rust::Vec<yse_ui::Row> rows)
{
  if (m->alive) {
    m->q->rustInsert(row, to_qstringlists(rows));
  }
}

void table_remove_rows(TableModel* m, int row, int count)
{
  if (m->alive) {
    m->q->rustRemove(row, count);
  }
}

void table_update_rows(TableModel* m, int row, rust::Vec<yse_ui::Row> rows)
{
  if (m->alive) {
    m->q->rustUpdate(row, to_qstringlists(rows));
  }
}

void table_reset(TableModel* m, rust::Vec<yse_ui::Row> rows)
{
  if (m->alive) {
    m->q->rustReset(to_qstringlists(rows));
  }
}

void table_model_set_row_icons(TableModel* m, rust::Vec<rust::String> paths)
{
  if (!m->alive) {
    return;
  }
  QVector<QIcon> icons;
  icons.reserve(paths.size());
  static QFileIconProvider provider;
  for (const auto& path : paths) {
    const QString file = QString::fromUtf8(path.data(), path.size());
    icons.append(file.isEmpty()
                   ? QIcon()
                   : provider.icon(QFileInfo(file)));
  }
  m->q->rustSetRowIcons(icons);
}

void table_model_set_heat(TableModel* m, int column, rust::Vec<double> values)
{
  if (!m->alive) {
    return;
  }
  QVector<qreal> heat;
  heat.reserve(values.size());
  for (double value : values) {
    heat.append(static_cast<qreal>(value));
  }
  m->q->rustSetHeat(column, heat);
}

int table_model_row_icon_count(TableModel* m)
{
  return m->alive ? m->q->rustIconCount() : 0;
}

int table_row_count(TableModel* m)
{
  return m->alive ? m->q->rustRowCount() : 0;
}

int table_column_count(TableModel* m)
{
  return m->alive ? m->q->columnCount() : 0;
}

rust::String table_text(TableModel* m, int row, int column)
{
  return rust::String(m->q->rustText(row, column).toUtf8().constData());
}

Widget* widget_new_table_view(TableModel* m, Widget* parent)
{
  auto* q = new QTableView(parent->q);
  q->setModel(m->q);
  if (q->selectionModel() == nullptr) {
    q->setSelectionModel(new QItemSelectionModel(m->q));
  }
  return widget_wrap_child(q, parent);
}

void view_set_selection_cb(Widget* view, Void* data)
{
  QObject::disconnect(view->selection_connection);
  view->selection_cb_data = static_cast<void*>(data);
  auto* item_view = static_cast<QAbstractItemView*>(view->q);
  view->selection_connection = QObject::connect(
    item_view->selectionModel(),
    &QItemSelectionModel::selectionChanged,
    item_view,
    [view] {
      if (view->selection_cb_data != nullptr) {
        yse_ui::on_selection_changed(
          static_cast<yse_ui::Void*>(view->selection_cb_data));
      }
    });
}

void view_set_header_clicked_cb(Widget* view, Void* data)
{
  QObject::disconnect(view->header_connection);
  view->header_cb_data = static_cast<void*>(data);
  auto* table_view = static_cast<QTableView*>(view->q);
  view->header_connection = QObject::connect(
    table_view->horizontalHeader(),
    &QHeaderView::sectionClicked,
    table_view,
    [view](int section) {
      if (view->header_cb_data != nullptr) {
        yse_ui::on_header_clicked(
          static_cast<yse_ui::Void*>(view->header_cb_data), section);
      }
    });
}

void view_set_context_menu_cb(Widget* view, Void* data)
{
  view->context_cb_data = static_cast<void*>(data);
  auto* table_view = static_cast<QTableView*>(view->q);
  // The filter is parented to the view, so it is destroyed with the widget.
  table_view->installEventFilter(new ContextMenuFilter(table_view, view, table_view));
}

void view_click_header(Widget* view, int section)
{
  if (view->alive && view->q != nullptr) {
    QMetaObject::invokeMethod(
      static_cast<QTableView*>(view->q)->horizontalHeader(),
      "sectionClicked",
      Q_ARG(int, section));
  }
}

void view_emit_context_menu(Widget* view, int row)
{
  // Test/automation helper: report the row as if the context menu action had
  // been chosen, without blocking on QMenu::exec.
  if (view->alive && view->context_cb_data != nullptr) {
    yse_ui::on_view_context_menu(
      static_cast<yse_ui::Void*>(view->context_cb_data), row);
  }
}

void view_set_column_width(Widget* view, int column, int width)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTableView*>(view->q)->setColumnWidth(column, width);
  }
}

void view_stretch_last_section(Widget* view, bool stretch)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTableView*>(view->q)
      ->horizontalHeader()->setStretchLastSection(stretch);
  }
}

void view_set_select_rows(Widget* view, bool on)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QAbstractItemView*>(view->q)
      ->setSelectionBehavior(on ? QAbstractItemView::SelectRows
                                : QAbstractItemView::SelectItems);
  }
}

void view_set_alternating_row_colors(Widget* view, bool on)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTableView*>(view->q)->setAlternatingRowColors(on);
  }
}

void view_set_heat_delegate(Widget* view)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTableView*>(view->q)->setItemDelegate(
      new HeatDelegate(static_cast<QTableView*>(view->q)));
  }
}

rust::Vec<int32_t> view_selected_rows(Widget* view)
{
  rust::Vec<int32_t> rows;
  if (view->alive && view->q != nullptr) {
    // selectedRows() can be empty on a view that has never been laid out;
    // dedupe rows from selectedIndexes() instead.
    const auto indexes =
      static_cast<QAbstractItemView*>(view->q)->selectionModel()->selectedIndexes();
    QSet<int> seen;
    rows.reserve(indexes.size());
    for (const auto& index : indexes) {
      if (!seen.contains(index.row())) {
        seen.insert(index.row());
        rows.push_back(index.row());
      }
    }
  }
  return rows;
}

void view_select_row(Widget* view, int row)
{
  if (view->alive && view->q != nullptr) {
    auto* item_view = static_cast<QAbstractItemView*>(view->q);
    auto* model = item_view->model();
    if (model != nullptr && row >= 0 && row < model->rowCount()) {
      item_view->selectionModel()->select(
        model->index(row, 0), QItemSelectionModel::ClearAndSelect);
    }
  }
}

void view_clear_selection(Widget* view)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QAbstractItemView*>(view->q)->selectionModel()->clearSelection();
  }
}

} // namespace yse_ui
