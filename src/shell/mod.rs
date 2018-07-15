use failure;
use lang;
use lang::parser;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use term;
pub struct Shell {
    command_buffer: String,
    history: Vec<String>,
    exit: bool,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            command_buffer: String::new(),
            history: Vec::new(),
            exit: false,
        }
    }

    fn print_error<T: failure::Fail>(e: T) {
        match e.cause() {
            Some(v) => println!("{}: {}", e, v),
            None => println!("{}", e),
        }
    }

    pub fn run(&mut self, environ: &mut lang::ExecutionEnvironment) {
        while !self.exit_requested() {
            let prefix_command = environ
                .variables()
                .value(&OsString::from("RUSH_PREFIX"))
                .to_string_lossy()
                .to_string();

            match environ.run(if prefix_command.is_empty() {
                "printf 'rush-%s$ ' \"$RUSH_VERSION\"".to_string()
            } else {
                prefix_command
            }) {
                Err(e) => Shell::print_error(e),
                _ => (),
            }

            let buffer = match self.readline(environ) {
                Ok(v) => v,
                Err(e) => {
                    println!();
                    Shell::print_error(e);
                    continue;
                }
            };
            if !self.exit_requested() {
                println!();

                if !buffer.is_empty() {
                    self.history.push(buffer.clone());
                    match environ.run(buffer) {
                        Err(e) => {
                            Shell::print_error(e);
                            continue;
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    pub fn readline(&mut self, environ: &mut lang::ExecutionEnvironment) -> term::Result<String> {
        io::stdout().flush();
        self.command_buffer.clear();

        let mut hist_index = self.history.len();
        let mut xoffset = 0;
        term::take_terminal(|k| {
            let backtrack = self.command_buffer.len();
            if backtrack != 0 && backtrack != xoffset {
                term::ansi::cursor_left(backtrack - xoffset);
            }

            match k {
                term::Key::Control(c) => {
                    if c == 'D' && self.command_buffer.len() == 0 {
                        print!("exit");
                        self.exit = true;
                        return false;
                    }
                    if c == 'C' {
                        print!("^{}", c);
                        self.command_buffer.clear();
                        return false;
                    }
                }
                term::Key::Newline => return false,
                term::Key::Escape => (),
                term::Key::Delete => {
                    if self.command_buffer.len() > 0 {
                        term::ansi::erase_line(term::ansi::ClearType::AfterCursor);
                        if xoffset == 0 {
                            self.command_buffer.pop();
                        } else {
                            self.command_buffer.remove(backtrack - xoffset - 1);
                        }
                    }
                }
                term::Key::Ascii(c) => {
                    if xoffset == 0 {
                        self.command_buffer.push(c);
                    } else {
                        self.command_buffer.insert(backtrack - xoffset, c);
                    }
                }
                term::Key::Arrow(d) => match d {
                    term::ArrowDirection::Up => if hist_index != 0 {
                        hist_index -= 1;
                        term::ansi::erase_line(term::ansi::ClearType::AfterCursor);
                        self.command_buffer = self.history[hist_index].clone();
                    },
                    term::ArrowDirection::Down => if self.history.len() > hist_index + 1 {
                        hist_index += 1;
                        term::ansi::erase_line(term::ansi::ClearType::AfterCursor);
                        self.command_buffer = self.history[hist_index].clone();
                    },
                    term::ArrowDirection::Left => if xoffset > 0 {
                        xoffset -= 1
                    },
                    term::ArrowDirection::Right => if xoffset < backtrack {
                        xoffset += 1
                    },
                },
                term::Key::Invalid(_) => print!("\u{FFFD}"),
            };
            let toks = parser::split_words(&self.command_buffer);
            if toks.len() > 0 {
                let name = environ.compile_word(&toks[0]).unwrap();
                let namelen = name.len();
                if environ.find_executable(name).is_err() {
                    term::xterm::kitty::set_underline(term::xterm::kitty::Underline::Curly);
                    term::xterm::kitty::set_underline_color(1);
                    print!("{}", &self.command_buffer[..namelen]);
                    term::xterm::kitty::set_underline(term::xterm::kitty::Underline::None);
                    print!("{}", &self.command_buffer[namelen..]);
                } else {
                    print!("{}", self.command_buffer);
                }
            }

            if xoffset != 0 {
                term::ansi::cursor_left(xoffset);
            }
            io::stdout().flush();
            true
        })?;

        Ok(self.command_buffer.clone())
    }

    pub fn exit_requested(&self) -> bool {
        self.exit
    }
}
