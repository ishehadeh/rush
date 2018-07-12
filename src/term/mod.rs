pub mod ansi;
mod error;
pub use self::error::*;
use failure::ResultExt;
use nix::sys::termios;
use nix::sys::termios::LocalFlags;
use std::io;
use std::io::Read;
use std::os::unix::io::RawFd;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Key {
    Ascii(char),
    Control(char),
    Delete,
    Newline,
    Escape,
    Arrow(ArrowDirection),
    Invalid(u8),
}

pub fn newline() {
    ansi::scroll_down(1);
    ansi::cursor_down(1);
    ansi::erase_line(ansi::ClearType::Everything);
    ansi::cursor_column(1);
}

pub fn take_terminal<F>(mut onkey: F) -> Result<()>
where
    F: FnMut(Key) -> bool,
{
    let original = init_raw_mode(0)?;
    for c in io::stdin().bytes() {
        let ch = c.context(ErrorKind::GetCharFailed)?;
        if !match ch {
            0...12 => onkey(Key::Control((ch + 64) as char)),
            13 => onkey(Key::Newline),
            27 => onkey(Key::Escape),
            127 => onkey(Key::Delete),
            32...126 => onkey(Key::Ascii(ch as char)),
            _ => onkey(Key::Invalid(ch)),
        } {
            break;
        }
    }

    termios::tcsetattr(0, termios::SetArg::TCSAFLUSH, &original)
        .context(ErrorKind::ExitRawModeFailed)?;
    Ok(())
}

fn init_raw_mode(fd: RawFd) -> Result<termios::Termios> {
    let mut raw_termios = termios::tcgetattr(fd).unwrap();
    let original_termios = raw_termios.clone();

    termios::cfmakeraw(&mut raw_termios); // TODO do this manually
    raw_termios.local_flags.remove(LocalFlags::ICANON);
    termios::tcsetattr(0, termios::SetArg::TCSAFLUSH, &raw_termios)
        .context(ErrorKind::InitRawModeFailed)?;
    Ok(original_termios)
}
