use std::env::current_dir;

#[macro_export]
macro_rules! ps {
    ($($arg:tt)*) => {
        $crate::print_hyperlinked!($($arg)*)
    };
}

#[macro_export]
macro_rules! todo {
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print_hyperlinked!(
            concat!("\x1b[1;35mTODO:\x1b[0m ", $fmt),
            $($arg)*
        );
    };
    ($msg:expr) => {
        $crate::print_hyperlinked!(
            concat!("\x1b[1;35mTODO:\x1b[0m ", $msg)
        );
    };
}

#[macro_export]
macro_rules! print_hyperlinked {
    ($fmt:expr, $($arg:tt)*) => {{
        let url = $crate::format_vscode_hyperlink(file!(), line!());
        println!("{}", $crate::format_osc8_hyperlink(&url, &format!($fmt, $($arg)*)));
    }};
    ($msg:expr) => {{
        let url = $crate::format_vscode_hyperlink(file!(), line!());
        println!("{}", $crate::format_osc8_hyperlink(&url, &format!($msg)));
    }};
}

pub fn format_vscode_hyperlink(rel_path: &str, line: u32) -> String {
    let path = current_dir().unwrap().join(rel_path);
    format!("cursor://file/{}:{}", path.display(), line)
}

pub fn format_osc8_hyperlink(url: &str, text: &str) -> String {
    format!(
        "{osc}8;;{url}{st}{text}{osc}8;;{st}",
        url = url,
        text = text,
        osc = "\x1b]",
        st = "\x1b\\"
    )
}

/// Trait for types that can be rendered as terminal hyperlinks.
pub trait TerminalHyperlink {
    /// Returns the display text for this item.
    fn display_text(&self) -> String;

    /// Returns the URL this item should link to.
    fn hyperlink_url(&self) -> String;

    /// Returns an OSC 8 hyperlinked string for terminal display.
    fn hyperlink(&self) -> String {
        format_osc8_hyperlink(&self.hyperlink_url(), &self.display_text())
    }
}
