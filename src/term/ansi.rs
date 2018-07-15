macro_rules! esc {
    ($c:expr) => {
        concat!('\x1B', $c)
    };
}

macro_rules! command_template {
    () => {};
    ($_:expr) => {
        "{}"
    };

    ($_:expr, $($items:expr),+) => {
        concat!("{};", command_template!($($items),*))
    };
}

macro_rules! command {
    ($prefix:expr, $suffix:expr, $mac:ident ! ($($args:tt),+)) => {
        $mac!(
            concat!($prefix, command_template!($($args),*), $suffix),
            $($args),*
        )
    };

}

#[macro_export]
macro_rules! osc_str {
    
    ($cmd:expr, $string:expr, $($args:expr),+) => {
        command!(
            esc!(']'),
            esc!('\\'),
            print!($cmd, $($args),*, $string)
        )
    };

    ($cmd:expr, ($($args:expr),+) ($string:expr) -> $($fmt_args:expr),+) => {
        print!(concat!(esc!(']'), "{};", command_template!($($args),*), $string, esc!('\\')), $cmd, $($args),*, $($fmt_args),*)
    };


    ($cmd:expr, $string:expr) => {
        command!(
            esc!(']'),
            esc!('\\'),
            print!($cmd, $string)
        )
    };
}

#[macro_export]
macro_rules! csi {
    ($typ:ident) => {
        print!(concat!(esc!("["), stringify!($typ)))
    };

    ($typ:ident, $($args:expr),+) => {
        command!(
            esc!('['),
            stringify!($typ),
            print!($($args),*)
        )
    };

    (fmt $typ:ident) => {
        format_args!(concat!(esc!("["), stringify!($typ)))
    };

    ($typ:ident, $($args:expr),+) => {
        command!(
            esc!('['),
            stringify!($typ),
            format_args!($($args),*)
        )
    };
}

pub enum ClearType {
    AfterCursor,
    BeforeCursor,
    Everything,
    EverthingAndReset,
}

pub enum Color {
    Index(u8),
    Rgb(u8, u8, u8),
}

pub enum Effect {
    Reset,
    Bold,
    Faint,
    Italic,
    Underline,
    SlowBlink,
    FastBlink,
    Invert,
    Conceal,
    StrikeThrough,
    Frame,
    Circle,
    Overline,
    Fraktur,                 // Rarely supported
    IdeogramUnderline,       // Effectively never supported
    IdeogramDoubleUnderline, // ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    IdeogramOverline,        // ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    IdeogramDoubleOverline,  // ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    IdeogramStressMarks,     // ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    UnsetIdeogram,           // unset everything in the Ideogram group
    UnsetFrameAndCircle,
    UnsetOverline,
    UnsetBold,
    UnsetBoldAndFaint,
    UnsetFrakturAndItalic,
    UnsetUnderline,
    UnsetBlink,
    UnsetInvert,
    UnsetConceal,
    UnsetStrikeThrough,
    Font(u8), // Fonts can be from 0 (default) to 9
    Foreground(u8),
    ForegroundCustom(u8, u8, u8),
    Background(u8),
    BackgroundCustom(u8, u8, u8),
    DefaultBackground,
    DefaultForeground,
    DefaultFont,
}

impl From<u8> for Color {
    fn from(v: u8) -> Color {
        Color::Index(v)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from(v: (u8, u8, u8)) -> Color {
        Color::Rgb(v.0, v.1, v.2)
    }
}

pub fn cursor_up(x: usize) {
    csi!(A, x);
}

pub fn cursor_down(x: usize) {
    csi!(B, x);
}

pub fn cursor_right(x: usize) {
    csi!(C, x);
}

pub fn cursor_left(x: usize) {
    csi!(D, x);
}

pub fn cursor_next_line(x: usize) {
    csi!(E, x);
}

pub fn cursor_last_line(x: usize) {
    csi!(F, x);
}

pub fn cursor_column(x: usize) {
    csi!(G, x);
}

pub fn cursor_move(x: usize, y: usize) {
    csi!(H, x, y);
}

pub fn erase(x: ClearType) {
    let ix = x as isize;
    csi!(K, ix);
}

pub fn erase_line(x: ClearType) {
    let ix = x as isize;
    csi!(K, ix);
}

pub fn scroll_down(x: usize) {
    csi!(S, x);
}

pub fn scroll_up(x: usize) {
    csi!(T, x);
}

pub fn effect(x: Effect) {
    match x {
        Effect::Background(x) => {
            if x < 8 {
                csi!(m, x + 40)
            } else if x < 16 {
                csi!(m, x + 92);
            } else {
                csi!(m, 48, 5, x);
            }
        }
        Effect::Foreground(x) => {
            if x < 8 {
                csi!(m, x + 30)
            } else if x < 16 {
                csi!(m, x + 82);
            } else {
                csi!(m, 38, 5, x);
            }
        }
        Effect::BackgroundCustom(r, g, b) => csi!(m, 48, 2, r, g, b),
        Effect::ForegroundCustom(r, g, b) => csi!(m, 38, 2, r, g, b),
        Effect::Font(x) => {
            let ix = x + 11;
            csi!(m, ix)
        }
        Effect::DefaultFont => csi!(m, 10),
        Effect::DefaultBackground => csi!(m, 49),
        Effect::DefaultForeground => csi!(m, 39),
        Effect::Reset => csi!(m, 0),
        Effect::Bold => csi!(m, 1),
        Effect::Faint => csi!(m, 2),
        Effect::Italic => csi!(m, 3),
        Effect::Underline => csi!(m, 4),
        Effect::SlowBlink => csi!(m, 5),
        Effect::FastBlink => csi!(m, 6),
        Effect::Invert => csi!(m, 7),
        Effect::Conceal => csi!(m, 8),
        Effect::StrikeThrough => csi!(m, 9),
        Effect::Fraktur => csi!(m, 20),
        Effect::Frame => csi!(m, 51),
        Effect::Circle => csi!(m, 52),
        Effect::Overline => csi!(m, 53),
        Effect::IdeogramUnderline => csi!(m, 60),
        Effect::IdeogramDoubleUnderline => csi!(m, 61),
        Effect::IdeogramOverline => csi!(m, 62),
        Effect::IdeogramDoubleOverline => csi!(m, 63),
        Effect::IdeogramStressMarks => csi!(m, 64),
        Effect::UnsetBold => csi!(m, 21),
        Effect::UnsetBoldAndFaint => csi!(m, 22),
        Effect::UnsetFrakturAndItalic => csi!(m, 23),
        Effect::UnsetUnderline => csi!(m, 24),
        Effect::UnsetBlink => csi!(m, 25),
        Effect::UnsetInvert => csi!(m, 27),
        Effect::UnsetConceal => csi!(m, 28),
        Effect::UnsetStrikeThrough => csi!(m, 29),
        Effect::UnsetFrameAndCircle => csi!(m, 54),
        Effect::UnsetOverline => csi!(m, 55),
        Effect::UnsetIdeogram => csi!(m, 65),
    }
}
