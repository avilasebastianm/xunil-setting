//! Graphical entry point of arch-setting.

use std::time::Duration;

use adw::prelude::*;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, gettext, textdomain};
use gtk::glib;

const APP_ID: &str = "io.github.avilasebastianm.ArchSetting";
const APP_NAME: &str = "Arch Setting";
const GETTEXT_DOMAIN: &str = "arch-setting";
const LOCALE_DIR: &str = match option_env!("ARCH_SETTING_LOCALE_DIR") {
    Some(dir) => dir,
    None => "/usr/share/locale",
};
const SMOKE_TEST_VAR: &str = "ARCH_SETTING_SMOKE_TEST";
const SMOKE_TEST_DURATION: Duration = Duration::from_millis(500);

fn main() -> glib::ExitCode {
    init_translations();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(present_main_window);
    app.run()
}

/// Selects the message catalog. GTK sets the process locale itself when it starts.
///
/// A failure here only leaves the interface untranslated, so it must not stop the application.
fn init_translations() {
    let result = bindtextdomain(GETTEXT_DOMAIN, LOCALE_DIR)
        .and_then(|_| bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8"))
        .and_then(|_| textdomain(GETTEXT_DOMAIN));
    if let Err(error) = result {
        eprintln!("translations unavailable: {error}");
    }
}

fn present_main_window(app: &adw::Application) {
    let placeholder = adw::StatusPage::builder()
        .icon_name(APP_ID)
        .title(gettext("No panels yet"))
        .description(gettext("Panels will appear here as modules are added."))
        .build();

    let content = adw::ToolbarView::builder().content(&placeholder).build();
    content.add_top_bar(&adw::HeaderBar::new());

    adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(900)
        .default_height(600)
        .content(&content)
        .build()
        .present();

    if std::env::var_os(SMOKE_TEST_VAR).is_some() {
        glib::timeout_add_local_once(
            SMOKE_TEST_DURATION,
            glib::clone!(
                #[weak]
                app,
                move || app.quit()
            ),
        );
    }
}
