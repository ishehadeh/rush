use self::ErrorKind::*;
use failure;
use nom;
use nom::le_u16;
use term::terminfo::fields::{BooleanField, NumericField, StringField};
use term::{Error, ErrorKind, Result};

const INVALID: u16 = 65535;

#[derive(Debug, Clone)]
pub struct TermHeader {
    names_size: usize,
    bools_size: usize,
    nums_size: usize,
    strings_size: usize,
    strtab_size: usize,
}

#[derive(Debug, Clone)]
pub struct ExtTermHeader {
    bools_size: usize,
    nums_size: usize,
    strings_size: usize,
    strtab_size: usize,
    strtab_end: usize,
}

#[derive(Debug, Clone)]
pub struct ExtendedTerm {
    bools: Vec<u8>,
    strings: Vec<u16>,
    numbers: Vec<u16>,
    custom_field_names: Vec<u16>,
    string_table: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Term {
    names: Vec<String>,

    bools: Vec<u8>,
    strings: Vec<u16>,
    numbers: Vec<u16>,

    string_table: Vec<u8>,
    extended: Option<ExtendedTerm>,
}

impl Term {
    pub fn name(&self) -> String {
        self.names
            .iter()
            .nth(0)
            .map(|s| s.clone())
            .unwrap_or(String::new())
    }

    pub fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    pub fn boolean(&self, field: BooleanField) -> bool {
        self.bools
            .iter()
            .nth(field as usize)
            .map(|&x| x != 0)
            .unwrap_or(false)
    }

    pub fn string(&self, field: StringField) -> Option<String> {
        match self.strings.iter().nth(field as usize) {
            Some(&v) => {
                if v == INVALID {
                    None
                } else {
                    Some(
                        self.string_table
                            .iter()
                            .skip(v as usize)
                            .take_while(|&&c| c != 0)
                            .map(|&c| c as char)
                            .collect(),
                    )
                }
            }
            None => None,
        }
    }

    pub fn str(&self, field: StringField) -> Option<&str> {
        match self.strings.iter().nth(field as usize) {
            Some(&v) => {
                if v == INVALID {
                    None
                } else {
                    let offset = v as usize;
                    let end = offset
                        + self.string_table
                            .iter()
                            .skip(offset)
                            .take_while(|&&c| c != 0)
                            .count();
                    Some(unsafe {
                        &*(&self.string_table[offset..end] as *const [u8] as *const str)
                    })
                }
            }
            None => None,
        }
    }

    pub fn number(&self, field: NumericField) -> Option<usize> {
        if let Some(&v) = self.numbers.iter().nth(field as usize) {
            if v == INVALID {
                None
            } else {
                Some(v as usize)
            }
        } else {
            None
        }
    }

    pub fn custom_field_name<T: AsRef<str>>(&self, s: T) -> Option<usize> {
        match &self.extended {
            Some(e) => {
                let bytes = s.as_ref().as_bytes();
                for (i, x) in e.custom_field_names.iter().enumerate() {
                    if e.string_table
                        .iter()
                        .skip(*x as usize)
                        .take_while(|&&c| c != 0)
                        .eq(bytes)
                    {
                        return Some(i);
                    }
                }
                None
            }
            None => None,
        }
    }

    pub fn ext_boolean<T: AsRef<str>>(&self, s: T) -> bool {
        match &self.extended {
            Some(e) => {
                let idx = match self.custom_field_name(s) {
                    Some(v) => v,
                    None => return false,
                };
                if idx >= e.bools.len() {
                    false
                } else {
                    e.bools[idx] != 0
                }
            }
            None => false,
        }
    }
    pub fn ext_number<T: AsRef<str>>(&self, s: T) -> Option<u16> {
        match &self.extended {
            Some(e) => {
                let idx = match self.custom_field_name(s) {
                    Some(v) => v,
                    None => return None,
                };
                if idx >= e.numbers.len() + e.bools.len() || idx <= e.bools.len()
                    || e.numbers[idx] == 0377
                {
                    None
                } else {
                    Some(e.numbers[idx])
                }
            }
            None => None,
        }
    }

    pub fn ext_string<T: AsRef<str>>(&self, s: T) -> Option<String> {
        match &self.extended {
            Some(e) => {
                let idx = match self.custom_field_name(s) {
                    Some(v) => v,
                    None => return None,
                };
                if idx >= e.strings.len() + e.numbers.len() + e.bools.len()
                    || idx <= e.bools.len() + e.numbers.len()
                    || e.strings[idx] == 0377
                    || e.strings[idx] as usize >= e.string_table.len()
                {
                    return None;
                }

                Some(
                    self.string_table
                        .iter()
                        .skip(e.strings[idx] as usize)
                        .take_while(|&&c| c != 0)
                        .map(|&c| c as char)
                        .collect(),
                )
            }
            None => None,
        }
    }

    pub fn ext_str<T: AsRef<str>>(&self, s: T) -> Option<&str> {
        match &self.extended {
            Some(e) => {
                let idx = match self.custom_field_name(s) {
                    Some(v) => v,
                    None => return None,
                };
                if idx >= e.strings.len() + e.numbers.len() + e.bools.len()
                    || idx <= e.bools.len() + e.numbers.len()
                    || e.strings[idx] == 0377
                    || e.strings[idx] as usize >= e.string_table.len()
                {
                    return None;
                }

                let offset = idx as usize;
                let end = offset
                    + self.string_table
                        .iter()
                        .skip(offset)
                        .take_while(|&&c| c != 0)
                        .count();
                Some(unsafe { &*(&self.string_table[offset..end] as *const [u8] as *const str) })
            }
            None => None,
        }
    }
}

#[rustfmt_skip]
named!(
    pub terminfo_ext<&[u8], ExtendedTerm, u32>,
    do_parse!(
        header: terminfo_ext_header >>
        bools: take!(header.bools_size) >>
        _padding: cond!(header.bools_size % 2 != 0, take!(1)) >>
        numbers: count!(le_u16, header.nums_size) >>
        strings: count!(le_u16, header.strings_size) >>
        names: count!(le_u16, header.strings_size + header.nums_size + header.bools_size) >>
        string_table: take!(header.strtab_end) >>
        ({
            let mut nametab_offset = 0;
            for x in &strings {
                nametab_offset += string_table.iter().skip(*x as usize).take_while(|&&c| c != 0).count();
            }

            ExtendedTerm {
                bools: Vec::from(bools),
                strings: strings,
                numbers: numbers,
            
                custom_field_names: names.iter().map(|x| *x  + nametab_offset as u16 - 1).collect(),
                string_table: Vec::from(string_table),

            }
        })
    )
);

#[rustfmt_skip]
named!(
    pub terminfo<&[u8], Term, u32>,
    do_parse!(
        header: terminfo_header >>
        names: terminfo_name_list >>
        bools: return_error!(ErrorKind::Custom(3), complete!(take!(header.bools_size))) >>
        _paddings: cond!((header.bools_size + header.names_size) % 2 != 0, take!(1)) >>
        numbers: return_error!(ErrorKind::Custom(4), complete!(count!(le_u16, header.nums_size))) >>
        strings: return_error!(ErrorKind::Custom(5), complete!(count!(le_u16, header.strings_size))) >>
        string_table: return_error!(ErrorKind::Custom(6), complete!(take!(header.strtab_size))) >>
        _padding2: cond!(header.strtab_size % 2 != 0, take!(1)) >>
        extended: opt!(terminfo_ext) >>
        ({
            Term {
                string_table: Vec::from(string_table),
                names: names,
                bools: Vec::from(bools),
                strings: strings,
                numbers: numbers,
                extended: extended, 
            }
        })
    )
);

#[rustfmt_skip]
named!(
    pub terminfo_header<&[u8], TermHeader, u32>,
    preceded!(
        // Check for the magic number, if it's not found bail out
        return_error!(ErrorKind::Custom(1), tag!(&[26, 1])),
        do_parse!(
            names: le_u16 >>
            bools: le_u16 >>
            nums: le_u16 >>
            strings: le_u16 >>
            strtab: le_u16 >>
            (TermHeader{
                names_size: names as usize,
                nums_size: nums as usize,
                bools_size: bools as usize,
                strings_size: strings as usize,
                strtab_size: strtab as usize,
            })
        )
    )
);

#[rustfmt_skip]
named!(
    pub terminfo_ext_header<&[u8], ExtTermHeader, u32>,
    do_parse!(
        bools: le_u16 >>
        nums: le_u16 >>
        strings: le_u16 >>
        strtab: le_u16 >>
        strtab_end: le_u16 >>
        (ExtTermHeader{
            nums_size: nums as usize,
            bools_size: bools as usize,
            strings_size: strings as usize,
            strtab_size: strtab as usize,
            strtab_end: strtab_end as usize,
        })
    )
);

#[rustfmt_skip]
named!(
    pub terminfo_name_list<&[u8], Vec<String>, u32>,
    terminated!(
        separated_list!(char!('|'),
            map!(return_error!(ErrorKind::Custom(2), complete!(take_until_either!("|\0"))), |v| v.iter().map(|&c| c as char).collect::<String>())),
        char!('\0')
    )
);
