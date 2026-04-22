    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ArchiverFlavor, WindowsToolchainRoots, archiver_flavor_from_override,
        find_program_in_directories, missing_archiver_error, missing_clang_hint,
        windows_llvm_bin_dirs_from_roots,
    };

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary toolchain test directory");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn archiver_override_style_can_force_lib_flavor_for_wrappers() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/tmp/mock-archiver.cmd"), Some("lib")),
            ArchiverFlavor::Lib
        );
    }

    #[test]
    fn archiver_override_style_can_force_ar_flavor_for_wrappers() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/tmp/mock-archiver.cmd"), Some("ar")),
            ArchiverFlavor::Ar
        );
    }

    #[test]
    fn archiver_override_falls_back_to_program_name_inference() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/LLVM/bin/llvm-lib.exe"), None),
            ArchiverFlavor::Lib
        );
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/LLVM/bin/llvm-ar.exe"), None),
            ArchiverFlavor::Ar
        );
    }

    #[test]
    fn find_program_in_directories_returns_matching_candidate() {
        let temp = TempDir::new("ql-driver-toolchain-find");
        let bin = temp.path.join("bin");
        fs::create_dir_all(&bin).expect("create fake bin dir");
        let clang = bin.join("clang.cmd");
        fs::write(&clang, "@echo off\r\n").expect("write fake clang wrapper");

        let found = find_program_in_directories(&["clang.exe", "clang.cmd"], &[bin]);

        assert_eq!(found, Some(clang));
    }

    #[test]
    fn windows_llvm_bin_dirs_cover_common_install_roots_without_duplicates() {
        let roots = WindowsToolchainRoots {
            scoop: Some(PathBuf::from("C:/Scoop")),
            user_profile: Some(PathBuf::from("C:/Users/alice")),
            local_appdata: Some(PathBuf::from("C:/Users/alice/AppData/Local")),
            program_files: Some(PathBuf::from("C:/Program Files")),
            program_files_x86: Some(PathBuf::from("C:/Program Files (x86)")),
        };

        let dirs = windows_llvm_bin_dirs_from_roots(&roots);

        assert!(dirs.contains(&PathBuf::from("C:/Scoop/apps/llvm/current/bin")));
        assert!(dirs.contains(&PathBuf::from("C:/Users/alice/scoop/apps/llvm/current/bin")));
        assert!(dirs.contains(&PathBuf::from(
            "C:/Users/alice/AppData/Local/Programs/LLVM/bin"
        )));
        assert!(dirs.contains(&PathBuf::from("C:/Program Files/LLVM/bin")));
        assert!(dirs.contains(&PathBuf::from("C:/Program Files (x86)/LLVM/bin")));
        assert_eq!(dirs.len(), 5);
    }

    #[cfg(windows)]
    #[test]
    fn missing_clang_hint_mentions_windows_candidates() {
        let hint = missing_clang_hint();

        assert!(hint.contains("QLANG_CLANG"));
        assert!(hint.contains("Scoop users can install LLVM with `scoop install llvm`"));
        assert!(hint.contains("clang.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn missing_archiver_hint_mentions_windows_candidates() {
        let error = missing_archiver_error();
        let rendered = error.to_string();

        assert!(rendered.contains("QLANG_AR"));
        assert!(rendered.contains("QLANG_AR_STYLE=lib|ar"));
        assert!(rendered.contains("llvm-lib.exe") || rendered.contains("llvm-ar.exe"));
    }
