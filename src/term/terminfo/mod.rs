mod fields;
pub mod term;

pub use self::fields::*;
pub use self::term::Term;
pub use self::BooleanField::*;
pub use self::NumericField::*;
pub use self::StringField::*;

use failure::ResultExt;
use nom;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str;
use term::{Error, ErrorKind, Result};

lazy_static! {
    static ref TERMINFO: Option<Term> = {
        if let Some(path) = path() {
            match parse_file(path) {
                Ok(v) => Some(v),
                Err(_) => None,
            }
        } else {
            None
        }
    };
}

pub fn parse<T: AsRef<[u8]>>(bytes: T) -> Result<Term> {
    term::terminfo(bytes.as_ref()).map(|v| v.1).map_err(|e| {
        Error::from(match e {
            nom::Err::Incomplete(i) => ErrorKind::TerminfoIncomplete(i),
            nom::Err::Error(ctx) => ErrorKind::from(ctx.into_error_kind()),
            nom::Err::Failure(ctx) => ErrorKind::from(ctx.into_error_kind()),
        })
    })
}

pub fn parse_file<T: AsRef<Path>>(path: T) -> Result<Term> {
    let mut file = File::open(path).context(ErrorKind::IoError)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data);
    parse(&data)
}

pub fn found_terminfo() -> bool {
    (*TERMINFO).is_some()
}

pub fn name() -> String {
    match *TERMINFO {
        Some(ref term) => term.name(),
        None => String::new(),
    }
}

pub fn names() -> Vec<String> {
    match *TERMINFO {
        Some(ref term) => term.names(),
        None => Vec::new(),
    }
}

pub fn boolean(field: BooleanField) -> bool {
    match *TERMINFO {
        Some(ref term) => term.boolean(field),
        None => false,
    }
}

pub fn string(field: StringField) -> Option<String> {
    match *TERMINFO {
        Some(ref term) => term.string(field),
        None => None,
    }
}

pub fn number(field: NumericField) -> Option<usize> {
    match *TERMINFO {
        Some(ref term) => term.number(field),
        None => None,
    }
}

pub fn ext_boolean<T: AsRef<str>>(s: T) -> bool {
    match *TERMINFO {
        Some(ref term) => term.ext_boolean(s),
        None => false,
    }
}

pub fn ext_string<T: AsRef<str>>(s: T) -> Option<String> {
    match *TERMINFO {
        Some(ref term) => term.ext_string(s),
        None => None,
    }
}

pub fn ext_number<T: AsRef<str>>(s: T) -> Option<u16> {
    match *TERMINFO {
        Some(ref term) => term.ext_number(s),
        None => None,
    }
}

pub fn path() -> Option<PathBuf> {
    let terminal_name = match env::var("TERM") {
        Ok(v) => {
            if v.is_empty() {
                return None;
            } else {
                v
            }
        }
        Err(_) => return None,
    };

    let letter = terminal_name.chars().take(1).collect::<String>();

    match env::var("TERMINFO") {
        Ok(v) => Some([v, letter, terminal_name].iter().collect()),
        Err(_) => {
            let mut home = PathBuf::from(env::home_dir().unwrap_or(PathBuf::new()));

            if let Some(home) = env::home_dir() {
                let path = home.join(&letter).join(&terminal_name);
                if path.exists() {
                    return Some(path);
                }
            }

            let dirlist = match env::var("TERMINFO_DIRS") {
                Ok(v) => v.to_string()
                    .split(":")
                    .map(|p| {
                        if p.is_empty() {
                            "/usr/share/terminfo".to_owned()
                        } else {
                            p.to_owned()
                        }
                    })
                    .collect::<Vec<String>>(),
                Err(_) => vec!["/usr/share/terminfo".to_owned()],
            };
            for dir in dirlist {
                let path: PathBuf = [&dir, &letter, &terminal_name].iter().collect();
                if path.exists() {
                    return Some(path);
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod test {
    // TODO add tests for checking extended capabilities
    use term::terminfo::*;

    const RXVT_INFO: &'static [u8] = include_bytes!("./test/rxvt");
    const XTERM_INFO: &'static [u8] = include_bytes!("./test/xterm");
    const LINUX_16COLOR_INFO: &'static [u8] = include_bytes!("./test/linux-16color");

    #[test]
    fn errors() {
        let fake_terminal = include_bytes!("./test/fake-terminal");
        let incomplete_terminal = include_bytes!("./test/incomplete-terminal");
        let incomplete_terminal2 = include_bytes!("./test/incomplete-terminal2");

        assert_eq!(
            parse(&fake_terminal[..]).unwrap_err().kind(),
            &ErrorKind::NotATermInfoFile
        );

        assert_eq!(
            parse(&incomplete_terminal[..]).unwrap_err().kind(),
            &ErrorKind::TooManyFieldsAtBool
        );

        assert_eq!(
            parse(&incomplete_terminal2[..]).unwrap_err().kind(),
            &ErrorKind::FailedToFindEndOfNames
        );
    }

    #[test]
    fn names() {
        let rxvt = parse(RXVT_INFO).unwrap();

        assert_eq!(rxvt.name(), "rxvt");
        assert_eq!(
            rxvt.names(),
            vec!["rxvt", "rxvt terminal emulator (X Window System)"]
        );
    }

    #[test]
    fn bools() {
        let rxvt = parse(RXVT_INFO).unwrap();
        let xterm = parse(XTERM_INFO).unwrap();
        let l16c = parse(LINUX_16COLOR_INFO).unwrap();

        assert_eq!(rxvt.boolean(AutoLeftMargin), false);
        assert_eq!(rxvt.boolean(AutoRightMargin), true);
        assert_eq!(rxvt.boolean(MoveInsertMode), true);
        assert_eq!(rxvt.boolean(XonXoff), true);

        assert_eq!(xterm.boolean(AutoLeftMargin), false);
        assert_eq!(xterm.boolean(AutoRightMargin), true);
        assert_eq!(xterm.boolean(MoveInsertMode), true);
        assert_eq!(xterm.boolean(XonXoff), false);

        assert_eq!(l16c.boolean(AutoLeftMargin), false);
        assert_eq!(l16c.boolean(AutoRightMargin), true);
        assert_eq!(l16c.boolean(MoveInsertMode), true);
        assert_eq!(l16c.boolean(CanChange), true);
    }

    #[test]
    fn numbers() {
        let rxvt = parse(RXVT_INFO).unwrap();
        let xterm = parse(XTERM_INFO).unwrap();
        let l16c = parse(LINUX_16COLOR_INFO).unwrap();

        assert_eq!(rxvt.number(Columns), Some(80));
        assert_eq!(rxvt.number(MaxColors), Some(8));

        assert_eq!(xterm.number(MaxColors), Some(8));

        assert_eq!(l16c.number(Columns), None);
        assert_eq!(l16c.number(MaxColors), Some(16));
    }

    #[test]
    fn strings() {
        let rxvt = parse(RXVT_INFO).unwrap();

        assert_eq!(rxvt.string(KeyF10), Some(String::from("\x1b[21~")));
        assert_eq!(rxvt.string(KeyHome), Some(String::from("\x1b[7~")));
        assert_eq!(rxvt.string(Bell), Some(String::from("\x07")));
        assert_eq!(rxvt.string(KeyCancel), None);

        assert_eq!(rxvt.str(KeyF10), Some("\x1b[21~"));
        assert_eq!(rxvt.str(KeyHome), Some("\x1b[7~"));
        assert_eq!(rxvt.str(Bell), Some("\x07"));
        assert_eq!(rxvt.str(KeyCancel), None);
    }
}
