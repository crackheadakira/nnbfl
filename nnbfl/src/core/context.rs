use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub(crate) type ContextValue = Box<dyn Any + Send + Sync>;

#[derive(Default)]
pub struct ContextStore {
    values: HashMap<TypeId, ContextValue>,
}

impl ContextStore {
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.values.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.values.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    pub(crate) fn replace<T: Any + Send + Sync>(&mut self, value: T) -> Option<ContextValue> {
        self.values.insert(TypeId::of::<T>(), Box::new(value))
    }

    pub(crate) fn restore<T: Any>(&mut self, previous: Option<ContextValue>) {
        let key = TypeId::of::<T>();

        if let Some(value) = previous {
            self.values.insert(key, value);
        } else {
            self.values.remove(&key);
        }
    }
}
