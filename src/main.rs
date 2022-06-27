extern crate core;

use std::fs::{File, read_dir, rename};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::os::unix::prelude::OsStrExt;
use std::path::Path;
use std::process::{Command, exit, ExitStatus};

use pico_args::Arguments;
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use args::Args;
use error::Error;
use escape_newlines::NewlineEscaped;

use crate::error::ArgError;
use crate::escape_newlines::EscapeNewlines;
use crate::path_ext::PathExt;

mod args;
mod error;
mod escape_newlines;
mod tempfile_ext;
mod path_ext;

fn main() {
    let args = Arguments::from_env();
    let args = match Args::try_from(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    match ere(args, EnvEditor) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }
}

trait Editor {
    fn edit(self, path: &Path) -> std::io::Result<ExitStatus>;
}

struct EnvEditor;

impl Editor for EnvEditor {
    fn edit(self, path: &Path) -> std::io::Result<ExitStatus> {
        let ed = std::env::var("EDITOR").unwrap_or("vi".to_owned());
        return Command::new(ed).arg(path).status();
    }
}

fn ere(args: Args, editor: impl Editor) -> Result<(), Error> {
    let path = args.path.as_path();

    let file_names = read_file_names_from_dir(path, args.recursive, args.all)?;

    let tmp = NamedTempFile::new_in(path)?;
    let mut writer = BufWriter::new(tmp);

    writeln!(writer, "# Rename the files below.")?;
    writeln!(
        writer,
        "# Do not delete or move lines as the order is used for the rename."
    )?;
    writeln!(writer, "")?;

    for file_name in &file_names {
        writeln!(writer, "{}", file_name.clone().escape_newlines())?;
    }

    let tmp = writer.into_inner().map_err(|e| e.into_error())?;

    let ed = editor.edit(tmp.path())?;
    if !ed.success() {
        return Err(Error::EditorStatus(ed));
    }

    let new_file_names = parse_file_names(tmp.path())?;

    drop(tmp);

    if file_names.len() != new_file_names.len() {
        return Err(Error::CountMismatch);
    }

    let mut failures = Vec::new();

    let mut file_names = file_names;
    let mut new_file_names = new_file_names;
    let mut temp_file_names = Vec::new();
    while !file_names.is_empty() {
        let file_name = file_names.remove(file_names.len() - 1);
        let new_file_name = new_file_names.remove(new_file_names.len() - 1).unescape();
        if file_name == new_file_name {
            continue;
        }

        let from = path.join(file_name);
        if file_names.contains(&new_file_name) {
            // file name already exists, rename to temp file for a second pass.
            match tempfile_ext::new_tmp_file_name(&file_names) {
                Ok(temp_file_name) => {
                    let to = path.join(&temp_file_name);
                    if let Err(source) = rename(&from, &to) {
                        failures.push(Error::Rename { from, to, source });
                    } else {
                        temp_file_names.push((temp_file_name, new_file_name));
                    }
                }
                Err(source) => {
                    failures.push(Error::Io(source));
                }
            }
        } else {
            // we are good to rename
            let to = path.join(new_file_name);
            if let Err(source) = rename(&from, &to) {
                failures.push(Error::Rename { from, to, source });
            }
        }
    }

    // now rename the temp files to their final names
    for (temp_file_name, new_file_name) in temp_file_names {
        let from = path.join(temp_file_name);
        let to = path.join(new_file_name);
        if let Err(source) = rename(&from, &to) {
            failures.push(Error::Rename { from, to, source });
        }
    }

    if !failures.is_empty() {
        return Err(Error::Group(failures));
    }

    Ok(())
}

fn read_file_names_from_dir(path: &Path, recursive: bool, include_hidden: bool) -> std::io::Result<Vec<String>> {
    let mut file_names = Vec::new();
    let itr = WalkDir::new(path)
        .same_file_system(true)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .into_iter()
        .filter_entry(|e| e.path() == path || include_hidden || !e.path().is_hidden())
        // skip root dir
        .skip(1);
    for entry in itr {
        let entry = entry?;
        let path = entry.path().strip_prefix(path).unwrap();
        file_names.push(path.to_string_lossy().into_owned());
    }
    Ok(file_names)
}

fn parse_file_names(path: &Path) -> std::io::Result<Vec<NewlineEscaped>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    return reader
        .lines()
        .filter(|line| {
            line.as_ref()
                .map_or(false, |line| !line.is_empty() && !line.starts_with("#"))
        })
        .map(|line| line.map(|line| NewlineEscaped::new(line)))
        .collect::<Result<Vec<_>, _>>();
}

#[cfg(test)]
mod test {
    use std::fs::{create_dir, read_to_string};
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::path::{Path, PathBuf};

    use assert_fs::prelude::*;
    use assert_fs::TempDir;
    use predicates::prelude::*;

    use crate::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct TestEditor<'a>(Box<dyn FnOnce(Vec<NewlineEscaped>) -> Vec<NewlineEscaped> + 'a>);

    impl<'a> TestEditor<'a> {
        fn new(f: impl FnOnce(Vec<NewlineEscaped>) -> Vec<NewlineEscaped> + 'a) -> TestEditor<'a> {
            TestEditor(Box::new(f))
        }
    }

    impl<'a> Editor for TestEditor<'a> {
        fn edit(self, path: &Path) -> io::Result<ExitStatus> {
            let file_names = parse_file_names(path)?;
            dbg!(&file_names);
            let new_file_names = self.0(file_names);
            dbg!(&new_file_names);
            let mut writer = File::create(path)?;

            for new_file_name in new_file_names {
                writeln!(writer, "{}", new_file_name)?;
            }

            Ok(ExitStatus::from_raw(0))
        }
    }

    fn paths(root: &Path) -> Result<Vec<PathBuf>, walkdir::Error> {
        let mut result = Vec::new();
        let itr = WalkDir::new(root)
            .into_iter()
            // skip root
            .skip(1);
        for entry in itr {
            result.push(entry?.into_path());
        }
        Ok(result)
    }

    #[test]
    fn renames_a_file() -> TestResult {
        let test_dir = TempDir::new()?;
        test_dir.child("a").touch()?;

        ere(
            Args {
                path: test_dir.to_path_buf(),
                ..Args::default()
            },
            TestEditor::new(|_names| vec!["b".escape_newlines()]),
        )?;

        assert_eq!(
            paths(test_dir.path())?,
            vec![test_dir.join("b")]
        );

        Ok(())
    }

    #[test]
    fn renames_a_file_with_a_newline_in_filename() -> TestResult {
        let test_dir = TempDir::new()?;
        test_dir.child("a\nb").touch()?;

        let mut provided_file_names = Vec::new();

        ere(
            Args {
                path: test_dir.to_path_buf(),
                ..Args::default()
            },
            TestEditor::new(|names| {
                provided_file_names = names;
                vec!["a\nc".escape_newlines()]
            }),
        )?;

        assert_eq!(
            paths(test_dir.path())?,
            vec![test_dir.join("a\nc")]
        );

        assert_eq!(
            provided_file_names,
            vec!["a\nb".escape_newlines()]
        );

        Ok(())
    }

    #[test]
    fn renames_two_files_to_each_other() -> TestResult {
        let test_dir = TempDir::new()?;
        test_dir.child("a").write_str("a")?;
        test_dir.child("b").write_str("b")?;

        ere(
            Args {
                path: test_dir.to_path_buf(),
                ..Args::default()
            },
            TestEditor::new(|file_names| {
                let mut new_file_names = file_names;
                new_file_names.reverse();
                new_file_names
            }),
        )?;

        assert_eq!(
            paths(test_dir.path())?,
            vec![test_dir.join("b"), test_dir.join("a")]
        );

        assert_eq!(read_to_string(test_dir.join("a"))?, "b");
        assert_eq!(read_to_string(test_dir.join("b"))?, "a");

        Ok(())
    }

    #[test]
    fn excludes_hidden_files_by_default() -> TestResult {
        let test_dir = TempDir::new()?;
        test_dir.child(".a").touch()?;
        test_dir.child("b").touch()?;

        let mut provided_file_names = Vec::new();

        ere(
            Args {
                path: test_dir.to_path_buf(),
                ..Args::default()
            },
            TestEditor::new(|names| {
                provided_file_names = names.clone();
                names
            }),
        )?;

        assert_eq!(
            paths(test_dir.path())?,
            vec![test_dir.join("b"), test_dir.join(".a")]
        );

        assert_eq!(provided_file_names, vec!["b".escape_newlines()]);

        Ok(())
    }

    #[test]
    fn recurses_if_recursive_option_given() -> TestResult {
        let test_dir = TempDir::new()?;
        test_dir.child("a/a").touch()?;
        test_dir.child("a/b").touch()?;

        let mut provided_file_names = Vec::new();

        ere(
            Args {
                recursive: true,
                path: test_dir.to_path_buf(),
                ..Args::default()
            },
            TestEditor::new(|names| {
                provided_file_names = names;
                vec!["a/b".escape_newlines(), "a/c".escape_newlines()]
            }),
        )?;

        assert_eq!(
            paths(test_dir.path())?,
            vec![
                test_dir.join("a"),
                test_dir.join("a/b"),
                test_dir.join("a/c"),
            ]
        );

        assert_eq!(provided_file_names, vec!["a/b".escape_newlines(), "a/a".escape_newlines()]);

        Ok(())
    }
}
