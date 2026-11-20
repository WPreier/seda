//! Build-time embedded documentation and reloadable in-memory site trees.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use thiserror::Error;
use walkdir::WalkDir;

use crate::config::{EmbeddedDocsConfig, SedaConfig, SiteConfig};

include!(concat!(env!("OUT_DIR"), "/embedded_content.rs"));

/// An immutable response body held entirely in memory.
#[derive(Clone, Debug)]
pub struct Content {
    bytes: Arc<[u8]>,
    mime: Arc<str>,
}

impl Content {
    /// Raw response bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// MIME value suitable for a `Content-Type` header.
    pub fn mime(&self) -> &str {
        &self.mime
    }
}

#[derive(Debug)]
struct LoadedSite {
    prefix: String,
    files: HashMap<String, Content>,
}

/// Hybrid content store. Reads never access the filesystem.
#[derive(Debug)]
pub struct ContentStore {
    embedded: HashMap<String, Content>,
    not_found: Content,
    sites: RwLock<Vec<LoadedSite>>,
}

/// Fully materialized site replacement, ready for an infallible swap.
#[derive(Debug)]
pub struct PreparedSites(Vec<LoadedSite>);

/// Failure while materializing configured content into memory.
#[derive(Debug, Error)]
pub enum ContentError {
    #[error("site root `{root}` contains a parent (`..`) component")]
    ParentTraversal { root: PathBuf },
    #[error("site root `{root}` is not a directory")]
    NotDirectory { root: PathBuf },
    #[error("site `{prefix}` duplicates an existing mount")]
    DuplicateMount { prefix: String },
    #[error("site entry `{path}` escapes canonical root `{root}`")]
    SymlinkEscape { path: PathBuf, root: PathBuf },
    #[error("site path `{path}` is not valid UTF-8")]
    NonUtf8Path { path: PathBuf },
    #[error("configured site content exceeds max_content_bytes ({limit} bytes)")]
    ContentLimit { limit: u64 },
    #[error("failed to walk site root: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("I/O error for `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl ContentStore {
    /// Load embedded docs and all configured site roots.
    pub fn load(config: &SedaConfig) -> Result<Self, ContentError> {
        Self::new(
            &config.embedded_docs,
            &config.sites,
            config.limits.max_content_bytes,
        )
    }

    /// Construct a store from the content-related config sections.
    pub fn new(
        docs: &EmbeddedDocsConfig,
        sites: &[SiteConfig],
        max_content_bytes: u64,
    ) -> Result<Self, ContentError> {
        let (embedded, not_found) = load_embedded(docs);
        let loaded_sites = load_sites(sites, max_content_bytes)?;
        Ok(Self {
            embedded,
            not_found,
            sites: RwLock::new(loaded_sites),
        })
    }

    /// Resolve a URL path. A matching site mount shadows less-specific mounts.
    pub fn lookup(&self, path: &str) -> Option<Content> {
        let path = normalize_request_path(path)?;
        let sites = self
            .sites
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(site) = sites
            .iter()
            .find(|site| path_has_prefix(path, &site.prefix))
        {
            return lookup_file(&site.files, path);
        }
        lookup_file(&self.embedded, path)
    }

    /// Embedded 404 response body.
    pub fn not_found(&self) -> Content {
        self.not_found.clone()
    }

    /// Re-read all optional site roots, then atomically replace the old set.
    ///
    /// If any site fails validation or loading, currently served sites remain
    /// unchanged.
    pub fn reload_sites(
        &self,
        sites: &[SiteConfig],
        max_content_bytes: u64,
    ) -> Result<(), ContentError> {
        let replacement = load_sites(sites, max_content_bytes)?;
        let mut current = self
            .sites
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current = replacement;
        Ok(())
    }

    /// Materialize a reload without changing the currently served content.
    pub fn prepare_sites(
        &self,
        sites: &[SiteConfig],
        max_content_bytes: u64,
    ) -> Result<PreparedSites, ContentError> {
        load_sites(sites, max_content_bytes).map(PreparedSites)
    }

    /// Atomically install a previously materialized site set.
    pub fn commit_sites(&self, replacement: PreparedSites) {
        let mut current = self
            .sites
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current = replacement.0;
    }

    /// Number of embedded and configured files, excluding generated aliases.
    pub fn len(&self) -> usize {
        let sites = self
            .sites
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.embedded.len() + sites.iter().map(|site| site.files.len()).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn load_embedded(config: &EmbeddedDocsConfig) -> (HashMap<String, Content>, Content) {
    let mut files = HashMap::new();
    let mut not_found = None;
    for &(relative, mime, bytes) in EMBEDDED_FILES {
        let content = Content {
            bytes: Arc::from(bytes),
            mime: Arc::from(mime),
        };
        if relative == "__404.html" {
            not_found = Some(content);
            continue;
        }
        if !config.enabled {
            continue;
        }
        let (collection, path) = relative
            .split_once('/')
            .expect("generated embedded path has a collection");
        let mount = match collection {
            "sedat" => &config.sedat_path,
            "seda" => &config.seda_path,
            _ => continue,
        };
        insert_file_and_index_aliases(&mut files, mount, path, content);
    }
    (
        files,
        not_found.expect("build script always embeds assets/404.html"),
    )
}

fn load_sites(
    configs: &[SiteConfig],
    max_content_bytes: u64,
) -> Result<Vec<LoadedSite>, ContentError> {
    let mut loaded = Vec::with_capacity(configs.len());
    let mut prefixes = HashSet::new();
    let mut total_bytes = 0_u64;
    for config in configs {
        if !prefixes.insert(config.path.clone()) {
            return Err(ContentError::DuplicateMount {
                prefix: config.path.clone(),
            });
        }
        loaded.push(load_site(config, &mut total_bytes, max_content_bytes)?);
    }
    loaded.sort_by(|left, right| {
        right
            .prefix
            .len()
            .cmp(&left.prefix.len())
            .then_with(|| left.prefix.cmp(&right.prefix))
    });
    Ok(loaded)
}

fn load_site(
    config: &SiteConfig,
    total_bytes: &mut u64,
    max_content_bytes: u64,
) -> Result<LoadedSite, ContentError> {
    let root = PathBuf::from(&config.root);
    if root
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(ContentError::ParentTraversal { root });
    }
    let canonical_root = canonicalize(&root)?;
    if !canonical_root.is_dir() {
        return Err(ContentError::NotDirectory { root });
    }

    let mut files = HashMap::new();
    for item in WalkDir::new(&root).follow_links(true) {
        let item = item?;
        let canonical = canonicalize(item.path())?;
        if !canonical.starts_with(&canonical_root) {
            return Err(ContentError::SymlinkEscape {
                path: item.path().to_path_buf(),
                root: canonical_root,
            });
        }
        if !item.file_type().is_file() {
            continue;
        }
        let relative = item
            .path()
            .strip_prefix(&root)
            .expect("walked item remains below root");
        let relative = url_relative_path(relative)?;
        let bytes = fs::read(item.path()).map_err(|source| ContentError::Io {
            path: item.path().to_path_buf(),
            source,
        })?;
        *total_bytes =
            total_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(ContentError::ContentLimit {
                    limit: max_content_bytes,
                })?;
        if *total_bytes > max_content_bytes {
            return Err(ContentError::ContentLimit {
                limit: max_content_bytes,
            });
        }
        let mime = mime_for(item.path());
        insert_file_and_index_aliases(
            &mut files,
            &config.path,
            &relative,
            Content {
                bytes: Arc::from(bytes),
                mime: Arc::from(mime),
            },
        );
    }
    Ok(LoadedSite {
        prefix: config.path.clone(),
        files,
    })
}

fn canonicalize(path: &Path) -> Result<PathBuf, ContentError> {
    fs::canonicalize(path).map_err(|source| ContentError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn url_relative_path(path: &Path) -> Result<String, ContentError> {
    let mut output = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                output.push(
                    value
                        .to_str()
                        .ok_or_else(|| ContentError::NonUtf8Path {
                            path: path.to_path_buf(),
                        })?
                        .to_owned(),
                );
            }
            Component::CurDir => {}
            _ => {
                return Err(ContentError::ParentTraversal {
                    root: path.to_path_buf(),
                });
            }
        }
    }
    Ok(output.join("/"))
}

fn insert_file_and_index_aliases(
    files: &mut HashMap<String, Content>,
    mount: &str,
    relative: &str,
    content: Content,
) {
    let route = join_route(mount, relative);
    files.insert(route.clone(), content.clone());
    if relative == "index.html" {
        files.insert(mount.to_owned(), content.clone());
        files.insert(format!("{mount}/"), content);
    } else if let Some(directory) = route.strip_suffix("/index.html") {
        files.insert(directory.to_owned(), content.clone());
        files.insert(format!("{directory}/"), content);
    }
}

fn lookup_file(files: &HashMap<String, Content>, path: &str) -> Option<Content> {
    files.get(path).cloned()
}

fn normalize_request_path(path: &str) -> Option<&str> {
    if !path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.split('/').any(|part| part == "..")
    {
        return None;
    }
    Some(path.split(['?', '#']).next().unwrap_or(path))
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    prefix == "/"
        || path == prefix
        || (path.starts_with(prefix) && path.as_bytes().get(prefix.len()).copied() == Some(b'/'))
}

fn join_route(mount: &str, relative: &str) -> String {
    if mount == "/" {
        format!("/{relative}")
    } else {
        format!("{mount}/{relative}")
    }
}

fn mime_for(path: &Path) -> String {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if mime.type_() == mime_guess::mime::TEXT
        || matches!(
            mime.subtype().as_str(),
            "javascript" | "json" | "xml" | "svg"
        )
    {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn embedded_docs_are_gated_and_have_index_aliases() {
        let enabled = ContentStore::new(&EmbeddedDocsConfig::default(), &[], 1024).unwrap();
        assert!(enabled.lookup("/sedat").is_some());
        assert!(enabled.lookup("/seda/docs/ARCHITECTURE.html").is_some());

        let disabled = ContentStore::new(
            &EmbeddedDocsConfig {
                enabled: false,
                ..EmbeddedDocsConfig::default()
            },
            &[],
            1024,
        )
        .unwrap();
        assert!(disabled.lookup("/sedat").is_none());
        assert!(!disabled.not_found().bytes().is_empty());
    }

    #[test]
    fn longest_site_prefix_wins() {
        let directory = tempdir().unwrap();
        let broad = directory.path().join("broad");
        let narrow = directory.path().join("narrow");
        fs::create_dir_all(&broad).unwrap();
        fs::create_dir_all(&narrow).unwrap();
        fs::write(broad.join("index.html"), "broad").unwrap();
        fs::write(narrow.join("index.html"), "narrow").unwrap();
        let sites = vec![
            SiteConfig {
                path: "/site".into(),
                root: broad.to_string_lossy().into_owned(),
            },
            SiteConfig {
                path: "/site/admin".into(),
                root: narrow.to_string_lossy().into_owned(),
            },
        ];
        let store = ContentStore::new(&EmbeddedDocsConfig::default(), &sites, 1024).unwrap();
        assert_eq!(store.lookup("/site").unwrap().bytes(), b"broad");
        assert_eq!(store.lookup("/site/admin").unwrap().bytes(), b"narrow");
    }

    #[test]
    fn root_site_matches_descendant_paths() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join("nested")).unwrap();
        fs::write(directory.path().join("nested/page.txt"), "root").unwrap();
        let sites = [SiteConfig {
            path: "/".into(),
            root: directory.path().to_string_lossy().into_owned(),
        }];
        let store = ContentStore::new(&EmbeddedDocsConfig::default(), &sites, 1024).unwrap();
        assert_eq!(store.lookup("/nested/page.txt").unwrap().bytes(), b"root");
    }

    #[test]
    fn failed_reload_keeps_existing_sites() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("index.html"), "old").unwrap();
        let initial = [SiteConfig {
            path: "/site".into(),
            root: directory.path().to_string_lossy().into_owned(),
        }];
        let store = ContentStore::new(&EmbeddedDocsConfig::default(), &initial, 1024).unwrap();
        let invalid = [SiteConfig {
            path: "/site".into(),
            root: "../escape".into(),
        }];
        assert!(store.reload_sites(&invalid, 1024).is_err());
        assert_eq!(store.lookup("/site").unwrap().bytes(), b"old");
    }
}
