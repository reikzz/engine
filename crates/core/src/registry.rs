use std::collections::HashMap;
use std::hash::Hash;

pub trait IdentifyableDef {
    type Id: Copy + Eq + Hash;
    fn key(&self) -> &str;
    fn from_index(index: usize) -> Self::Id;
    fn to_index(id: Self::Id) -> usize;
}

#[derive(Debug)]
pub struct Registry<Id, Def> {
    defs: Vec<Def>,
    id_map: HashMap<String, Id>,
}

impl<Id, Def> Default for Registry<Id, Def> {
    fn default() -> Self {
        Self {
            defs: Vec::new(),
            id_map: HashMap::new(),
        }
    }
}

impl<Id, Def> Registry<Id, Def>
where
    Def: IdentifyableDef<Id = Id>,
    Id: Copy + Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: Def) -> Result<Id, String> {
        let key = def.key().to_string();
        if self.id_map.contains_key(&key) {
            return Err(format!("Duplicate key in registry: {}", key));
        }

        let id = Def::from_index(self.defs.len());
        self.id_map.insert(key, id);
        self.defs.push(def);

        Ok(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id, &Def)> {
        self.defs
            .iter()
            .enumerate()
            .map(|(idx, def)| (Def::from_index(idx), def))
    }

    pub fn values(&self) -> std::slice::Iter<'_, Def> {
        self.defs.iter()
    }

    #[inline]
    pub fn get_id(&self, key: &str) -> Option<Id> {
        self.id_map.get(key).copied()
    }

    #[inline]
    pub fn get(&self, id: Id) -> Option<&Def> {
        self.defs.get(Def::to_index(id))
    }

    #[inline]
    pub fn all(&self) -> &[Def] {
        &self.defs
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.defs.len()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
