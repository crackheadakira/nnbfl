use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

use nnbfl::{
    core::FileReadWriteable,
    sarc::file::{MagicFiles, Sarc, SarcFile},
};

pub type EntryPath = Vec<usize>;

pub enum Node {
    Zstd {
        raw: Vec<u8>,
        inner: Box<Node>,
    },

    Yaz0 {
        raw: Vec<u8>,
        inner: Box<Node>,
    },

    Sarc {
        raw: Vec<u8>,
        sarc: Sarc,
        children: Vec<Node>,
    },

    Leaf {
        raw: Vec<u8>,
    },

    /// Compressed data that couldn't be decompressed
    Opaque {
        raw: Vec<u8>,
    },
}

impl Node {
    pub fn open(raw: Vec<u8>) -> Self {
        match raw.get(..4) {
            Some([0x28, 0xB5, 0x2F, 0xFD]) => {
                let mut decompressed = Vec::new();
                match tomolib::formats::zs::decompress(&raw[..], &mut decompressed) {
                    Ok(_) => Self::Zstd {
                        inner: Box::new(Self::open(decompressed)),
                        raw,
                    },
                    Err(_) => Self::Opaque { raw },
                }
            }
            Some(b"Yaz0") => match szs::decode(&raw) {
                Ok(decompressed) => Self::Yaz0 {
                    inner: Box::new(Self::open(decompressed)),
                    raw,
                },
                Err(_) => Self::Opaque { raw },
            },
            Some(b"SARC") => match Sarc::parse_file(&raw) {
                Ok(mut sarc) => {
                    let children = sarc
                        .files
                        .iter_mut()
                        .map(|file| Self::open(std::mem::take(&mut file.data)))
                        .collect();

                    Self::Sarc {
                        raw,
                        sarc,
                        children,
                    }
                }
                Err(_) => Self::Leaf { raw },
            },
            _ => Self::Leaf { raw },
        }
    }

    pub fn raw(&self) -> &[u8] {
        match self {
            Self::Zstd { raw, .. }
            | Self::Yaz0 { raw, .. }
            | Self::Sarc { raw, .. }
            | Self::Leaf { raw }
            | Self::Opaque { raw } => raw,
        }
    }

    pub fn at(&self, path: &[usize]) -> Option<&Node> {
        match (self, path) {
            (Self::Zstd { inner, .. } | Self::Yaz0 { inner, .. }, _) => inner.at(path),
            (_, []) => Some(self),
            (Self::Sarc { children, .. }, [first, rest @ ..]) => children.get(*first)?.at(rest),
            _ => None,
        }
    }

    pub fn rebuild(
        &self,
        path: &mut EntryPath,
        edits: &HashMap<EntryPath, Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, String> {
        match self {
            Self::Leaf { .. } => Ok(edits.get(path).cloned()),
            Self::Opaque { .. } => Ok(None),

            Self::Zstd { inner, .. } => {
                let Some(data) = inner.rebuild(path, edits)? else {
                    return Ok(None);
                };

                let mut compressed = Vec::new();

                tomolib::formats::zs::compress(
                    &data[..],
                    &mut compressed,
                    tomolib::formats::zs::DEFAULT_LEVEL,
                    Some(data.len() as u64),
                )
                .map_err(|e| format!("zstd compression failed: {e}"))?;

                Ok(Some(compressed))
            }

            Self::Yaz0 { raw, inner } => {
                let Some(data) = inner.rebuild(path, edits)? else {
                    return Ok(None);
                };

                let mut encoded = szs::encode(&data, szs::EncodeAlgo::MK8)
                    .map_err(|e| format!("Yaz0 compression failed: {e:?}"))?;

                if let (Some(dst), Some(src)) = (encoded.get_mut(8..12), raw.get(8..12)) {
                    dst.copy_from_slice(src);
                }

                Ok(Some(encoded))
            }

            Self::Sarc { sarc, children, .. } => {
                let mut changed = false;
                let mut files = Vec::with_capacity(children.len());

                for (i, (file, child)) in sarc.files.iter().zip(children).enumerate() {
                    path.push(i);
                    let rebuilt = child.rebuild(path, edits);

                    path.pop();

                    let data = match rebuilt? {
                        Some(data) => {
                            changed = true;
                            data
                        }
                        None => child.raw().to_vec(),
                    };

                    files.push(SarcFile {
                        data,
                        ..file.clone()
                    });
                }

                if !changed {
                    return Ok(None);
                }

                Sarc {
                    files,
                    ..sarc.clone()
                }
                .to_bytes()
                .map(Some)
                .map_err(|e| e.to_string())
            }
        }
    }

    pub fn flatten(
        &self,
        name: Option<String>,
        path: &mut EntryPath,
        out: &mut Vec<(EntryPath, MagicFiles)>,
    ) {
        match self {
            Self::Zstd { inner, .. } | Self::Yaz0 { inner, .. } => inner.flatten(name, path, out),

            Self::Sarc { sarc, children, .. } => {
                for (i, (file, child)) in sarc.files.iter().zip(children).enumerate() {
                    path.push(i);
                    child.flatten(file.name.clone(), path, out);
                    path.pop();
                }
            }

            Self::Leaf { raw } => {
                let file = SarcFile {
                    name,
                    hash: 0,
                    alignment: None,
                    data: raw.clone(),
                };

                out.push((path.clone(), file.match_by_magic()));
            }

            Self::Opaque { raw } => {
                out.push((path.clone(), MagicFiles::Unknown(name, raw.clone())))
            }
        }
    }
}

pub struct ArchiveDoc {
    pub path: PathBuf,
    pub save_path: Option<PathBuf>,
    pub root: Node,
    disk: (PathBuf, u64),
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl ArchiveDoc {
    pub fn file_name(&self) -> Option<String> {
        self.path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
    }

    pub fn files(&self, scope: &[usize]) -> Vec<(EntryPath, MagicFiles)> {
        let mut out = Vec::new();
        if let Some(node) = self.root.at(scope) {
            node.flatten(self.file_name(), &mut scope.to_vec(), &mut out);
        }

        out
    }

    pub fn save(
        &mut self,
        target: &Path,
        edits: &HashMap<EntryPath, Vec<u8>>,
    ) -> Result<(), String> {
        if target == self.disk.0
            && let Ok(current) = std::fs::read(target)
            && hash_bytes(&current) != self.disk.1
        {
            return Err(format!(
                "{} changed on disk since it was opened, use Save As",
                target.display()
            ));
        }

        let bytes = match self.root.rebuild(&mut Vec::new(), edits)? {
            Some(bytes) => bytes,
            None => self.root.raw().to_vec(),
        };

        let mut temporary = target.as_os_str().to_owned();
        temporary.push(".tmp");
        std::fs::write(&temporary, &bytes).map_err(|e| format!("Write error: {e}"))?;
        std::fs::rename(&temporary, target).map_err(|e| format!("Write error: {e}"))?;

        self.disk = (target.to_path_buf(), hash_bytes(&bytes));
        self.save_path = Some(target.to_path_buf());
        self.root = Node::open(bytes);

        Ok(())
    }
}

#[derive(Default)]
pub struct DocStore {
    pub docs: Vec<ArchiveDoc>,
}

impl DocStore {
    pub fn open(&mut self, path: &Path) -> Result<usize, String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(id) = self.docs.iter().position(|doc| doc.path == canonical) {
            return Ok(id);
        }

        let bytes = std::fs::read(&canonical).map_err(|e| format!("Failed to read file: {e}"))?;
        self.docs.push(ArchiveDoc {
            disk: (canonical.clone(), hash_bytes(&bytes)),
            path: canonical,
            save_path: None,
            root: Node::open(bytes),
        });

        Ok(self.docs.len() - 1)
    }
}

pub struct LayoutSource {
    pub doc: usize,
    pub layout: EntryPath,
}

pub fn file_stem(name: &str) -> &str {
    let base = name.rsplit('/').next().unwrap_or(name);
    base.split_once('.').map_or(base, |(stem, _)| stem)
}

pub fn anim_owner<'a>(
    anim_stem: &str,
    layouts: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    layouts
        .into_iter()
        .filter(|layout| {
            anim_stem == *layout
                || anim_stem
                    .strip_prefix(*layout)
                    .is_some_and(|rest| rest.starts_with('_'))
        })
        .max_by_key(|layout| layout.len())
}
