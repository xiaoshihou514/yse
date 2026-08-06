// Phase 0 C++ shim for Yse.
//
// Deliberately small: the Rust side owns the reactive state (the Counter
// QObject) and this file owns the raw Qt Widgets surface, which CXX-Qt does
// not wrap yet. Expansion pattern: one small factory per widget family,
// wired together in run_app().

#include "yse-spike/src/spike.h"

#include <QApplication>
#include <QDebug>
#include <QLabel>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>
#include <QWidget>

#include "yse-spike/src/bridge.cxxqt.h"

namespace {

// --- Widget factories (the Phase 0 "wrapper" surface) -----------------------

QVBoxLayout* create_layout(QWidget* window)
{
  auto* layout = new QVBoxLayout(window);
  layout->setContentsMargins(12, 12, 12, 12);
  layout->setSpacing(8);
  return layout;
}

QLabel* create_label(const QString& text, QWidget* window)
{
  auto* label = new QLabel(text, window);
  label->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
  return label;
}

QPushButton* create_button(const QString& text, QWidget* window)
{
  return new QPushButton(text, window);
}

// --- Signal wiring ----------------------------------------------------------

void wire_signals(yse::spike::Counter& counter,
                  QWidget* window,
                  QLabel* count_label,
                  QLabel* result_label,
                  QPushButton* increment_button,
                  QPushButton* work_button)
{
  // Qt widget signals -> Rust invokables.
  QObject::connect(increment_button, &QPushButton::clicked, &counter, [&counter] {
    counter.incrementCount();
  });
  QObject::connect(work_button, &QPushButton::clicked, &counter, [&counter] {
    counter.startBackgroundWork();
  });

  // Rust property changes -> Qt widget labels (Rust-originated updates).
  QObject::connect(&counter, &yse::spike::Counter::countChanged, window, [&counter, count_label] {
    count_label->setText(
      QStringLiteral("Count: %1").arg(static_cast<qlonglong>(counter.getCount())));
  });
  QObject::connect(
    &counter, &yse::spike::Counter::resultChanged, window, [&counter, result_label] {
      result_label->setText(
        QStringLiteral("Background result: %1").arg(static_cast<qlonglong>(counter.getResult())));
    });

  // Lifecycle hooks: widget destruction is observable, and the Rust state
  // dies with the QObject (printed by the Drop impl on the Rust side).
  QObject::connect(&counter, &QObject::destroyed, [] {
    qInfo() << "[spike] QObject::destroyed fired for Counter";
  });
  QObject::connect(window, &QObject::destroyed, [] {
    qInfo() << "[spike] QObject::destroyed fired for window";
  });
}

} // namespace

int run_app() noexcept
{
  int argc = 1;
  static const char arg0[] = "yse";
  char* argv[] = { const_cast<char*>(arg0), nullptr };
  QApplication app(argc, argv);

  QWidget window;
  window.setWindowTitle(QStringLiteral("Yse Phase 0 spike"));
  window.resize(360, 180);
  auto* layout = create_layout(&window);
  auto* count_label = create_label(QStringLiteral("Count: 0"), &window);
  auto* result_label = create_label(QStringLiteral("Background result: —"), &window);
  auto* increment_button = create_button(QStringLiteral("Increment"), &window);
  auto* work_button = create_button(QStringLiteral("Run background work"), &window);

  layout->addWidget(count_label);
  layout->addWidget(result_label);
  layout->addWidget(increment_button);
  layout->addWidget(work_button);

  // The Rust-owned QObject is a child of the window, so it is destroyed
  // together with the widget tree.
  yse::spike::Counter counter(&window);

  wire_signals(counter, &window, count_label, result_label, increment_button, work_button);

  window.show();
  qInfo() << "[spike] QApplication running, QWidget window shown";

  if (qEnvironmentVariableIsSet("YSE_SMOKE")) {
    // Deterministic headless run: two clicks, one background task, then close.
    QTimer::singleShot(200, &app, [&] { increment_button->click(); });
    QTimer::singleShot(450, &app, [&] { increment_button->click(); });
    QTimer::singleShot(650, &app, [&] { work_button->click(); });
    QTimer::singleShot(1400, &app, [&] {
      window.close();
      app.quit();
    });
  }

  const int exit_code = app.exec();
  qInfo() << "[spike] event loop exited with code" << exit_code;
  return exit_code;
}
