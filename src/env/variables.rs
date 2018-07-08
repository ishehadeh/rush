///! Variables is a wrapper around a `BTreeMap<OsString, OsString>`.
///! It provides specialized methods for working with shell variables.
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;

pub type Name = OsString;
pub type Value = OsString;

pub type Iter<'a> = btree_map::Iter<'a, Name, Value>;
pub type IterMut<'a> = btree_map::IterMut<'a, Name, Value>;
pub type IntoIter = btree_map::IntoIter<Name, Value>;

#[derive(Debug, Clone)]
pub struct Variables {
    map: BTreeMap<Name, Value>,
}

pub enum Entry<'a> {
    Vacant(VacantEntry<'a>),
    Occupied(OccupiedEntry<'a>),
}

#[derive(Debug)]
pub struct OccupiedEntry<'a> {
    entry: btree_map::OccupiedEntry<'a, Name, Value>,
}

#[derive(Debug)]
pub struct VacantEntry<'a> {
    entry: btree_map::VacantEntry<'a, Name, Value>,
}

impl Variables {
    pub fn new() -> Variables {
        Variables {
            map: BTreeMap::new(),
        }
    }

    pub fn from_env() -> Variables {
        Variables {
            map: env::vars_os().collect(),
        }
    }

    pub fn import_env(&mut self) {
        self.map.append(&mut env::vars_os().collect());
    }

    pub fn define<T: Into<OsString>, U: Into<OsString>>(&mut self, k: T, v: U) {
        self.map.insert(k.into(), v.into());
    }

    pub fn remove(&mut self, k: &OsString) {
        self.map.remove(k);
    }

    pub fn value(&self, k: &OsString) -> OsString {
        self.map
            .get(k)
            .map(|v| v.clone())
            .unwrap_or(OsString::new())
    }

    pub fn exists<T: Into<OsString>>(&self, k: &OsString) -> bool {
        self.map.contains_key(k)
    }

    pub fn has_value<T: Into<OsString>>(&self, k: &OsString) -> bool {
        self.map.get(k).map(|v| v.len() > 0).unwrap_or(false)
    }

    pub fn entry<'a, T: Into<Name>>(&'a mut self, key: T) -> Entry<'a> {
        match self.map.entry(key.into()) {
            btree_map::Entry::Occupied(v) => Entry::Occupied(OccupiedEntry { entry: v }),
            btree_map::Entry::Vacant(v) => Entry::Vacant(VacantEntry { entry: v }),
        }
    }

    pub fn export(&self, k: &OsString) {
        env::set_var(k, self.value(k));
    }

    pub fn iter<'a>(&'a self) -> Iter<'a> {
        self.map.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a> {
        self.map.iter_mut()
    }
}

impl<'a> OccupiedEntry<'a> {
    pub fn key(&self) -> &Name {
        self.entry.key()
    }

    pub fn get(&self) -> &Value {
        self.entry.get()
    }

    pub fn get_mut(&mut self) -> &mut Value {
        self.entry.get_mut()
    }

    pub fn remove_entry(self) -> (Name, Value) {
        self.entry.remove_entry()
    }

    pub fn remove(self) -> Value {
        self.entry.remove()
    }

    pub fn into_mut(self) -> &'a mut Value {
        self.entry.into_mut()
    }

    pub fn insert<T: Into<Value>>(mut self, value: T) -> Value {
        self.entry.insert(value.into())
    }
}

impl<'a> VacantEntry<'a> {
    pub fn key(&self) -> &Name {
        self.entry.key()
    }

    pub fn into_name(self) -> Name {
        self.entry.into_key()
    }

    pub fn insert<T: Into<Value>>(self, value: T) -> &'a mut Value {
        self.entry.insert(value.into())
    }
}

impl<'a> Entry<'a> {
    pub fn name(&self) -> &Name {
        match self {
            Entry::Occupied(e) => e.key(),
            Entry::Vacant(e) => e.key(),
        }
    }

    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut Value),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    pub fn or_insert<T: Into<Value>>(self, default: T) -> &'a mut Value {
        match self {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(default.into()),
        }
    }

    pub fn insert<T: Into<Value>>(self, value: T) -> &'a mut Value {
        match self {
            Entry::Occupied(e) => {
                let mutref = e.into_mut();
                *mutref = value.into();
                mutref
            }
            Entry::Vacant(e) => e.insert(value.into()),
        }
    }

    pub fn remove(self) -> Option<Value> {
        match self {
            Entry::Occupied(e) => Some(e.remove()),
            Entry::Vacant(_) => None,
        }
    }

    pub fn or_insert_null<T: Into<Value>>(self, default: T) -> &'a mut Value {
        match self {
            Entry::Occupied(e) => {
                let isnull = e.get().len() == 0;
                let mutref = e.into_mut();
                if isnull {
                    *mutref = default.into();
                }
                mutref
            }
            Entry::Vacant(e) => e.insert(default.into()),
        }
    }

    pub fn or_insert_with<T, F>(self, default: F) -> &'a mut Value
    where
        T: Into<Value>,
        F: FnOnce() -> T,
    {
        match self {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(default().into()),
        }
    }
}

impl IntoIterator for Variables {
    type IntoIter = IntoIter;
    type Item = (Name, Value);

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
