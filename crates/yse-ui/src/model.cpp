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
#include <QScrollBar>
#include <QSet>
#include <QStyledItemDelegate>
#include <functional>
#include <QtWidgets/QAbstractItemView>
#include <QtWidgets/QListView>
#include <QtWidgets/QTableView>
#include <QtWidgets/QTreeView>

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
  ContextMenuFilter(QAbstractItemView* view, Widget* widget, QObject* parent)
    : QObject(parent), view_(view), widget_(widget) {}

protected:
  bool eventFilter(QObject* watched, QEvent* event) override {
    if (event->type() == QEvent::ContextMenu) {
      auto* context = static_cast<QContextMenuEvent*>(event);
      const QModelIndex index = view_->indexAt(context->pos());
      if (index.isValid()) {
        if (widget_->alive && widget_->context_cb_data != nullptr) {
          const int row = qobject_cast<QTreeView*>(view_) != nullptr
                            ? static_cast<int>(index.internalId())
                            : index.row();
          yse_ui::on_view_context_menu(
            static_cast<yse_ui::Void*>(widget_->context_cb_data), row);
        }
      }
      return true;
    }
    return QObject::eventFilter(watched, event);
  }

private:
  QAbstractItemView* view_;
  Widget* widget_;
};

Model* model_new()
{
  return new Model{ new RustListModel(), true, nullptr };
}

TreeModel* tree_model_new(int columns)
{
  return new TreeModel{ new RustTreeModel(columns), true, nullptr };
}

void tree_model_drop(TreeModel* m)
{
  if (m == nullptr) {
    return;
  }
  m->alive = false;
  delete m->q;
  delete m;
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

void table_model_set_column_alignment(TableModel* m, int column, int alignment)
{
  if (m->alive) {
    m->q->rustSetColumnAlignment(column, alignment);
  }
}

void tree_model_reset(TreeModel* m, rust::Vec<yse_ui::TreeRow> rows)
{
  if (!m->alive) {
    return;
  }
  static QFileIconProvider provider;
  QVector<RustTreeRow> native;
  native.reserve(rows.size());
  for (const auto& row : rows) {
    const auto cells = to_qstrings(row.cells);
    QStringList cellList;
    for (const QString& cell : cells) {
      cellList.append(cell);
    }
    native.append(RustTreeRow{
      row.parent,
      cellList,
      row.icon.empty() ? QIcon() : provider.icon(QFileInfo(QString::fromUtf8(row.icon.data(), row.icon.size()))),
    });
  }
  m->q->rustTreeReset(native);
}

void tree_model_set_headers(TreeModel* m, rust::Vec<rust::String> headers)
{
  if (m->alive) {
    m->q->rustTreeSetHeaders(to_qstrings(headers));
  }
}

void tree_model_set_heat(TreeModel* m, int column, rust::Vec<double> values)
{
  if (!m->alive) {
    return;
  }
  QVector<qreal> heat;
  heat.reserve(values.size());
  for (double value : values) {
    heat.append(static_cast<qreal>(value));
  }
  m->q->rustTreeSetHeat(column, heat);
}

void tree_model_set_column_alignment(TreeModel* m, int column, int alignment)
{
  if (m->alive) {
    m->q->rustTreeSetColumnAlignment(column, alignment);
  }
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

Widget* widget_new_tree_view(TreeModel* m, Widget* parent)
{
  auto* q = new QTreeView(parent != nullptr ? parent->q : nullptr);
  q->setModel(m->q);
  if (q->selectionModel() == nullptr) {
    q->setSelectionModel(new QItemSelectionModel(m->q));
  }
  q->setUniformRowHeights(true);
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

void view_set_heat_delegate(Widget* view)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QAbstractItemView*>(view->q)->setItemDelegate(
      new HeatDelegate(static_cast<QAbstractItemView*>(view->q)));
  }
}

void view_set_sort_indicator(Widget* view, int column, bool ascending)
{
  if (view->alive && view->q != nullptr) {
    QHeaderView* header = nullptr;
    if (auto* table = qobject_cast<QTableView*>(view->q)) {
      header = table->horizontalHeader();
    } else if (auto* tree = qobject_cast<QTreeView*>(view->q)) {
      header = tree->header();
    }
    if (header != nullptr) {
      header->setSortIndicatorShown(true);
      header->setSortIndicator(column, ascending ? Qt::AscendingOrder : Qt::DescendingOrder);
    }
  }
}

void view_set_column_hidden(Widget* view, int column, bool hidden)
{
  if (view->alive && view->q != nullptr) {
    QHeaderView* header = nullptr;
    if (auto* table = qobject_cast<QTableView*>(view->q)) {
      header = table->horizontalHeader();
    } else if (auto* tree = qobject_cast<QTreeView*>(view->q)) {
      header = tree->header();
    }
    if (header != nullptr) {
      header->setSectionHidden(column, hidden);
    }
  }
}

void tree_view_expand_all(Widget* view, bool expand)
{
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    if (expand) {
      tree->expandAll();
    } else {
      tree->collapseAll();
    }
  }
}

void tree_view_set_column_width(Widget* view, int column, int width)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTreeView*>(view->q)->setColumnWidth(column, width);
  }
}

void tree_view_stretch_last_section(Widget* view, bool stretch)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTreeView*>(view->q)->header()->setStretchLastSection(stretch);
  }
}

void tree_view_set_header_clicked_cb(Widget* view, Void* data)
{
  QObject::disconnect(view->header_connection);
  view->header_cb_data = static_cast<void*>(data);
  auto* tree = static_cast<QTreeView*>(view->q);
  view->header_connection = QObject::connect(
    tree->header(),
    &QHeaderView::sectionClicked,
    tree,
    [view](int section) {
      if (view->header_cb_data != nullptr) {
        yse_ui::on_header_clicked(
          static_cast<yse_ui::Void*>(view->header_cb_data), section);
      }
    });
}

void tree_view_click_header(Widget* view, int section)
{
  if (view->alive && view->q != nullptr) {
    QMetaObject::invokeMethod(
      static_cast<QTreeView*>(view->q)->header(),
      "sectionClicked",
      Q_ARG(int, section));
  }
}

void tree_view_select_flat(Widget* view, int flat)
{
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    auto* model = static_cast<RustTreeModel*>(tree->model());
    const QModelIndex target = model->rustIndexFromFlat(flat);
    if (target.isValid()) {
      tree->selectionModel()->select(target, QItemSelectionModel::ClearAndSelect);
    }
  }
}

rust::Vec<int32_t> tree_view_expanded_rows(Widget* view)
{
  rust::Vec<int32_t> rows;
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    auto* model = static_cast<RustTreeModel*>(tree->model());
    std::function<void(int)> collect = [&](int flat) {
      const QModelIndex index = model->rustIndexFromFlat(flat);
      if (!index.isValid()) {
        return;
      }
      if (tree->isExpanded(index)) {
        rows.push_back(flat);
      }
      for (int row = 0; row < model->rowCount(index); ++row) {
        const QModelIndex child = model->index(row, 0, index);
        collect(static_cast<int>(child.internalId()));
      }
    };
    for (int row = 0; row < model->rowCount(QModelIndex()); ++row) {
      collect(static_cast<int>(model->index(row, 0, QModelIndex()).internalId()));
    }
  }
  return rows;
}

void tree_view_expand_rows(Widget* view, rust::Vec<int32_t> rows)
{
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    auto* model = static_cast<RustTreeModel*>(tree->model());
    for (int32_t flat : rows) {
      const QModelIndex index = model->rustIndexFromFlat(flat);
      if (index.isValid()) {
        tree->expand(index);
      }
    }
  }
}

int tree_view_top_row(Widget* view)
{
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    const QModelIndex top = tree->indexAt(QPoint(4, 4));
    if (top.isValid()) {
      return static_cast<int>(top.internalId());
    }
  }
  return -1;
}

void tree_view_scroll_to_flat(Widget* view, int flat)
{
  if (view->alive && view->q != nullptr) {
    auto* tree = static_cast<QTreeView*>(view->q);
    auto* model = static_cast<RustTreeModel*>(tree->model());
    const QModelIndex index = model->rustIndexFromFlat(flat);
    if (index.isValid()) {
      tree->scrollTo(index, QAbstractItemView::PositionAtTop);
    }
  }
}

int tree_view_scroll_value(Widget* view)
{
  if (view->alive && view->q != nullptr) {
    return static_cast<QTreeView*>(view->q)->verticalScrollBar()->value();
  }
  return 0;
}

void tree_view_set_scroll_value(Widget* view, int value)
{
  if (view->alive && view->q != nullptr) {
    static_cast<QTreeView*>(view->q)->verticalScrollBar()->setValue(value);
  }
}

int tree_model_flat_row_count(TreeModel* m)
{
  return m->alive ? m->q->rustFlatRowCount() : 0;
}

rust::Vec<int32_t> view_selected_rows(Widget* view)
{
  rust::Vec<int32_t> rows;
  if (view->alive && view->q != nullptr) {
    // For trees, report flat model rows; for flat views, report deduped
    // row() values (selectedRows() can be empty before layout).
    const auto indexes =
      static_cast<QAbstractItemView*>(view->q)->selectionModel()->selectedIndexes();
    QSet<int> seen;
    const bool isTree = qobject_cast<QTreeView*>(view->q) != nullptr;
    rows.reserve(indexes.size());
    for (const auto& index : indexes) {
      const int row = isTree ? static_cast<int>(index.internalId()) : index.row();
      if (!seen.contains(row)) {
        seen.insert(row);
        rows.push_back(row);
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
