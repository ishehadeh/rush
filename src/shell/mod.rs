use crate::lang;
use failure;
use lang::ast::Command;
use lang::parser;
use lang::word::Word;
use nixterm;
use nixterm::events::Key;
use std::ffi::OsString;
use std::io;
use std::io::Write;

pub struct Shell {
    command_buffer: String,
    old_settings: nixterm::term::Settings,
    term: nixterm::Term<io::Stdin, io::Stdout>,
    history: Vec<String>,
    exit: bool,
}

impl Shell {
    pub fn new() -> nixterm::Result<Shell> {
        let t = nixterm::Term::new()?;
        Ok(Shell {
            command_buffer: String::new(),
            history: Vec::new(),
            exit: false,
            old_settings: t.settings(),
            term: t,
        })
    }

    fn print_error<T: failure::Fail>(e: T) {
        match e.cause() {
            Some(v) => println!("{}: {}", e, v),
            None => println!("{}", e),
        }
    }

    pub fn run(&mut self, ec: &mut lang::ExecutionContext, jm: &mut lang::JobManager) {
        while !self.exit_requested() {
            let prefix_command = ec
                .variables()
                .value(&OsString::from("RUSH_PROMPT"))
                .to_string_lossy()
                .to_string();

            match jm.run(
                ec,
                if prefix_command.is_empty() {
                    Command::simple(
                        ["printf", "'rush-%s$ '", "$RUSH_VERSION"]
                            .iter()
                            .map(|w| Word::parse(w))
                            .collect(),
                    )
                } else {
                    Command::from(prefix_command)
                },
            ) {
                Err(e) => Shell::print_error(e),
                _ => (),
            }

            let buffer = match self.readline(ec) {
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
                    match jm.run(ec, Command::from(buffer)) {
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

    pub fn readline(&mut self, environ: &mut lang::ExecutionContext) -> nixterm::Result<String> {
        self.command_buffer.clear();

        let mut hist_index = self.history.len();
        let mut xoffset: isize = 0;
        self.term.update(self.old_settings.clone().raw()).unwrap();

        for k in self.term.read_keys() {
            let backtrack = self.command_buffer.len() as isize;
            self.term
                .writer()
                .shift_cursor(xoffset - backtrack, 0)
                .done();

            match k? {
                Key::Control(c) => {
                    if c == 'D' && self.command_buffer.len() == 0 {
                        self.term
                            .writer()
                            .print(&self.command_buffer)
                            .print("exit")
                            .done();
                        self.exit = true;
                        break;
                    }
                    if c == 'C' {
                        self.term
                            .writer()
                            .print(&self.command_buffer)
                            .print("^C")
                            .done();
                        self.command_buffer.clear();
                        break;
                    }
                }
                Key::Enter => break,
                Key::Escape => self.command_buffer.push_str("^["),
                Key::Delete => {
                    if self.command_buffer.len() > 0 {
                        if xoffset == 0 {
                            self.command_buffer.pop();
                        } else {
                            self.command_buffer
                                .remove((backtrack - xoffset - 1) as usize);
                        }
                    }
                }
                Key::Char(c) => {
                    if xoffset == 0 {
                        self.command_buffer.push(c);
                    } else {
                        self.command_buffer
                            .insert((backtrack - xoffset) as usize, c);
                    }
                }
                Key::Up => {
                    if hist_index != 0 {
                        hist_index -= 1;
                        self.command_buffer = self.history[hist_index].clone();
                    }
                }
                Key::Down => {
                    if self.history.len() > hist_index + 1 {
                        hist_index += 1;
                        self.command_buffer = self.history[hist_index].clone();
                    }
                }
                Key::Left if xoffset < backtrack => xoffset += 1,
                Key::Right if xoffset > 0 => xoffset -= 1,
                _ => (),
            };

            self.term
                .writer()
                .print(&self.command_buffer)
                .print(self.term.info.string(nixterm::terminfo::ClrEol).unwrap())
                .shift_cursor(-xoffset, 0)
                .done()
                .unwrap();
        }

        self.term.update(self.old_settings.clone());
        Ok(self.command_buffer.clone())
    }

    pub fn exit_requested(&self) -> bool {
        self.exit
    }
}
