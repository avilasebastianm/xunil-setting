//! Safe changes to files that belong to the user.
//!
//! A change is prepared first, so it can be shown as a diff, and written only when it is applied.
//! The state of a file before its first change is kept in a backup, and restoring it brings back
//! the same bytes and the same permissions.

use std::error::Error;
use std::fmt;
use std::fs::{self, DirBuilder, File};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use similar::TextDiff;
use tempfile::NamedTempFile;

const NEW_FILE_MODE: u32 = 0o600;
const BACKUP_DIR_MODE: u32 = 0o700;
const MODE_MASK: u32 = 0o7777;
const OWNER_WRITE: u32 = 0o200;
const DIFF_CONTEXT_LINES: usize = 3;
/// Keeps the files that existed before their first change.
const ORIGINALS_DIR: &str = "originals";
/// Keeps an empty marker for each file that did not exist before its first change.
const ABSENT_DIR: &str = "absent";

/// The reason a change could not be prepared or applied.
#[derive(Debug)]
pub enum ConfigError {
    /// The path is not absolute, has no file name, or contains `..`.
    InvalidTarget(PathBuf),
    /// The path is a directory, a broken link, or anything else that is not a regular file.
    NotAFile(PathBuf),
    /// The user removed the write permission from the file, so it is left alone.
    ReadOnly(PathBuf),
    /// The file is no longer what the prepared change was computed from.
    ChangedOnDisk(PathBuf),
    /// The file has no backup to restore.
    NoBackup(PathBuf),
    /// The operating system refused an operation on this path.
    Io {
        /// The path of the failed operation.
        path: PathBuf,
        /// The error reported by the operating system.
        source: io::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(path) => write!(formatter, "invalid path: {}", path.display()),
            Self::NotAFile(path) => write!(formatter, "not a regular file: {}", path.display()),
            Self::ReadOnly(path) => write!(formatter, "file is read-only: {}", path.display()),
            Self::ChangedOnDisk(path) => {
                write!(formatter, "file changed on disk: {}", path.display())
            }
            Self::NoBackup(path) => write!(formatter, "no backup for: {}", path.display()),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ConfigError {
    let path = path.to_path_buf();
    move |source| ConfigError::Io { path, source }
}

#[derive(Debug)]
enum Action {
    Edit(Vec<u8>),
    RestoreContent { content: Vec<u8>, mode: u32 },
    RestoreAbsence,
}

/// A change that has been computed but not written.
#[derive(Debug)]
pub struct Change {
    target: PathBuf,
    before: Option<Vec<u8>>,
    action: Action,
}

impl Change {
    /// Prepares replacing the content of `target`, which is created if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when `target` is not valid or cannot be read.
    pub fn edit(target: &Path, content: impl Into<Vec<u8>>) -> Result<Self, ConfigError> {
        normal_components(target)?;
        Ok(Self {
            target: target.to_path_buf(),
            before: read_current(target)?,
            action: Action::Edit(content.into()),
        })
    }

    /// Returns the file this change applies to.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Returns whether applying this change leaves the content of the file as it is.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.before.as_deref() == self.after()
    }

    /// Returns the change as a unified diff, empty when the content stays the same.
    ///
    /// Bytes that are not valid UTF-8 are shown with a replacement character. This only affects
    /// what the diff shows, not what gets written.
    #[must_use]
    pub fn diff(&self) -> String {
        let before = String::from_utf8_lossy(self.before.as_deref().unwrap_or_default());
        let after = String::from_utf8_lossy(self.after().unwrap_or_default());
        let name = self.target.display().to_string();
        TextDiff::from_lines(&before, &after)
            .unified_diff()
            .context_radius(DIFF_CONTEXT_LINES)
            .header(&name, &name)
            .to_string()
    }

    fn after(&self) -> Option<&[u8]> {
        match &self.action {
            Action::Edit(content) | Action::RestoreContent { content, .. } => Some(content),
            Action::RestoreAbsence => None,
        }
    }
}

/// Applies changes and keeps the backups that make them reversible.
#[derive(Debug)]
pub struct ConfigEditor {
    backup_dir: PathBuf,
}

impl ConfigEditor {
    /// Creates an editor that keeps its backups in `backup_dir`, created when first needed.
    #[must_use]
    pub fn new(backup_dir: PathBuf) -> Self {
        Self { backup_dir }
    }

    /// Returns whether `target` has a backup to restore.
    #[must_use]
    pub fn has_backup(&self, target: &Path) -> bool {
        normal_components(target).is_ok_and(|relative| {
            self.backup_dir.join(ORIGINALS_DIR).join(&relative).exists()
                || self.backup_dir.join(ABSENT_DIR).join(&relative).exists()
        })
    }

    /// Prepares bringing `target` back to its state before its first change.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::NoBackup`] when there is nothing to restore, and another
    /// [`ConfigError`] when `target` is not valid or a file cannot be read.
    pub fn preview_restore(&self, target: &Path) -> Result<Change, ConfigError> {
        let relative = normal_components(target)?;
        let original = self.backup_dir.join(ORIGINALS_DIR).join(&relative);
        let action = if original.exists() {
            Action::RestoreContent {
                content: fs::read(&original).map_err(io_error(&original))?,
                mode: mode_of(&original)?,
            }
        } else if self.backup_dir.join(ABSENT_DIR).join(&relative).exists() {
            Action::RestoreAbsence
        } else {
            return Err(ConfigError::NoBackup(target.to_path_buf()));
        };
        Ok(Change {
            target: target.to_path_buf(),
            before: read_current(target)?,
            action,
        })
    }

    /// Writes a prepared change.
    ///
    /// An edit saves a backup first if the file does not have one yet, and an edit that changes
    /// nothing writes nothing. A restore also discards the backup, so that the next edit saves a
    /// new one.
    ///
    /// The directory that holds the file must already exist: creating it could not be undone
    /// exactly.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::ChangedOnDisk`] when the file changed after the change was prepared,
    /// [`ConfigError::ReadOnly`] when an edit finds a file without write permission, and another
    /// [`ConfigError`] when an operation fails.
    pub fn apply(&self, change: &Change) -> Result<(), ConfigError> {
        let target = change.target();
        if read_current(target)? != change.before {
            return Err(ConfigError::ChangedOnDisk(target.to_path_buf()));
        }
        let relative = normal_components(target)?;
        match &change.action {
            Action::Edit(content) => self.apply_edit(change, content, &relative),
            Action::RestoreContent { content, mode } => {
                write_atomic(&resolve_links(target)?, content, *mode)?;
                remove_backup(&self.backup_dir.join(ORIGINALS_DIR).join(&relative))
            }
            Action::RestoreAbsence => {
                if change.before.is_some() {
                    fs::remove_file(target).map_err(io_error(target))?;
                    sync_parent(target)?;
                }
                remove_backup(&self.backup_dir.join(ABSENT_DIR).join(&relative))
            }
        }
    }

    fn apply_edit(
        &self,
        change: &Change,
        content: &[u8],
        relative: &Path,
    ) -> Result<(), ConfigError> {
        if change.is_noop() {
            return Ok(());
        }
        let target = change.target();
        let mode = if change.before.is_some() {
            let mode = mode_of(target)?;
            if mode & OWNER_WRITE == 0 {
                return Err(ConfigError::ReadOnly(target.to_path_buf()));
            }
            mode
        } else {
            let parent = parent_of(target)?;
            fs::metadata(parent).map_err(io_error(parent))?;
            NEW_FILE_MODE
        };
        if !self.has_backup(target) {
            self.save_backup(change, mode, relative)?;
        }
        write_atomic(&resolve_links(target)?, content, mode)
    }

    fn save_backup(&self, change: &Change, mode: u32, relative: &Path) -> Result<(), ConfigError> {
        let (directory, content) = match &change.before {
            Some(content) => (ORIGINALS_DIR, content.as_slice()),
            None => (ABSENT_DIR, [].as_slice()),
        };
        let path = self.backup_dir.join(directory).join(relative);
        create_backup_parent(&path)?;
        write_atomic(&path, content, mode)
    }
}

/// Returns the path without its root, which is where its backup goes inside a backup directory.
fn normal_components(target: &Path) -> Result<PathBuf, ConfigError> {
    let invalid = || ConfigError::InvalidTarget(target.to_path_buf());
    if !target.is_absolute() || target.file_name().is_none() {
        return Err(invalid());
    }
    let mut relative = PathBuf::new();
    for component in target.components() {
        match component {
            Component::Normal(name) => relative.push(name),
            Component::ParentDir => return Err(invalid()),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    Ok(relative)
}

/// Returns the content of the file, or `None` when nothing exists at that path.
fn read_current(target: &Path) -> Result<Option<Vec<u8>>, ConfigError> {
    match fs::metadata(target) {
        Ok(metadata) if metadata.is_file() => fs::read(target).map(Some).map_err(io_error(target)),
        Ok(_) => Err(ConfigError::NotAFile(target.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if fs::symlink_metadata(target).is_ok() {
                Err(ConfigError::NotAFile(target.to_path_buf()))
            } else {
                Ok(None)
            }
        }
        Err(error) => Err(io_error(target)(error)),
    }
}

/// Follows links, because replacing a link with a regular file would break dotfile managers.
fn resolve_links(target: &Path) -> Result<PathBuf, ConfigError> {
    match fs::canonicalize(target) {
        Ok(resolved) => Ok(resolved),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(target.to_path_buf()),
        Err(error) => Err(io_error(target)(error)),
    }
}

fn mode_of(path: &Path) -> Result<u32, ConfigError> {
    let metadata = fs::metadata(path).map_err(io_error(path))?;
    Ok(metadata.permissions().mode() & MODE_MASK)
}

fn parent_of(path: &Path) -> Result<&Path, ConfigError> {
    path.parent()
        .ok_or_else(|| ConfigError::InvalidTarget(path.to_path_buf()))
}

fn create_backup_parent(path: &Path) -> Result<(), ConfigError> {
    let parent = parent_of(path)?;
    DirBuilder::new()
        .recursive(true)
        .mode(BACKUP_DIR_MODE)
        .create(parent)
        .map_err(io_error(parent))
}

fn remove_backup(path: &Path) -> Result<(), ConfigError> {
    fs::remove_file(path).map_err(io_error(path))?;
    sync_parent(path)
}

/// Writes through a temporary file in the same directory, so the file is never left half written.
fn write_atomic(path: &Path, content: &[u8], mode: u32) -> Result<(), ConfigError> {
    let parent = parent_of(path)?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(io_error(parent))?;
    temporary.write_all(content).map_err(io_error(path))?;
    let file = temporary.as_file();
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(io_error(path))?;
    file.sync_all().map_err(io_error(path))?;
    temporary
        .persist(path)
        .map_err(|error| io_error(path)(error.error))?;
    sync_parent(path)
}

/// Makes a rename or a removal survive a power failure.
fn sync_parent(path: &Path) -> Result<(), ConfigError> {
    let parent = parent_of(path)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error(parent))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        root: TempDir,
        editor: ConfigEditor,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary directory");
            let editor = ConfigEditor::new(root.path().join("backups"));
            fs::create_dir(root.path().join("config")).expect("config directory");
            Self { root, editor }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.path().join("config").join(name)
        }

        fn existing(&self, name: &str, content: &[u8], mode: u32) -> PathBuf {
            let path = self.path(name);
            fs::write(&path, content).expect("write file");
            set_mode(&path, mode);
            path
        }

        fn edit(&self, target: &Path, content: &str) {
            let change = Change::edit(target, content).expect("prepare edit");
            self.editor.apply(&change).expect("apply edit");
        }

        fn restore(&self, target: &Path) {
            let change = self
                .editor
                .preview_restore(target)
                .expect("prepare restore");
            self.editor.apply(&change).expect("apply restore");
        }
    }

    fn set_mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set permissions");
    }

    fn entries(directory: &Path) -> Vec<PathBuf> {
        let mut entries: Vec<PathBuf> = fs::read_dir(directory)
            .expect("read directory")
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        entries
    }

    #[test]
    fn edit_replaces_the_content_and_leaves_no_temporary_files() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);

        fixture.edit(&target, "new\n");

        assert_eq!(fs::read(&target).expect("read"), b"new\n");
        assert_eq!(
            entries(target.parent().expect("parent")),
            std::slice::from_ref(&target)
        );
    }

    #[test]
    fn edit_keeps_the_permissions_of_the_file() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o640);

        fixture.edit(&target, "new\n");

        assert_eq!(mode_of(&target).expect("mode"), 0o640);
    }

    #[test]
    fn nothing_is_written_until_the_change_is_applied() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);

        let change = Change::edit(&target, "new\n").expect("prepare edit");

        assert_eq!(change.target(), target);
        assert_eq!(fs::read(&target).expect("read"), b"old\n");
        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn restore_brings_back_the_same_bytes_and_permissions() {
        let fixture = Fixture::new();
        let original = b"not utf-8: \xff\xfe\nno final newline";
        let target = fixture.existing("app.conf", original, 0o640);

        fixture.edit(&target, "new\n");
        set_mode(&target, 0o600);
        fixture.restore(&target);

        assert_eq!(fs::read(&target).expect("read"), original);
        assert_eq!(mode_of(&target).expect("mode"), 0o640);
    }

    #[test]
    fn backup_keeps_the_state_before_the_first_change_only() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"first\n", 0o644);

        fixture.edit(&target, "second\n");
        fixture.edit(&target, "third\n");
        fixture.restore(&target);

        assert_eq!(fs::read(&target).expect("read"), b"first\n");
    }

    #[test]
    fn restore_discards_the_backup_and_the_next_edit_saves_a_new_one() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"first\n", 0o644);

        fixture.edit(&target, "second\n");
        fixture.restore(&target);
        assert!(!fixture.editor.has_backup(&target));

        fs::write(&target, "edited by hand\n").expect("write");
        fixture.edit(&target, "third\n");
        fixture.restore(&target);
        assert_eq!(fs::read(&target).expect("read"), b"edited by hand\n");
    }

    #[test]
    fn new_file_is_private_and_restoring_removes_it() {
        let fixture = Fixture::new();
        let target = fixture.path("new.conf");

        fixture.edit(&target, "content\n");
        assert_eq!(mode_of(&target).expect("mode"), NEW_FILE_MODE);
        assert!(fixture.editor.has_backup(&target));

        fixture.restore(&target);
        assert!(!target.exists());
        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn restoring_a_new_file_the_user_already_removed_only_clears_the_backup() {
        let fixture = Fixture::new();
        let target = fixture.path("new.conf");

        fixture.edit(&target, "content\n");
        fs::remove_file(&target).expect("remove");
        fixture.restore(&target);

        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn edit_without_differences_writes_nothing_and_saves_no_backup() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"same\n", 0o644);

        let change = Change::edit(&target, "same\n").expect("prepare edit");
        assert!(change.is_noop());
        fixture.editor.apply(&change).expect("apply edit");

        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn edit_through_a_link_keeps_the_link_and_changes_the_real_file() {
        let fixture = Fixture::new();
        let real = fixture.existing("real.conf", b"old\n", 0o644);
        let link = fixture.path("link.conf");
        symlink(&real, &link).expect("create link");

        fixture.edit(&link, "new\n");
        assert!(fs::symlink_metadata(&link).expect("link").is_symlink());
        assert_eq!(fs::read(&real).expect("read"), b"new\n");

        fixture.restore(&link);
        assert!(fs::symlink_metadata(&link).expect("link").is_symlink());
        assert_eq!(fs::read(&real).expect("read"), b"old\n");
    }

    #[test]
    fn broken_link_is_rejected() {
        let fixture = Fixture::new();
        let link = fixture.path("link.conf");
        symlink(fixture.path("missing.conf"), &link).expect("create link");

        let error = Change::edit(&link, "new\n").expect_err("broken link");

        assert!(matches!(error, ConfigError::NotAFile(_)));
    }

    #[test]
    fn directory_is_rejected() {
        let fixture = Fixture::new();
        let directory = fixture.path("directory");
        fs::create_dir(&directory).expect("create directory");

        let error = Change::edit(&directory, "new\n").expect_err("directory");

        assert!(matches!(error, ConfigError::NotAFile(_)));
    }

    #[test]
    fn invalid_paths_are_rejected() {
        for path in ["relative.conf", "/", "/tmp/../etc/app.conf"] {
            let error = Change::edit(Path::new(path), "new\n").expect_err(path);
            assert!(matches!(error, ConfigError::InvalidTarget(_)), "{path}");
        }
    }

    #[test]
    fn invalid_path_never_has_a_backup() {
        let fixture = Fixture::new();

        assert!(!fixture.editor.has_backup(Path::new("/tmp/../etc/app.conf")));
    }

    #[test]
    fn edit_is_refused_when_the_file_changed_after_the_preview() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);
        let change = Change::edit(&target, "new\n").expect("prepare edit");

        fs::write(&target, "edited by hand\n").expect("write");
        let error = fixture.editor.apply(&change).expect_err("stale change");

        assert!(matches!(error, ConfigError::ChangedOnDisk(_)));
        assert_eq!(fs::read(&target).expect("read"), b"edited by hand\n");
        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn restore_is_refused_when_the_file_changed_after_the_preview() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);
        fixture.edit(&target, "new\n");
        let change = fixture.editor.preview_restore(&target).expect("prepare");

        fs::write(&target, "edited by hand\n").expect("write");
        let error = fixture.editor.apply(&change).expect_err("stale change");

        assert!(matches!(error, ConfigError::ChangedOnDisk(_)));
        assert!(fixture.editor.has_backup(&target));
    }

    #[test]
    fn read_only_file_is_left_alone() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o444);
        let change = Change::edit(&target, "new\n").expect("prepare edit");

        let error = fixture.editor.apply(&change).expect_err("read-only");

        assert!(matches!(error, ConfigError::ReadOnly(_)));
        assert_eq!(fs::read(&target).expect("read"), b"old\n");
        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn restore_without_a_backup_is_an_error() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);

        let error = fixture.editor.preview_restore(&target).expect_err("backup");

        assert!(matches!(error, ConfigError::NoBackup(_)));
    }

    #[test]
    fn new_file_in_a_missing_directory_is_an_error_and_saves_no_backup() {
        let fixture = Fixture::new();
        let target = fixture.path("missing/app.conf");
        let change = Change::edit(&target, "new\n").expect("prepare edit");

        let error = fixture
            .editor
            .apply(&change)
            .expect_err("missing directory");

        assert!(matches!(error, ConfigError::Io { .. }));
        assert!(error.source().is_some());
        assert!(!fixture.editor.has_backup(&target));
    }

    #[test]
    fn backups_are_private() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);

        fixture.edit(&target, "new\n");

        let backups = fixture.root.path().join("backups");
        assert_eq!(mode_of(&backups).expect("mode"), BACKUP_DIR_MODE);
        assert_eq!(
            mode_of(&backups.join(ORIGINALS_DIR)).expect("mode"),
            BACKUP_DIR_MODE
        );
    }

    #[test]
    fn diff_shows_the_lines_that_change() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"keep\nold\n", 0o644);

        let diff = Change::edit(&target, "keep\nnew\n")
            .expect("prepare")
            .diff();

        assert!(diff.contains("-old\n"), "{diff}");
        assert!(diff.contains("+new\n"), "{diff}");
        assert!(diff.contains(" keep\n"), "{diff}");
        assert!(diff.contains(&target.display().to_string()), "{diff}");
    }

    #[test]
    fn diff_of_a_new_file_only_adds_lines() {
        let fixture = Fixture::new();
        let target = fixture.path("new.conf");

        let diff = Change::edit(&target, "line\n").expect("prepare").diff();

        assert!(diff.contains("+line\n"), "{diff}");
    }

    #[test]
    fn diff_of_a_restore_shows_the_way_back() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"old\n", 0o644);
        fixture.edit(&target, "new\n");

        let diff = fixture
            .editor
            .preview_restore(&target)
            .expect("prepare")
            .diff();

        assert!(diff.contains("-new\n"), "{diff}");
        assert!(diff.contains("+old\n"), "{diff}");
    }

    #[test]
    fn diff_without_differences_is_empty() {
        let fixture = Fixture::new();
        let target = fixture.existing("app.conf", b"same\n", 0o644);

        let change = Change::edit(&target, "same\n").expect("prepare");

        assert_eq!(change.diff(), "");
    }

    #[test]
    fn errors_describe_the_path() {
        let path = PathBuf::from("/some/file");
        let errors = [
            ConfigError::InvalidTarget(path.clone()),
            ConfigError::NotAFile(path.clone()),
            ConfigError::ReadOnly(path.clone()),
            ConfigError::ChangedOnDisk(path.clone()),
            ConfigError::NoBackup(path.clone()),
            io_error(&path)(io::Error::from(io::ErrorKind::NotFound)),
        ];

        for error in errors {
            assert!(error.to_string().contains("/some/file"), "{error}");
        }
    }
}
