//! Runs tests using actual binary, apapted from 'fd' method: https://github.com/sharkdp/fd/blob/master/tests/testenv/mod.rs
#![allow(dead_code)]
use std::fs::File;
use std::io::{BufReader, Read};
use std::env;
#[cfg(windows)]
use std::os::windows;
use std::path::{Path, PathBuf};
use std::process;
use std::env::temp_dir;

/// Dump from the `system_profiler` command on macOS
pub const SYSTEM_PROFILER_DUMP_PATH: &'static str = "./tests/data/system_profiler_dump.json";
/// Dump using macOS system_profiler so no [`USBDeviceExtra`]
pub const CYME_SP_TREE_DUMP: &'static str = "./tests/data/cyme_sp_tree_dump.json";
/// Dump using Linux with libusb so with [`USBDeviceExtra`]
pub const CYME_LIBUSB_LINUX_TREE_DUMP: &'static str = "./tests/data/cyme_libusb_linux_tree.json";
/// Dump using macOS with libusb so with [`USBDeviceExtra`]
pub const CYME_LIBUSB_MACOS_TREE_DUMP: &'static str = "./tests/data/cyme_libusb_macos_tree.json"; // TODO
/// Output of lsusb --tree
pub const LSUSB_TREE_OUTPUT: &'static str = "./tests/data/lsusb_tree.txt";
/// Output of lsusb --tree -vvv
pub const LSUSB_TREE_OUTPUT_VERBOSE: &'static str = "./tests/data/lsusb_tree_verbose.txt";
/// Output of lsusb
pub const LSUSB_OUTPUT: &'static str = "./tests/data/lsusb_list.txt";
/// Output of lsusb --verbose
pub const LSUSB_OUTPUT_VERBOSE: &'static str = "./tests/data/lsusb_verbose.txt"; 

pub fn read_dump(file_name: &str) -> BufReader<File> {
    let f = File::open(file_name).expect("Unable to open json dump file");
    BufReader::new(f)
}

pub fn read_dump_to_string(file_name: &str) -> String {
    let mut ret = String::new();
    let mut br = read_dump(file_name);
    br.read_to_string(&mut ret).expect(&format!("Failed to read {}", file_name));
    ret
}

pub fn sp_data_from_system_profiler() -> cyme::system_profiler::SPUSBDataType {
    let mut br = read_dump(SYSTEM_PROFILER_DUMP_PATH);
    let mut data = String::new();
    br.read_to_string(&mut data).expect("Unable to read string");

    serde_json::from_str::<cyme::system_profiler::SPUSBDataType>(&data).unwrap()
}

pub fn sp_data_from_libusb_linux() -> cyme::system_profiler::SPUSBDataType {
    let mut br = read_dump(CYME_LIBUSB_LINUX_TREE_DUMP);
    let mut data = String::new();
    br.read_to_string(&mut data).expect("Unable to read string");

    serde_json::from_str::<cyme::system_profiler::SPUSBDataType>(&data).unwrap()
}

/// Environment for the integration tests.
pub struct TestEnv {
    /// Temporary working directory.
    temp_dir: PathBuf,
    /// Path to the *cyme* executable.
    cyme_exe: PathBuf,
    /// Normalize each line by sorting the whitespace-separated words
    normalize_line: bool,
}

/// Find the *cyme* executable.
fn find_cyme_exe() -> PathBuf {
    // Tests exe is in target/debug/deps, the *cyme* exe is in target/debug
    let root = env::current_exe()
        .expect("tests executable")
        .parent()
        .expect("tests executable directory")
        .parent()
        .expect("cyme executable directory")
        .to_path_buf();

    let exe_name = if cfg!(windows) { "cyme.exe" } else { "cyme" };

    root.join(exe_name)
}

/// Format an error message for when *cyme* did not exit successfully.
fn format_exit_error(args: &[&str], output: &process::Output) -> String {
    format!(
        "`cyme {}` did not exit successfully.\nstdout:\n---\n{}---\nstderr:\n---\n{}---",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Format an error message for when the output of *cyme* did not match the expected output.
fn format_output_error(args: &[&str], expected: &str, actual: &str) -> String {
    // Generate diff text.
    let diff_text = diff::lines(expected, actual)
        .into_iter()
        .map(|diff| match diff {
            diff::Result::Left(l) => format!("-{}", l),
            diff::Result::Both(l, _) => format!(" {}", l),
            diff::Result::Right(r) => format!("+{}", r),
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        concat!(
            "`cyme {}` did not produce the expected output.\n",
            "Showing diff between expected and actual:\n{}\n"
        ),
        args.join(" "),
        diff_text
    )
}

/// Normalize the output for comparison.
fn normalize_output(s: &str, trim_start: bool, normalize_line: bool) -> String {
    // Split into lines and normalize separators.
    let mut lines = s
        .replace('\0', "NULL\n")
        .lines()
        .map(|line| {
            let line = if trim_start { line.trim_start() } else { line };
            let line = line.replace('/', &std::path::MAIN_SEPARATOR.to_string());
            if normalize_line {
                let mut words: Vec<_> = line.split_whitespace().collect();
                words.sort_unstable();
                return words.join(" ");
            }
            line
        })
        .collect::<Vec<_>>();

    lines.sort();
    lines.join("\n")
}

/// Trim whitespace from the beginning of each line.
fn trim_lines(s: &str) -> String {
    s.lines()
        .map(|line| line.trim_start())
        .fold(String::new(), |mut str, line| {
            str.push_str(line);
            str.push('\n');
            str
        })
}

impl TestEnv {
    pub fn new() -> TestEnv {
        let temp_dir = temp_dir();
        let cyme_exe = find_cyme_exe();

        TestEnv {
            temp_dir,
            cyme_exe,
            normalize_line: false,
        }
    }

    pub fn normalize_line(self, normalize: bool) -> TestEnv {
        TestEnv {
            temp_dir: self.temp_dir,
            cyme_exe: self.cyme_exe,
            normalize_line: normalize,
        }
    }

    /// Get the path of the cyme executable.
    #[cfg_attr(windows, allow(unused))]
    pub fn test_exe(&self) -> &PathBuf {
        &self.cyme_exe
    }

    /// Assert that calling *cyme* in the specified path under the root working directory,
    /// and with the specified arguments produces the expected output.
    pub fn assert_success_and_get_output(
        &self,
        dump_file: Option<&str>,
        args: &[&str],
    ) -> process::Output {
        // Setup *cyme* command.
        let mut cmd = process::Command::new(&self.cyme_exe);
        if let Some(dump) = dump_file {
            cmd.arg("--from-json").arg(dump).args(args);
        } else {
            cmd.arg("--json ").args(args);
        }

        // Run *cyme*.
        let output = cmd.output().expect("cyme output");

        // Check for exit status.
        if !output.status.success() {
            panic!("{}", format_exit_error(args, &output));
        }

        output
    }

    pub fn assert_success_and_get_normalized_output(
        &self,
        dump_file: Option<&str>,
        args: &[&str],
    ) -> String {
        let output = self.assert_success_and_get_output(dump_file, args);
        normalize_output(
            &String::from_utf8_lossy(&output.stdout),
            false,
            self.normalize_line,
        )
    }

    /// Assert that calling *cyme* with the specified arguments produces the expected output.
    pub fn assert_output(&self, dump_file: Option<&str>, args: &[&str], expected: &str) {
        self.assert_output_subdirectory(dump_file, args, expected)
    }

    /// Similar to assert_output, but able to handle non-utf8 output
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn assert_output_raw(&self, args: &[&str], expected: &[u8]) {
        let output = self.assert_success_and_get_output(".", args);

        assert_eq!(expected, &output.stdout[..]);
    }

    pub fn assert_output_subdirectory(
        &self,
        dump_file: Option<&str>,
        args: &[&str],
        expected: &str,
    ) {
        // Normalize both expected and actual output.
        let expected = normalize_output(expected, true, self.normalize_line);
        let actual = self.assert_success_and_get_normalized_output(dump_file, args);

        // Compare actual output to expected output.
        if expected != actual {
            panic!("{}", format_output_error(args, &expected, &actual));
        }
    }

    /// Assert that calling *cyme* with the specified arguments produces the expected error,
    /// and does not succeed.
    pub fn assert_failure_with_error(&self, args: &[&str], expected: &str) {
        let status = self.assert_error_subdirectory(".", args, Some(expected));
        if status.success() {
            panic!("error '{}' did not occur.", expected);
        }
    }

    /// Assert that calling *cyme* with the specified arguments does not succeed.
    pub fn assert_failure(&self, args: &[&str]) {
        let status = self.assert_error_subdirectory(".", args, None);
        if status.success() {
            panic!("Failure did not occur as expected.");
        }
    }

    /// Assert that calling *cyme* with the specified arguments produces the expected error.
    pub fn assert_error(&self, args: &[&str], expected: &str) -> process::ExitStatus {
        self.assert_error_subdirectory(".", args, Some(expected))
    }

    /// Assert that calling *cyme* in the specified path under the root working directory,
    /// and with the specified arguments produces an error with the expected message.
    fn assert_error_subdirectory<P: AsRef<Path>>(
        &self,
        path: P,
        args: &[&str],
        expected: Option<&str>,
    ) -> process::ExitStatus {
        // Setup *cyme* command.
        let mut cmd = process::Command::new(&self.cyme_exe);
        cmd.current_dir(self.temp_dir.join(path));
        cmd.arg("--no-global-ignore-file").args(args);

        // Run *cyme*.
        let output = cmd.output().expect("cyme output");

        if let Some(expected) = expected {
            // Normalize both expected and actual output.
            let expected_error = trim_lines(expected);
            let actual_err = trim_lines(&String::from_utf8_lossy(&output.stderr));

            // Compare actual output to expected output.
            if !actual_err.trim_start().starts_with(&expected_error) {
                panic!(
                    "{}",
                    format_output_error(args, &expected_error, &actual_err)
                );
            }
        }

        output.status
    }
}