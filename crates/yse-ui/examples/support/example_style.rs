use yse_ui::Application;

/// A tiny, opt-in accent layer for the showcase apps.
///
/// It deliberately styles only widgets carrying a `yseClass`, so menus,
/// toolbars, fonts, palette, focus rings, and all ordinary controls continue
/// to come from the user's active Qt/KDE theme.
pub fn apply(app: &Application) {
    app.set_style_sheet(
        r#"
        QLineEdit[yseClass="display"] {
          font-size: 24pt; font-weight: 500; padding: 12px 14px;
          min-height: 38px;
        }
        QPushButton[yseClass="key"] { min-height: 38px; font-size: 12pt; }
        QPushButton[yseClass="operator"] { min-height: 38px; font-size: 12pt; font-weight: 600; }
        QPushButton[yseClass="accent"] { min-height: 38px; font-size: 12pt; font-weight: 600; }
        QPushButton[yseClass="quiet"] { min-height: 32px; }
        QDateEdit[yseClass="schedule"], QTimeEdit[yseClass="schedule"] { min-height: 32px; }
        "#,
    );
}
