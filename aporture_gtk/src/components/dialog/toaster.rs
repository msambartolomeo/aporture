use adw::prelude::*;
use relm4::prelude::*;
use relm4_icons::icon_names;

#[derive(Debug, Default)]
pub struct Toaster {
    toaster: adw::ToastOverlay,
}

impl AsRef<adw::ToastOverlay> for Toaster {
    fn as_ref(&self) -> &adw::ToastOverlay {
        &self.toaster
    }
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Info,
    Success,
    Warn,
    Error,
}

impl Severity {
    fn icon(&self) -> &str {
        match self {
            Self::Info => icon_names::INFO_OUTLINE,
            Self::Success => icon_names::SUCCESS_SMALL,
            Self::Warn => icon_names::WARNING_OUTLINE,
            Self::Error => icon_names::MINUS_CIRCLE_OUTLINE,
        }
    }

    fn color(&self) -> &str {
        match self {
            Self::Info => "accent",
            Self::Success => "success",
            Self::Warn => "warning",
            Self::Error => "error",
        }
    }
}

impl Toaster {
    pub fn add_toast(&self, message: &str, severity: Severity) {
        relm4::view! {
            title = gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                gtk::Image {
                    set_icon_name: Some(severity.icon()),

                    add_css_class: severity.color(),
                },

                // Spacer
                gtk::Label {
                    add_css_class: "heading",
                    set_text: "  ",
                },

                gtk::Label {
                    add_css_class: "heading",
                    set_text: &message,
                }
            }
        }

        let toast = adw::Toast::builder()
            .timeout(3)
            .custom_title(&title)
            .build();

        self.toaster.add_toast(toast);
    }
}
