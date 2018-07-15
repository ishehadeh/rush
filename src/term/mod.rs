#[macro_use]
pub mod ansi;
mod error;
pub mod terminfo;
pub mod xterm;

pub use self::error::*;
use failure::ResultExt;
use nix::sys::termios;
use nix::sys::termios::LocalFlags;
use std::collections::VecDeque;
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

pub struct Keys<T: io::Read> {
    // Keys may need to be buffered if we have to back out of an escape code
    buffer: VecDeque<Key>,
    stream: T,
}

pub fn newline() {
    ansi::scroll_down(1);
    ansi::cursor_down(1);
    ansi::erase_line(ansi::ClearType::Everything);
    ansi::cursor_column(1);
}

/// Get an iterator over the users keystrokes from `STDIN`.
pub fn keys<'a>() -> Keys<io::Stdin> {
    Keys {
        buffer: VecDeque::with_capacity(4),
        stream: io::stdin(),
    }
}

pub fn take_terminal<F>(mut onkey: F) -> Result<()>
where
    F: FnMut(Key) -> bool,
{
    let original = init_raw_mode(0)?;

    for key in keys() {
        if !onkey(key?) {
            break;
        }
    }

    termios::tcsetattr(0, termios::SetArg::TCSAFLUSH, &original)
        .context(ErrorKind::ExitRawModeFailed)?;
    Ok(())
}

impl<T> Keys<T>
where
    T: Read,
{
    fn getch(&mut self) -> Result<Option<u8>> {
        let mut c: [u8; 1] = [0; 1];
        let read = self.stream.read(&mut c).context(ErrorKind::GetCharFailed)?;
        if read == 0 {
            Ok(None)
        } else {
            Ok(Some(c[0]))
        }
    }

    fn getkey(&mut self) -> Result<Key> {
        // get the next character, or if there isn't one return `Timeout`

        let mut c = self.getch()?;
        while c.is_none() {
            c = self.getch()?;
        }
        let ch = c.unwrap();

        Ok(match ch {
            0...12 => Key::Control((ch + 64) as char),
            13 => Key::Newline,
            27 => match self.getch()? {
                Some(91) => match self.getch()? {
                    Some(65) => Key::Arrow(ArrowDirection::Up),
                    Some(66) => Key::Arrow(ArrowDirection::Down),
                    Some(67) => Key::Arrow(ArrowDirection::Left),
                    Some(68) => Key::Arrow(ArrowDirection::Right),
                    Some(v) => {
                        self.buffer.push_back(Key::Escape);
                        self.buffer.push_back(Key::Ascii(']'));
                        Key::Ascii(v as char)
                    }
                    None => {
                        self.buffer.push_back(Key::Escape);
                        Key::Ascii('[')
                    }
                },
                Some(v) => {
                    self.buffer.push_back(Key::Escape);
                    Key::Ascii(v as char)
                }
                None => Key::Escape,
            },
            127 => Key::Delete,
            32...126 => Key::Ascii(ch as char),
            _ => Key::Invalid(ch),
        })
    }
}

impl<T> Iterator for Keys<T>
where
    T: Read,
{
    type Item = Result<Key>;

    fn next(&mut self) -> Option<Self::Item> {
        // if a key is in the buffer then return it
        match self.buffer.pop_front() {
            Some(v) => return Some(Ok(v)),
            None => (),
        };

        Some(self.getkey())
    }
}

fn init_raw_mode(fd: RawFd) -> Result<termios::Termios> {
    let mut raw_termios = termios::tcgetattr(fd).unwrap();
    let original_termios = raw_termios.clone();

    termios::cfmakeraw(&mut raw_termios); // TODO do this manually
    raw_termios.local_flags.remove(LocalFlags::ICANON);
    raw_termios.control_chars[termios::SpecialCharacterIndices::VTIME as usize] = 10;
    raw_termios.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 0;
    termios::tcsetattr(0, termios::SetArg::TCSAFLUSH, &raw_termios)
        .context(ErrorKind::InitRawModeFailed)?;

    Ok(original_termios)
}
