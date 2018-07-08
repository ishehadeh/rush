use shell::ast;
use std::collections::btree_map;
use std::collections::BTreeMap;

pub type Name = String;
pub type Value<'a> = ast::Function<'a>;
pub type Iter<'a, 'b> = btree_map::Iter<'b, Name, Value<'a>>;
pub type IterMut<'a, 'b> = btree_map::IterMut<'b, Name, Value<'a>>;
pub type IntoIter<'a> = btree_map::IntoIter<Name, Value<'a>>;

#[derive(Debug, Clone)]
pub struct Functions<'a> {
    map: BTreeMap<Name, Value<'a>>,
}
pub enum Entry<'a> {
    Vacant(VacantEntry<'a>),
    Occupied(OccupiedEntry<'a>),
}

#[derive(Debug)]
pub struct OccupiedEntry<'a> {
    entry: btree_map::OccupiedEntry<'a, Name, Value<'a>>,
}

#[derive(Debug)]
pub struct VacantEntry<'a> {
    entry: btree_map::VacantEntry<'a, Name, Value<'a>>,
}

impl<'a> Functions<'a> {
    pub fn new() -> Functions<'a> {
        Functions {
            map: BTreeMap::new(),
        }
    }

    pub fn remove(&mut self, k: &Name) {
        self.map.remove(k);
    }

    pub fn value(&self, k: &Name) -> Option<Value<'a>> {
        self.map.get(k).map(|v| v.clone())
    }

    pub fn exists(&self, k: &Name) -> bool {
        self.map.contains_key(k)
    }

    pub fn entry<T: Into<Name>>(&'a mut self, key: T) -> Entry<'a> {
        match self.map.entry(key.into()) {
            btree_map::Entry::Occupied(v) => Entry::Occupied(OccupiedEntry { entry: v }),
            btree_map::Entry::Vacant(v) => Entry::Vacant(VacantEntry { entry: v }),
        }
    }

    pub fn iter<'b>(&'b self) -> Iter<'a, 'b> {
        self.map.iter()
    }

    pub fn iter_mut<'b>(&'b mut self) -> IterMut<'a, 'b> {
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

    pub fn get_mut(&mut self) -> &mut Value<'a> {
        self.entry.get_mut()
    }

    pub fn remove_entry(self) -> (Name, Value<'a>) {
        self.entry.remove_entry()
    }

    pub fn remove(self) -> Value<'a> {
        self.entry.remove()
    }

    pub fn into_mut(self) -> &'a mut Value<'a> {
        self.entry.into_mut()
    }

    pub fn insert<T: Into<Value<'a>>>(mut self, value: T) -> Value<'a> {
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

    pub fn insert<T: Into<Value<'a>>>(self, value: T) -> &'a mut Value<'a> {
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
        F: FnOnce(&mut Value<'a>),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    pub fn or_insert<T: Into<Value<'a>>>(self, default: T) -> &'a mut Value<'a> {
        match self {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(default.into()),
        }
    }

    pub fn insert<T: Into<Value<'a>>>(self, value: T) -> &'a mut Value<'a> {
        match self {
            Entry::Occupied(e) => {
                let mutref = e.into_mut();
                *mutref = value.into();
                mutref
            }
            Entry::Vacant(e) => e.insert(value.into()),
        }
    }

    pub fn remove(self) -> Option<Value<'a>> {
        match self {
            Entry::Occupied(e) => Some(e.remove()),
            Entry::Vacant(_) => None,
        }
    }

    pub fn or_insert_with<T, F>(self, default: F) -> &'a mut Value<'a>
    where
        T: Into<Value<'a>>,
        F: FnOnce() -> T,
    {
        match self {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(default().into()),
        }
    }
}

impl<'a> IntoIterator for Functions<'a> {
    type IntoIter = IntoIter<'a>;
    type Item = (Name, Value<'a>);

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
