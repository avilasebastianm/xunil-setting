//! Graphical entry point of xunil-setting.

use std::time::Duration;

use adw::prelude::*;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, gettext, textdomain};
use gtk::glib;
use xunil_setting_core::capability::SystemEnvironment;
use xunil_setting_core::module::PanelDescriptor;
use xunil_setting_core::modules::builtin_registry;

const APP_ID: &str = "io.github.avilasebastianm.XunilSetting";
const APP_NAME: &str = "Xunil Setting";
const GETTEXT_DOMAIN: &str = "xunil-setting";
const LOCALE_DIR: &str = match option_env!("XUNIL_SETTING_LOCALE_DIR") {
    Some(dir) => dir,
    None => "/usr/share/locale",
};
const SMOKE_TEST_VAR: &str = "XUNIL_SETTING_SMOKE_TEST";
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
    let registry = builtin_registry().expect("built-in identifiers are unique");
    let panels = registry.available_panels(&SystemEnvironment);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(900)
        .default_height(600)
        .build();
    if panels.is_empty() {
        window.set_content(Some(&empty_view()));
    } else {
        window.set_content(Some(&panels_view(&panels)));
    }
    window.present();

    if std::env::var_os(SMOKE_TEST_VAR).is_some() {
        for panel in &panels {
            println!("panel: {}", panel.id);
        }
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

fn empty_view() -> adw::ToolbarView {
    let placeholder = adw::StatusPage::builder()
        .icon_name(APP_ID)
        .title(gettext("No panels yet"))
        .description(gettext("Panels will appear here as modules are added."))
        .build();

    with_header_bar(&placeholder)
}

fn panels_view(panels: &[&'static PanelDescriptor]) -> adw::NavigationSplitView {
    let stack = gtk::Stack::new();
    let list = gtk::ListBox::builder()
        .css_classes(["navigation-sidebar"])
        .build();

    for panel in panels {
        let page = adw::StatusPage::builder()
            .icon_name(panel.icon_name)
            .title(gettext(panel.title))
            .description(gettext("The settings of this panel will appear here."))
            .build();
        stack.add_named(&page, Some(panel.id));

        let row = adw::ActionRow::builder()
            .title(gettext(panel.title))
            .activatable(true)
            .build();
        row.add_prefix(&gtk::Image::from_icon_name(panel.icon_name));
        list.append(&row);
    }

    let ids: Vec<&'static str> = panels.iter().map(|panel| panel.id).collect();
    list.connect_row_selected(glib::clone!(
        #[weak]
        stack,
        move |_, row| {
            let id = row.and_then(|row| usize::try_from(row.index()).ok().and_then(|i| ids.get(i)));
            if let Some(id) = id {
                stack.set_visible_child_name(id);
            }
        }
    ));
    list.select_row(list.row_at_index(0).as_ref());

    adw::NavigationSplitView::builder()
        .sidebar(&adw::NavigationPage::new(
            &with_header_bar(&list),
            &gettext("Settings"),
        ))
        .content(&adw::NavigationPage::new(
            &with_header_bar(&stack),
            APP_NAME,
        ))
        .build()
}

fn with_header_bar(content: &impl IsA<gtk::Widget>) -> adw::ToolbarView {
    let view = adw::ToolbarView::builder().content(content).build();
    view.add_top_bar(&adw::HeaderBar::new());
    view
}
