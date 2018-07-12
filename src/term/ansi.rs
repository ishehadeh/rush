use std::fmt;
macro_rules! esc {
    ($c:expr) => {
        concat!('\x1B', $c)
    };
}

macro_rules! eos {
    ($s:expr) => {
        concat!($s, esc!('\\'))
    };
}

macro_rules! osc {
    ($command:expr) => {
        concat!(esc!("]"), $command, ';')
    };
}

pub const RESET: &'static str = esc!("c");
const CSI: &'static str = esc!('[');

pub enum ClearType {
    AfterCursor,
    BeforeCursor,
    Everything,
    EverthingAndReset,
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

pub fn cursor_up(x: usize) {
    print!("{}{}{}", CSI, x, 'A')
}

pub fn cursor_down(x: usize) {
    print!("{}{}{}", CSI, x, 'B')
}

pub fn cursor_right(x: usize) {
    print!("{}{}{}", CSI, x, 'C')
}

pub fn cursor_left(x: usize) {
    print!("{}{}{}", CSI, x, 'D')
}

pub fn cursor_next_line(x: usize) {
    print!("{}{}{}", CSI, x, 'E')
}

pub fn cursor_last_line(x: usize) {
    print!("{}{}{}", CSI, x, 'F')
}

pub fn cursor_column(x: usize) {
    print!("{}{}{}", CSI, x, 'G')
}

pub fn cursor_move(x: usize, y: usize) {
    print!("{}{};{}{}", CSI, x, y, 'H') // f would also work instead of H
}

pub fn erase(x: ClearType) {
    print!("{}{}{}", CSI, x as isize, 'J')
}

pub fn erase_line(x: ClearType) {
    print!("{}{}{}", CSI, x as isize, 'K')
}

pub fn scroll_down(x: usize) {
    print!("{}{}{}", CSI, x, 'S')
}

pub fn scroll_up(x: usize) {
    print!("{}{}{}", CSI, x, 'T')
}

pub fn effect(x: Effect) {
    print!(
        "{}{}m",
        CSI,
        match x {
            Effect::Background(x) => {
                if x < 8 {
                    format!("{}", 40 + x)
                } else if x < 16 {
                    format!("{}", 100 + x - 8)
                } else {
                    format!("48;5;{}", x)
                }
            }
            Effect::Foreground(x) => {
                if x < 8 {
                    format!("{}", 30 + x)
                } else if x < 16 {
                    format!("{}", 90 + x - 8)
                } else {
                    format!("38;5;{}", x)
                }
            }
            Effect::BackgroundCustom(r, g, b) => format!("48;2;{};{};{}", r, g, b),
            Effect::ForegroundCustom(r, g, b) => format!("38;2;{};{};{}", r, g, b),
            Effect::Font(x) => format!("{}", 11 + x),
            Effect::DefaultFont => format!("10"),
            Effect::DefaultBackground => format!("40"),
            Effect::DefaultForeground => format!("30"),
            Effect::Reset => format!("0"),
            Effect::Bold => format!("1"),
            Effect::Faint => format!("2"),
            Effect::Italic => format!("3"),
            Effect::Underline => format!("4"),
            Effect::SlowBlink => format!("5"),
            Effect::FastBlink => format!("6"),
            Effect::Invert => format!("7"),
            Effect::Conceal => format!("8"),
            Effect::StrikeThrough => format!("9"),
            Effect::Fraktur => format!("20"),
            Effect::Frame => format!("51"),
            Effect::Circle => format!("52"),
            Effect::Overline => format!("53"),
            Effect::IdeogramUnderline => format!("60"),
            Effect::IdeogramDoubleUnderline => format!("61"),
            Effect::IdeogramOverline => format!("62"),
            Effect::IdeogramDoubleOverline => format!("63"),
            Effect::IdeogramStressMarks => format!("64"),
            Effect::UnsetBold => format!("21"),
            Effect::UnsetBoldAndFaint => format!("22"),
            Effect::UnsetFrakturAndItalic => format!("23"),
            Effect::UnsetUnderline => format!("24"),
            Effect::UnsetBlink => format!("25"),
            Effect::UnsetInvert => format!("27"),
            Effect::UnsetConceal => format!("28"),
            Effect::UnsetStrikeThrough => format!("29"),
            Effect::UnsetFrameAndCircle => format!("54"),
            Effect::UnsetOverline => format!("55"),
            Effect::UnsetIdeogram => format!("65"),
        }
    )
}
