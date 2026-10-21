//! Descriptor-relative, private staging and atomic no-replace publication.
//!
//! Rustix supplies safe wrappers for renameat2(RENAME_NOREPLACE) on Linux and
//! renameatx_np(RENAME_EXCL) on macOS. There is no ordinary-rename fallback.
//! All generated entries are single components below held directory handles.
//! Cleanup removes only entries whose recorded device/inode identity still
//! matches. Those identity checks followed by unlinkat are not atomic, so
//! cleanup does not protect against arbitrary concurrent hostile replacement
//! of filesystem entries; the parent must be trusted. Exclusive creation
//! protects against ordinary collisions. If a just-created directory cannot be
//! opened and identified immediately after creation, cleanup has no trusted
//! identity to compare and leaves it in place rather than removing a possibly
//! replaced entry.

use std::{
    ffi::OsString,
    fs::File,
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rustix::{
    fd::OwnedFd,
    fs::{self, AtFlags, Mode, OFlags, RenameFlags, Stat},
};

use crate::bundle::PreparedBundle;

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::CLOEXEC);
const PRIVATE_DIRECTORY: Mode = Mode::RWXU;
const PRIVATE_FILE: Mode = Mode::RUSR.union(Mode::WUSR);
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    device: i128,
    inode: u128,
}

impl Identity {
    fn of(stat: Stat) -> Self {
        Self {
            device: i128::from(stat.st_dev),
            inode: u128::from(stat.st_ino),
        }
    }
}

struct Entry {
    name: OsString,
    identity: Identity,
}

struct Staging {
    parent: OwnedFd,
    name: OsString,
    identity: Option<Identity>,
    directory: Option<OwnedFd>,
    sources: Option<OwnedFd>,
    sources_entry: Option<Entry>,
    manifest: Option<Entry>,
    source_files: Vec<Entry>,
    armed: bool,
}

pub(super) fn publish(destination: &Path, prepared: &PreparedBundle) -> io::Result<()> {
    publish_with(destination, prepared, |file, contents| {
        file.write_all(contents)?;
        file.sync_all()
    })
}

fn publish_with(
    destination: &Path,
    prepared: &PreparedBundle,
    mut write: impl FnMut(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let name = destination
        .file_name()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let parent_path = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Parent symlinks follow normal filesystem semantics; the opened parent
    // handle then anchors staging, cleanup and publication to one directory.
    let parent = fs::openat(fs::CWD, parent_path, DIRECTORY, Mode::empty())?;
    let mut staging = Staging::create_with(parent, || {
        // Even an unusually named destination must never itself be used as
        // staging: the final name becomes visible only at the commit point.
        loop {
            let candidate = staging_name();
            if candidate.as_os_str() != name {
                break candidate;
            }
        }
    })?;
    let result = (|| {
        staging.initialize()?;
        staging.write(prepared, &mut write)?;
        staging.check_identity()?;
        fs::renameat_with(
            &staging.parent,
            &staging.name,
            &staging.parent,
            name,
            RenameFlags::NOREPLACE,
        )?;
        // Commit point: no fallible operations follow, and Drop cannot remove
        // the published directory. Atomic visibility is not a power-loss guarantee.
        staging.armed = false;
        Ok(())
    })();
    if let Err(error) = result {
        // Explicit cleanup reports failure; Drop also retries best-effort.
        staging.cleanup()?;
        return Err(error);
    }
    Ok(())
}

fn staging_name() -> OsString {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
    format!(
        ".cribra-bundle-{}-{nonce:x}-{sequence:x}",
        std::process::id()
    )
    .into()
}

impl Staging {
    fn create_with(parent: OwnedFd, name: impl FnMut() -> OsString) -> io::Result<Self> {
        Self::create_with_stat(parent, name, |parent, name| {
            fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        })
    }

    fn create_with_stat(
        parent: OwnedFd,
        mut name: impl FnMut() -> OsString,
        mut statat: impl FnMut(&OwnedFd, &OsString) -> rustix::io::Result<Stat>,
    ) -> io::Result<Self> {
        for _ in 0..128 {
            let name = name();
            match fs::mkdirat(&parent, &name, PRIVATE_DIRECTORY) {
                Ok(()) => {
                    let mut staging = Self {
                        parent,
                        name,
                        identity: None,
                        directory: None,
                        sources: None,
                        sources_entry: None,
                        manifest: None,
                        source_files: Vec::new(),
                        armed: true,
                    };
                    // Open immediately after exclusive creation and anchor the
                    // identity in the directory descriptor. If the later name
                    // lookup fails, cleanup can compare against this recorded
                    // identity instead of adopting a replacement.
                    let directory = fs::openat(
                        &staging.parent,
                        &staging.name,
                        DIRECTORY | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )?;
                    staging.identity = Some(Identity::of(fs::fstat(&directory)?));
                    match statat(&staging.parent, &staging.name) {
                        Ok(stat) if Some(Identity::of(stat)) == staging.identity => {
                            return Ok(staging);
                        }
                        Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
                        Err(error) => {
                            // Cleanup verifies against the recorded identity
                            // and reports its own failure rather than masking it.
                            staging.cleanup()?;
                            return Err(error.into());
                        }
                    }
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::ErrorKind::AlreadyExists.into())
    }

    fn initialize(&mut self) -> io::Result<()> {
        // Verify against the identity recorded at creation; never adopt the
        // identity of whatever currently occupies the staging name.
        self.check_identity()?;
        let directory = fs::openat(
            &self.parent,
            &self.name,
            DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )?;
        if Some(Identity::of(fs::fstat(&directory)?)) != self.identity {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.directory = Some(directory);
        Ok(())
    }

    fn directory(&self) -> io::Result<&OwnedFd> {
        self.directory
            .as_ref()
            .ok_or_else(|| io::ErrorKind::InvalidData.into())
    }

    fn check_identity(&self) -> io::Result<()> {
        let current = Identity::of(fs::statat(
            &self.parent,
            &self.name,
            AtFlags::SYMLINK_NOFOLLOW,
        )?);
        if Some(current) != self.identity {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }

    fn write(
        &mut self,
        prepared: &PreparedBundle,
        write: &mut impl FnMut(&mut File, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let (mut manifest, entry) = create_new(self.directory()?, "manifest.json")?;
        self.manifest = Some(entry);
        write(&mut manifest, prepared.manifest_json.as_bytes())?;
        fs::mkdirat(self.directory()?, "sources", PRIVATE_DIRECTORY)?;
        self.sources_entry = Some(Entry {
            name: "sources".into(),
            identity: Identity::of(fs::statat(
                self.directory()?,
                "sources",
                AtFlags::SYMLINK_NOFOLLOW,
            )?),
        });
        self.sources = Some(fs::openat(
            self.directory()?,
            "sources",
            DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        let sources_identity = Identity::of(fs::fstat(
            self.sources.as_ref().ok_or(io::ErrorKind::InvalidData)?,
        )?);
        if self.sources_entry.as_ref().map(|entry| entry.identity) != Some(sources_identity) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        for (index, source) in prepared.sources.iter().enumerate() {
            let expected = format!("sources/{:04}.txt", index + 1);
            if source.output_path != expected {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let name = &source.output_path["sources/".len()..];
            let directory = self.sources.as_ref().ok_or(io::ErrorKind::InvalidData)?;
            let (mut file, entry) = create_new(directory, name)?;
            self.source_files.push(entry);
            write(&mut file, source.content.as_bytes())?;
        }
        Ok(())
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        // Creation records identity before any fallible initialization step, so
        // an incomplete initialization can still be cleaned up. If the name no
        // longer matches that identity, leave it in place and report failure.
        self.check_identity()?;
        let mut failure = None;
        let mut record = |result: io::Result<()>| {
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
        };
        if let Some(sources) = &self.sources {
            for file in &self.source_files {
                record(remove_owned(sources, file, AtFlags::empty()));
            }
        }
        if let Some(directory) = &self.directory {
            if let Some(manifest) = &self.manifest {
                record(remove_owned(directory, manifest, AtFlags::empty()));
            }
            if let Some(sources) = &self.sources_entry {
                record(remove_owned(directory, sources, AtFlags::REMOVEDIR));
            }
        }
        record(fs::unlinkat(&self.parent, &self.name, AtFlags::REMOVEDIR).map_err(Into::into));
        if let Some(error) = failure {
            return Err(error);
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn create_new(directory: &OwnedFd, name: &str) -> io::Result<(File, Entry)> {
    // Descriptor-relative create_new(true): CREATE|EXCL is atomic exclusive
    // creation, while NOFOLLOW additionally forbids a final-component symlink.
    let fd = fs::openat(
        directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        PRIVATE_FILE,
    )?;
    let identity = Identity::of(fs::fstat(&fd)?);
    Ok((
        File::from(fd),
        Entry {
            name: name.into(),
            identity,
        },
    ))
}

fn remove_owned(parent: &OwnedFd, entry: &Entry, flags: AtFlags) -> io::Result<()> {
    // This rejects observed identity mismatches, but statat followed by unlinkat
    // cannot protect against arbitrary concurrent hostile entry replacement.
    match fs::statat(parent, &entry.name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if Identity::of(stat) == entry.identity => {
            fs::unlinkat(parent, &entry.name, flags).map_err(Into::into)
        }
        Ok(_) => Err(io::ErrorKind::InvalidData.into()),
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::PreparedSource;

    fn root() -> std::path::PathBuf {
        for _ in 0..128 {
            let path = std::env::temp_dir().join(staging_name());
            match std::fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary fixture directory should be created: {error}"),
            }
        }
        panic!("temporary fixture directory should be available");
    }

    #[test]
    fn cleanup_recovers_from_controlled_stat_failure_during_creation() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut failed = false;
        let error = Staging::create_with_stat(parent, staging_name, |parent, name| {
            if !failed {
                failed = true;
                return Err(rustix::io::Errno::PERM);
            }
            fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        })
        .err()
        .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn creation_stat_failure_reports_cleanup_failure_without_removing_replacement() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let staged_name: OsString = ".cribra-bundle-controlled".into();
        let replacement_name = staged_name.clone();
        let mut replaced = false;
        let error = Staging::create_with_stat(
            parent,
            || staged_name.clone(),
            |parent, name| {
                if !replaced {
                    replaced = true;
                    fs::renameat_with(
                        parent,
                        name,
                        parent,
                        "owned-staging",
                        RenameFlags::NOREPLACE,
                    )
                    .unwrap();
                    fs::mkdirat(parent, name, PRIVATE_DIRECTORY).unwrap();
                    std::fs::write(root.join(name).join("preexisting"), b"preserve").unwrap();
                    return Err(rustix::io::Errno::PERM);
                }
                fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            std::fs::read(root.join(replacement_name).join("preexisting")).unwrap(),
            b"preserve"
        );
        assert!(root.join("owned-staging").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_after_failed_initialization_removes_recorded_staging() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut stage = Staging::create_with(parent, staging_name).unwrap();
        // Creation records identity before initialization can fail.
        assert!(stage.identity.is_some());
        fs::renameat_with(
            &stage.parent,
            &stage.name,
            &stage.parent,
            "temporarily-moved",
            RenameFlags::NOREPLACE,
        )
        .unwrap();
        assert_eq!(
            stage.initialize().unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        fs::renameat_with(
            &stage.parent,
            "temporarily-moved",
            &stage.parent,
            &stage.name,
            RenameFlags::NOREPLACE,
        )
        .unwrap();
        stage.cleanup().unwrap();
        assert!(!stage.armed);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn incomplete_initialization_leaves_unverifiable_entries_and_reports_failure() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut stage = Staging::create_with(parent, staging_name).unwrap();
        fs::renameat_with(
            &stage.parent,
            &stage.name,
            &stage.parent,
            "temporarily-moved",
            RenameFlags::NOREPLACE,
        )
        .unwrap();
        assert!(stage.initialize().is_err());
        assert_eq!(stage.cleanup().unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(stage.identity.is_some());
        assert!(stage.armed);
        assert!(root.join("temporarily-moved").is_dir());
        // A replacement symlink must neither be followed nor removed, including
        // by Drop's best-effort retry. Its target predates the retry.
        std::fs::create_dir(root.join("preexisting")).unwrap();
        fs::symlinkat("preexisting", &stage.parent, &stage.name).unwrap();
        let name = stage.name.clone();
        assert!(stage.cleanup().is_err());
        drop(stage);
        assert!(
            std::fs::symlink_metadata(root.join(name))
                .unwrap()
                .is_symlink()
        );
        assert!(root.join("preexisting").is_dir());
        assert!(root.join("temporarily-moved").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_initialization_does_not_adopt_or_remove_replacement_directory() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut stage = Staging::create_with(parent, staging_name).unwrap();
        fs::renameat_with(
            &stage.parent,
            &stage.name,
            &stage.parent,
            "owned-staging",
            RenameFlags::NOREPLACE,
        )
        .unwrap();
        fs::mkdirat(&stage.parent, &stage.name, PRIVATE_DIRECTORY).unwrap();
        std::fs::write(root.join(&stage.name).join("preexisting"), b"preserve").unwrap();
        assert_eq!(
            stage.initialize().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            stage.cleanup().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let name = stage.name.clone();
        drop(stage);
        assert_eq!(
            std::fs::read(root.join(name).join("preexisting")).unwrap(),
            b"preserve"
        );
        assert!(root.join("owned-staging").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_write_failure_removes_all_owned_staging() {
        let root = root();
        let destination = root.join("export");
        let prepared = PreparedBundle {
            manifest_json: "{}\n".into(),
            sources: vec![PreparedSource {
                output_path: "sources/0001.txt".into(),
                content: "synthetic fixture".into(),
            }],
        };
        let mut calls = 0;
        let error = publish_with(&destination, &prepared, |file, bytes| {
            calls += 1;
            if calls == 2 {
                file.write_all(&bytes[..3])?;
                return Err(io::ErrorKind::WriteZero.into());
            }
            file.write_all(bytes)
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        assert!(!destination.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn collision_is_never_reused_or_removed() {
        let root = root();
        let collision = root.join(".cribra-bundle-collision");
        std::fs::create_dir(&collision).unwrap();
        std::fs::write(collision.join("preserve"), b"unchanged").unwrap();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut attempts = 0;
        let mut stage = Staging::create_with(parent, || {
            attempts += 1;
            if attempts == 1 {
                ".cribra-bundle-collision".into()
            } else {
                ".cribra-bundle-owned".into()
            }
        })
        .unwrap();
        stage.initialize().unwrap();
        stage.cleanup().unwrap();
        assert_eq!(
            std::fs::read(collision.join("preserve")).unwrap(),
            b"unchanged"
        );
        assert!(!root.join(".cribra-bundle-owned").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_failure_preserves_unowned_entries_and_is_reported() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut stage = Staging::create_with(parent, staging_name).unwrap();
        stage.initialize().unwrap();
        let (mut unrelated, _) = create_new(stage.directory().unwrap(), "unowned").unwrap();
        unrelated.write_all(b"preserve").unwrap();
        assert!(stage.cleanup().is_err());
        assert_eq!(
            std::fs::read(root.join(&stage.name).join("unowned")).unwrap(),
            b"preserve"
        );
        fs::unlinkat(stage.directory().unwrap(), "unowned", AtFlags::empty()).unwrap();
        stage.cleanup().unwrap();
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn cleanup_refuses_replaced_entry_identity() {
        let root = root();
        let parent = fs::openat(fs::CWD, &root, DIRECTORY, Mode::empty()).unwrap();
        let mut stage = Staging::create_with(parent, staging_name).unwrap();
        stage.initialize().unwrap();
        let (file, mut entry) = create_new(stage.directory().unwrap(), "entry").unwrap();
        // An observed identity mismatch prevents this unlink attempt.
        entry.identity.inode = entry.identity.inode.wrapping_add(1);
        assert!(remove_owned(stage.directory().unwrap(), &entry, AtFlags::empty()).is_err());
        assert!(root.join(&stage.name).join("entry").is_file());
        drop(file);
        fs::unlinkat(stage.directory().unwrap(), "entry", AtFlags::empty()).unwrap();
        stage.cleanup().unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
