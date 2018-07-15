///! Xterm OS Commands
///!
///! This module can also be used xterm-based terminal (rxvt, xterm-256, kitty, etc)
use term::ansi;

#[derive(Debug, Clone)]
pub enum XColor {
    Index(u8),
    Rgbi(f32, f32, f32),
    Rgb(u16, u16, u16),
    Raw(String),
}

pub fn set_icon_and_title<T: AsRef<str>>(s: T) {
    osc_str!(0, s.as_ref());
}

pub fn set_icon<T: AsRef<str>>(s: T) {
    osc_str!(1, s.as_ref());
}

pub fn set_title<T: AsRef<str>>(s: T) {
    osc_str!(2, s.as_ref());
}

pub fn restore_title() {
    osc_str!(2, "");
}

pub fn set_x_property<T: AsRef<str>, U: AsRef<str>>(k: T, v: T) {
    osc_str!(3, format!("{}={}", k.as_ref(), v.as_ref()));
}

pub fn remove_x_property<T: AsRef<str>>(k: T) {
    let k_str = k.as_ref();
    osc_str!(3, k_str);
}

pub fn query_x_property<T: AsRef<str>>(k: T) {
    osc_str!(3, format!("?{}", k.as_ref()));
}

pub fn map_color(color: u8, new_color: XColor) {
    match new_color {
        XColor::Index(x) => osc_str!(4, color, x),
        XColor::Rgbi(r, g, b) => osc_str!(4, (color) ("rgbi:{}/{}/{}") -> r, g, b),
        XColor::Rgb(r, g, b) => osc_str!(4, (color) ("rgb:{}/{}/{}") -> r, g, b),
        XColor::Raw(s) => osc_str!(4, color, s),
    }
}

pub fn query_color(color: u8) {
    osc_str!(4, color, "?");
}

impl From<ansi::Color> for XColor {
    fn from(c: ansi::Color) -> XColor {
        match c {
            ansi::Color::Rgb(r, g, b) => XColor::Rgb(r as u16, g as u16, b as u16),
            ansi::Color::Index(c) => XColor::Index(c),
        }
    }
}

impl From<(u16, u16, u16)> for XColor {
    fn from(c: (u16, u16, u16)) -> XColor {
        XColor::Rgb(c.0, c.1, c.2)
    }
}

impl From<(f32, f32, f32)> for XColor {
    fn from(c: (f32, f32, f32)) -> XColor {
        XColor::Rgbi(c.0, c.1, c.2)
    }
}

impl<'a> From<&'a str> for XColor {
    fn from(s: &'a str) -> XColor {
        XColor::Raw(s.to_string())
    }
}

impl From<String> for XColor {
    fn from(s: String) -> XColor {
        XColor::Raw(s)
    }
}

///! Kitty extensions to the xterm protocol
///! [details](https://sw.kovidgoyal.net/kitty/protocol-extensions.html)
pub mod kitty {
    use term::ansi;
    use term::terminfo;

    pub enum Underline {
        None,
        Straight,
        Double,
        Curly,
        Dotted,
        Dashed,
    }

    pub fn supported() -> bool {
        terminfo::ext_boolean("Su")
    }

    pub fn set_underline(u: Underline) {
        match u {
            Underline::None => csi!(m, "4:0"),
            Underline::Straight => csi!(m, "4:1"),
            Underline::Double => csi!(m, "4:2"),
            Underline::Curly => csi!(m, "4:3"),
            Underline::Dotted => csi!(m, "4:4"),
            Underline::Dashed => csi!(m, "4:5"),
        }
    }

    pub fn set_underline_color<T: Into<ansi::Color>>(x: T) {
        match x.into() {
            ansi::Color::Index(i) => csi!(m, 58, 5, i),
            ansi::Color::Rgb(r, g, b) => csi!(m, 58, 2, r, g, b),
        }
    }

    pub fn reset_underline_color() {
        csi!(m, 59);
    }
}
