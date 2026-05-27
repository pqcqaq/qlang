use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_driver::{ArchiverFlavor, ArchiverInvocation, ProgramInvocation};

pub(super) struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub(super) fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create test parent directory");
        }
        fs::write(&path, contents).expect("write test file");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn mock_success_invocation(dir: &TestDir) -> ProgramInvocation {
    if cfg!(windows) {
        let script = dir.write(
            "mock-clang-success.ps1",
            r#"
$out = $null
$isCompile = $false
$isAssembly = $false
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-S') {
        $isAssembly = $true
    }
    if ($args[$i] -eq '-c') {
        $isCompile = $true
    }
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') {
        $isShared = $true
    }
    if ($args[$i] -eq '-o') {
        $out = $args[$i + 1]
    }
}
if ($null -eq $out) {
    Write-Error "missing -o"
    exit 1
}
if ($isAssembly) {
    Set-Content -Path $out -NoNewline -Value "mock-assembly"
} elseif ($isCompile) {
    Set-Content -Path $out -NoNewline -Value "mock-object"
} elseif ($isShared) {
    Set-Content -Path $out -NoNewline -Value "mock-dylib"
} else {
    Set-Content -Path $out -NoNewline -Value "mock-executable"
}
"#,
        );
        ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
            "-ExecutionPolicy".to_owned(),
            "Bypass".to_owned(),
            "-File".to_owned(),
            script.display().to_string(),
        ])
    } else {
        let script = dir.write(
            "mock-clang-success.sh",
            r#"out=""
is_compile=0
is_assembly=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-S" ]; then
    is_assembly=1
    shift
    continue
  fi
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$is_assembly" -eq 1 ]; then
  printf 'mock-assembly' > "$out"
elif [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
        );
        ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
    }
}

pub(super) fn mock_success_archiver_invocation(dir: &TestDir) -> ArchiverInvocation {
    if cfg!(windows) {
        let script = dir.write(
            "mock-archiver-success.ps1",
            r#"
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -like '/OUT:*') {
        $out = $args[$i].Substring(5)
    }
}
if ($null -eq $out) {
    Write-Error "missing /OUT"
    exit 1
}
Set-Content -Path $out -NoNewline -Value "mock-staticlib"
"#,
        );
        ArchiverInvocation {
            program: ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ]),
            flavor: ArchiverFlavor::Lib,
        }
    } else {
        let script = dir.write(
            "mock-archiver-success.sh",
            r#"out="$2"
printf 'mock-staticlib' > "$out"
"#,
        );
        ArchiverInvocation {
            program: ProgramInvocation::new("/bin/sh")
                .with_args_prefix(vec![script.display().to_string()]),
            flavor: ArchiverFlavor::Ar,
        }
    }
}
