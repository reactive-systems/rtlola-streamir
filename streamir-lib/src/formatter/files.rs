//! A framework allowing to easily add further constructs to the resulting files during compilation.
//! Further allows distributing the resulting code across different files.

use std::{
    collections::{BTreeMap, HashMap},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use thiserror::Error;

#[derive(Debug)]
/// A datastructure for holding the additional requirements during compilation.
/// Needs to be part of the formatter to be used.
pub struct ConstructStore<F: FilesFormatter>(Mutex<HashMap<PathBuf, FileConstructs<F>>>);

impl<F: FilesFormatter> Default for ConstructStore<F> {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[derive(Debug, Clone)]
struct FileConstructs<F: FilesFormatter>(BTreeMap<F::Key, String>);

impl<F: FilesFormatter> Default for FileConstructs<F> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

#[derive(Debug, Error)]
/// An error that can happen when writing the constructs into the corresponding files.
pub enum ConstructWriteError {
    #[error("The output file already exists: {0}")]
    /// The file already exists.
    AlreadyExist(PathBuf),
    #[error("IO Error")]
    /// Another io error happened.
    IO(#[from] std::io::Error),
}

/// A formatter that uses the [ConstructStore] to compile different files.
pub trait FilesFormatter
where
    Self: Sized,
{
    /// The key that is used for ordering the generated constructs inside each file.
    /// Often an enum with derived Ord.
    type Key: Ord + std::fmt::Debug;

    /// Returns a reference to the construct store.
    fn get_construct_store(&self) -> &ConstructStore<Self>;

    /// Adds a string requirement to the construct store (inside the given `file`, at the position of `key`)
    fn add_requirement_string<P: AsRef<Path>>(&self, file: P, key: Self::Key, requirement: String) {
        let store = self.get_construct_store();
        let mut store = store.0.lock().unwrap();
        if !store.contains_key(file.as_ref()) {
            store.insert(file.as_ref().to_owned(), FileConstructs::default());
        }
        let file = store.get_mut(file.as_ref()).unwrap();
        file.0.entry(key).or_insert(requirement);
    }

    /// Adds a requirement to the construct store (inside the given `file`)
    fn add_requirement<R: Requirement<Self>>(&self, requirement: R) {
        let key = requirement.key();
        let file = requirement.file(self);
        let string = requirement.format(self);
        self.add_requirement_string(file, key, string);
    }

    /// Whether the generation of the output files overwrites existing files
    fn overwrite(&self) -> bool {
        false
    }

    /// Consumes the formatter and generates the output files.
    fn generate_files(self) -> Result<(), ConstructWriteError> {
        let mut store = self.get_construct_store().0.lock().unwrap();
        for (path, content) in store.drain() {
            if path.exists() && !self.overwrite() {
                return Err(ConstructWriteError::AlreadyExist(path));
            }
            let file = std::fs::File::create(path)?;
            let writer = BufWriter::new(file);
            self.write_file(writer, content.0.into_values())?;
        }
        Ok(())
    }

    /// Write a single file
    fn write_file<W: Write>(
        &self,
        mut w: W,
        constructs: impl Iterator<Item = String>,
    ) -> Result<(), ConstructWriteError> {
        let mut is_first = true;
        for construct in constructs {
            if !is_first {
                writeln!(w)?;
            }
            write!(w, "{construct}")?;
            is_first = false;
        }
        Ok(())
    }
}

/// A trait representing any construct of the target language.
///
/// Can be implemented for Functions, Structs, etc.
pub trait Requirement<F: FilesFormatter> {
    /// The key of the requirement.
    fn key(&self) -> F::Key;

    /// The file where the requirement should be written into
    fn file(&self, formatter: &F) -> PathBuf;

    /// Format the requirement as a string.
    fn format(self, formatter: &F) -> String;
}
