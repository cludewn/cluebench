// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! These tests use real local files. Faults simulate failures at explicit I/O
//! boundaries; they do not fill a disk or demonstrate power-loss durability.

use super::*;
use std::fs;
use std::time::{Duration, Instant};

fn committed(outcome: WriteOutcome) {
    if cfg!(windows) {
        assert_eq!(outcome, WriteOutcome::CommittedDurabilityUncertain);
    } else {
        assert_eq!(outcome, WriteOutcome::Saved);
    }
}

fn temporary_files(dir: &Path) -> Vec<std::path::PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".cluebench-")
        })
        .collect()
}

struct Inject(&'static [Step]);
impl Faults for Inject {
    fn check(&self, step: Step) -> Result<(), FsError> {
        if self.0.contains(&step) {
            Err(FsError::Io)
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_file_create_open_replace_round_trip_and_no_replace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    let mut new = FileBinding::new(&path).unwrap();
    committed(new.write(b"synthetic ciphertext one"));
    assert!(temporary_files(dir.path()).is_empty());
    assert!(matches!(
        FileBinding::new(&path),
        Err(FsError::AlreadyExists)
    ));
    let (mut open, bytes) = FileBinding::open(&path).unwrap();
    assert_eq!(bytes, b"synthetic ciphertext one");
    assert_eq!(open.read_unchanged().unwrap(), bytes);
    committed(open.write(b"synthetic ciphertext two"));
    assert_eq!(open.read_unchanged().unwrap(), b"synthetic ciphertext two");
    committed(open.write(b"synthetic ciphertext three"));
    assert_eq!(fs::read(&path).unwrap(), b"synthetic ciphertext three");
}

#[test]
fn same_size_ciphertext_edit_and_identity_change_are_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"AAA").unwrap();
    let (mut binding, _) = FileBinding::open(&path).unwrap();
    fs::write(&path, b"BBB").unwrap();
    assert_eq!(
        binding.read_unchanged().err(),
        Some(FsError::ExternalChange)
    );
    assert_eq!(
        binding.write(b"CCC"),
        WriteOutcome::NotCommitted(FsError::ExternalChange)
    );
    assert_eq!(fs::read(&path).unwrap(), b"BBB");
    // Keep the old object alive so the OS cannot recycle its inode/FileId.
    let held = fs::File::open(&path).unwrap();
    let (mut binding2, _) = FileBinding::open(&path).unwrap();
    fs::rename(&path, dir.path().join("old.age")).unwrap();
    fs::write(&path, b"BBB").unwrap();
    assert_eq!(
        binding2.read_unchanged().err(),
        Some(FsError::ExternalChange)
    );
    assert_eq!(
        binding2.write(b"DDD"),
        WriteOutcome::NotCommitted(FsError::ExternalChange)
    );
    assert_eq!(fs::read(&path).unwrap(), b"BBB");
    drop(held);
}

#[test]
fn new_destination_that_appears_after_selection_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    let mut binding = FileBinding::new(&path).unwrap();
    fs::write(&path, b"external").unwrap();
    assert_eq!(
        binding.write(b"replacement"),
        WriteOutcome::NotCommitted(FsError::ExternalChange)
    );
    assert_eq!(fs::read(&path).unwrap(), b"external");
}

struct CreateAtRename(std::path::PathBuf);
impl Faults for CreateAtRename {
    fn check(&self, step: Step) -> Result<(), FsError> {
        if step == Step::Rename {
            fs::write(&self.0, b"external at commit").unwrap();
        }
        Ok(())
    }
}

#[test]
fn atomic_no_replace_preserves_destination_created_after_final_check() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    let mut binding = FileBinding::new(&path).unwrap();
    assert!(matches!(
        binding.write_with(b"replacement", &CreateAtRename(path.clone())),
        WriteOutcome::NotCommitted(FsError::ExternalChange | FsError::Io)
    ));
    assert_eq!(fs::read(path).unwrap(), b"external at commit");
    assert!(temporary_files(dir.path()).is_empty());
}

#[test]
fn parent_directory_handle_remains_pinned_after_path_is_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("parent");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("synthetic.age");
    let mut binding = FileBinding::new(&path).unwrap();
    let moved = dir.path().join("moved");
    fs::rename(&parent, &moved).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::write(&path, b"unrelated external").unwrap();
    committed(binding.write(b"pinned ciphertext"));
    assert_eq!(
        fs::read(moved.join("synthetic.age")).unwrap(),
        b"pinned ciphertext"
    );
    assert_eq!(fs::read(path).unwrap(), b"unrelated external");
}

#[test]
fn bounded_reads_and_writes_enforce_exact_ciphertext_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    let mut binding = FileBinding::new(&path).unwrap();
    let maximum = vec![b'x'; MAX_FILE_BYTES];
    committed(binding.write(&maximum));
    assert_eq!(FileBinding::open(&path).unwrap().1.len(), MAX_FILE_BYTES);
    assert_eq!(
        binding.write(&vec![b'x'; MAX_FILE_BYTES + 1]),
        WriteOutcome::NotCommitted(FsError::TooLarge)
    );
    fs::write(&path, vec![b'x'; MAX_FILE_BYTES + 1]).unwrap();
    assert!(matches!(FileBinding::open(&path), Err(FsError::TooLarge)));
}

#[test]
fn injected_precommit_failures_preserve_old_file_and_clean_own_temp() {
    for step in [
        Step::Write,
        Step::Flush,
        Step::FileSync,
        Step::Check,
        Step::Rename,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.age");
        fs::write(&path, b"old synthetic ciphertext").unwrap();
        let (mut binding, _) = FileBinding::open(&path).unwrap();
        let failure = Inject(match step {
            Step::Write => &[Step::Write],
            Step::Flush => &[Step::Flush],
            Step::FileSync => &[Step::FileSync],
            Step::Check => &[Step::Check],
            Step::Rename => &[Step::Rename],
            _ => unreachable!(),
        });
        assert_eq!(
            binding.write_with(b"new synthetic ciphertext", &failure),
            WriteOutcome::NotCommitted(FsError::Io)
        );
        assert_eq!(fs::read(&path).unwrap(), b"old synthetic ciphertext");
        assert!(temporary_files(dir.path()).is_empty());
        assert_eq!(
            binding.read_unchanged().unwrap(),
            b"old synthetic ciphertext"
        );
    }
}

#[test]
fn injected_cleanup_failure_leaves_only_ciphertext_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"old synthetic ciphertext").unwrap();
    let (mut binding, _) = FileBinding::open(&path).unwrap();
    assert_eq!(
        binding.write_with(
            b"new synthetic ciphertext",
            &Inject(&[Step::Flush, Step::Cleanup])
        ),
        WriteOutcome::NotCommitted(FsError::Io)
    );
    assert_eq!(fs::read(&path).unwrap(), b"old synthetic ciphertext");
    let files = temporary_files(dir.path());
    assert_eq!(files.len(), 1);
    assert_eq!(fs::read(&files[0]).unwrap(), b"new synthetic ciphertext");
}

#[test]
fn directory_sync_failure_is_committed_and_unknown_outcome_poisoned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"old").unwrap();
    let (mut binding, _) = FileBinding::open(&path).unwrap();
    assert_eq!(
        binding.write_with(b"visible new", &Inject(&[Step::DirectorySync])),
        WriteOutcome::CommittedDurabilityUncertain
    );
    assert_eq!(fs::read(&path).unwrap(), b"visible new");
    assert_eq!(binding.read_unchanged().unwrap(), b"visible new");
    assert_eq!(
        binding.write_with(b"possibly committed", &Inject(&[Step::AfterRename])),
        WriteOutcome::OutcomeUnknown
    );
    assert_eq!(fs::read(&path).unwrap(), b"possibly committed");
    assert_eq!(
        binding.read_unchanged().err(),
        Some(FsError::OutcomeUnknown)
    );
    assert_eq!(
        binding.write(b"must not retry"),
        WriteOutcome::OutcomeUnknown
    );
    assert_eq!(fs::read(&path).unwrap(), b"possibly committed");
}

#[test]
fn rejects_nonregular_targets_and_relative_paths() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        FileBinding::open(dir.path()),
        Err(FsError::InvalidTarget | FsError::Io)
    ));
    assert!(matches!(
        FileBinding::new(Path::new("relative.age")),
        Err(FsError::InvalidTarget)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_symlink_targets_parent_symlinks_and_unsupported_filesystem() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"ciphertext").unwrap();
    let link = dir.path().join("link.age");
    symlink(&path, &link).unwrap();
    assert!(matches!(
        FileBinding::open(&link),
        Err(FsError::InvalidTarget)
    ));
    let linkdir = dir.path().join("linkdir");
    symlink(dir.path(), &linkdir).unwrap();
    assert!(matches!(
        FileBinding::open(&linkdir.join("synthetic.age")),
        Err(FsError::InvalidTarget)
    ));
    assert!(matches!(
        FileBinding::new(Path::new("/proc/cluebench-synthetic.age")),
        Err(FsError::UnsupportedFilesystem)
    ));
    let fifo = dir.path().join("fifo");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(matches!(
        FileBinding::open(&fifo),
        Err(FsError::InvalidTarget)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn encrypted_temp_install_uses_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    let mut binding = FileBinding::new(&path).unwrap();
    committed(binding.write(b"ciphertext"));
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(target_os = "linux")]
#[test]
fn nonsticky_shared_writable_parents_are_rejected_but_trusted_sticky_is_admitted() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    for mode in [0o770, 0o777] {
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(mode)).unwrap();
        assert!(matches!(
            FileBinding::new(&path),
            Err(FsError::InvalidTarget)
        ));
    }
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o750)).unwrap();
    assert!(FileBinding::new(&path).is_ok());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o1777)).unwrap();
    let mut binding = FileBinding::new(&path).unwrap();
    committed(binding.write(b"sticky-protected ciphertext"));
    assert_eq!(
        binding.read_unchanged().unwrap(),
        b"sticky-protected ciphertext"
    );
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(target_os = "linux")]
struct UnprotectParent(std::path::PathBuf);
#[cfg(target_os = "linux")]
impl Faults for UnprotectParent {
    fn check(&self, step: Step) -> Result<(), FsError> {
        use std::os::unix::fs::PermissionsExt;
        if step == Step::Rename {
            fs::set_permissions(&self.0, fs::Permissions::from_mode(0o777)).unwrap();
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[test]
fn changed_parent_permissions_are_rechecked_before_write_install_and_cleanup() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"old ciphertext").unwrap();
    let (mut binding, _) = FileBinding::open(&path).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(
        binding.write(b"new ciphertext"),
        WriteOutcome::NotCommitted(FsError::InvalidTarget)
    );
    assert!(temporary_files(dir.path()).is_empty());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        binding.write_with(b"new ciphertext", &UnprotectParent(dir.path().into())),
        WriteOutcome::NotCommitted(FsError::InvalidTarget)
    );
    assert_eq!(fs::read(&path).unwrap(), b"old ciphertext");
    let files = temporary_files(dir.path());
    assert_eq!(files.len(), 1);
    assert_eq!(fs::read(&files[0]).unwrap(), b"new ciphertext");
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(binding.read_unchanged().unwrap(), b"old ciphertext");
}

#[cfg(windows)]
#[test]
fn rejects_ads_unc_and_reserved_windows_names() {
    for path in [
        r"C:\synthetic.age:stream",
        r"\\server\share\synthetic.age",
        r"\\?\UNC\server\share\synthetic.age",
        r"C:\CON.age",
    ] {
        assert!(matches!(
            FileBinding::new(Path::new(path)),
            Err(FsError::InvalidTarget)
        ));
    }
}

struct Interrupt;
impl Faults for Interrupt {
    fn check(&self, step: Step) -> Result<(), FsError> {
        if step == Step::Check {
            let dir = std::env::var_os("CLUEBENCH_TEST_INTERRUPT_DIR").unwrap();
            fs::write(Path::new(&dir).join("ready"), b"ready").unwrap();
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Ok(())
    }
}

#[test]
#[ignore = "subprocess helper for the process-interruption integration test"]
fn interrupted_child() {
    let dir = std::env::var_os("CLUEBENCH_TEST_INTERRUPT_DIR").unwrap();
    let path = Path::new(&dir).join("synthetic.age");
    let (mut binding, _) = FileBinding::open(&path).unwrap();
    let _ = binding.write_with(b"new synthetic ciphertext", &Interrupt);
}

#[test]
fn process_kill_before_commit_preserves_old_file_and_ciphertext_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.age");
    fs::write(&path, b"old synthetic ciphertext").unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "project_fs::tests::interrupted_child",
        ])
        .env("CLUEBENCH_TEST_INTERRUPT_DIR", dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.path().join("ready").exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("interruption helper exited before its checkpoint");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let ready = dir.path().join("ready").exists();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(ready, "interruption helper did not reach its checkpoint");
    assert_eq!(fs::read(path).unwrap(), b"old synthetic ciphertext");
    let files = temporary_files(dir.path());
    assert_eq!(files.len(), 1);
    assert_eq!(fs::read(&files[0]).unwrap(), b"new synthetic ciphertext");
}

#[test]
fn filesystem_errors_are_fixed_nonsecret_codes() {
    for (error, code) in [
        (
            FsError::UnsupportedFilesystem,
            "PROJECT_UNSUPPORTED_FILESYSTEM",
        ),
        (FsError::InvalidTarget, "PROJECT_UNSAFE_DESTINATION"),
        (FsError::AlreadyExists, "PROJECT_DESTINATION_EXISTS"),
        (FsError::TooLarge, "PROJECT_BUDGET_EXCEEDED"),
        (FsError::ExternalChange, "PROJECT_EXTERNAL_CHANGE"),
        (FsError::Io, "PROJECT_IO"),
        (FsError::OutcomeUnknown, "PROJECT_OUTCOME_UNKNOWN"),
    ] {
        assert_eq!(error.code(), code);
    }
}
