// A minimal QAbstractListModel mirror whose rows live in a QVector<QString>.
// Rust drives it through beginInsertRows/beginRemoveRows/dataChanged so Qt
// views update incrementally instead of resetting.

#pragma once

#include <QAbstractListModel>
#include <QIcon>
#include <QStringList>
#include <QVector>

class RustListModel : public QAbstractListModel
{
public:
  using QAbstractListModel::QAbstractListModel;

  int rowCount(const QModelIndex& = QModelIndex()) const override
  {
    return rows_.size();
  }

  QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override
  {
    if (!index.isValid() || role != Qt::DisplayRole || index.row() >= rows_.size()) {
      return {};
    }
    return rows_.at(index.row());
  }

  void rustInsert(int row, const QVector<QString>& items)
  {
    if (items.isEmpty()) {
      return;
    }
    beginInsertRows({}, row, row + items.size() - 1);
    for (int i = 0; i < items.size(); ++i) {
      rows_.insert(row + i, items.at(i));
    }
    endInsertRows();
  }

  void rustRemove(int row, int count)
  {
    if (count <= 0 || row >= rows_.size()) {
      return;
    }
    const int end = qMin(row + count, rows_.size());
    beginRemoveRows({}, row, end - 1);
    rows_.remove(row, end - row);
    endRemoveRows();
  }

  void rustUpdate(int row, const QVector<QString>& items)
  {
    if (items.isEmpty() || row >= rows_.size()) {
      return;
    }
    for (int i = 0; i < items.size() && row + i < rows_.size(); ++i) {
      rows_[row + i] = items.at(i);
    }
    emit dataChanged(index(row, 0), index(row + items.size() - 1, 0), { Qt::DisplayRole });
  }

  void rustReset(const QVector<QString>& items)
  {
    beginResetModel();
    rows_ = items;
    endResetModel();
  }

  int rustRowCount() const
  {
    return rows_.size();
  }

  QString rustText(int row) const
  {
    return row >= 0 && row < rows_.size() ? rows_.at(row) : QString();
  }

private:
  QVector<QString> rows_;
};

/// A QAbstractTableModel mirror with a fixed column count, headers, and
/// string rows, driven incrementally from Rust.
class RustTableModel : public QAbstractTableModel
{
public:
  explicit RustTableModel(int columns)
    : columns_(columns)
  {
  }

  int rowCount(const QModelIndex& = QModelIndex()) const override
  {
    return rows_.size();
  }

  int columnCount(const QModelIndex& = QModelIndex()) const override
  {
    return columns_;
  }

  QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override
  {
    if (!index.isValid()) {
      return {};
    }
    if (index.row() >= rows_.size() || index.column() >= rows_.at(index.row()).size()) {
      return {};
    }
    if (role == Qt::DecorationRole && index.column() == 0
        && index.row() < icons_.size() && !icons_.at(index.row()).isNull()) {
      return icons_.at(index.row());
    }
    if (role != Qt::DisplayRole) {
      return {};
    }
    return rows_.at(index.row()).at(index.column());
  }

  QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override
  {
    if (role != Qt::DisplayRole) {
      return {};
    }
    if (orientation == Qt::Horizontal) {
      return section < headers_.size() ? headers_.at(section) : QString();
    }
    return QString::number(section + 1);
  }

  void rustSetHeaders(const QVector<QString>& headers)
  {
    beginResetModel();
    headers_ = headers;
    endResetModel();
  }

  void rustInsert(int row, const QVector<QStringList>& rows)
  {
    if (rows.isEmpty()) {
      return;
    }
    beginInsertRows({}, row, row + rows.size() - 1);
    for (int i = 0; i < rows.size(); ++i) {
      rows_.insert(row + i, rows.at(i));
      icons_.insert(row + i, QIcon());
    }
    endInsertRows();
  }

  void rustRemove(int row, int count)
  {
    if (count <= 0 || row >= rows_.size()) {
      return;
    }
    const int end = qMin(row + count, rows_.size());
    beginRemoveRows({}, row, end - 1);
    rows_.remove(row, end - row);
    icons_.remove(row, end - row);
    endRemoveRows();
  }

  void rustUpdate(int row, const QVector<QStringList>& rows)
  {
    if (rows.isEmpty() || row >= rows_.size()) {
      return;
    }
    for (int i = 0; i < rows.size() && row + i < rows_.size(); ++i) {
      rows_[row + i] = rows.at(i);
    }
    emit dataChanged(index(row, 0), index(row + rows.size() - 1, columns_ - 1), { Qt::DisplayRole });
  }

  void rustReset(const QVector<QStringList>& rows)
  {
    beginResetModel();
    rows_ = rows;
    // Keep icons aligned with the new row count instead of dropping them:
    // a deferred reset can land after a same-pass icon update (the table sync
    // runs in the follow-up propagation pass), and clearing here would erase
    // icons that were just set for the new rows.
    icons_.resize(rows_.size());
    endResetModel();
  }

  void rustSetRowIcons(const QVector<QIcon>& icons)
  {
    icons_ = icons;
    if (!rows_.isEmpty()) {
      emit dataChanged(index(0, 0), index(rows_.size() - 1, 0), { Qt::DecorationRole });
    }
  }

  int rustRowCount() const
  {
    return rows_.size();
  }

  int rustIconCount() const
  {
    int count = 0;
    for (const QIcon& icon : icons_) {
      if (!icon.isNull()) {
        ++count;
      }
    }
    return count;
  }

  QString rustText(int row, int column) const
  {
    if (row < 0 || row >= rows_.size() || column < 0 || column >= rows_.at(row).size()) {
      return QString();
    }
    return rows_.at(row).at(column);
  }

private:
  int columns_;
  QVector<QStringList> rows_;
  QStringList headers_;
  QVector<QIcon> icons_;
};
