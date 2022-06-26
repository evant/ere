use std::fs::{File, read_dir, rename};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Command, exit, ExitStatus};

use tempfile::NamedTempFile;

use encode_newlines::NewlineEncoded;
use error::Error;

use crate::encode_newlines::EncodeNewlines;

mod tempfile_ext;
mod error;
mod encode_newlines;

fn main() {
    match ere(".", EnvEditor) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }
}

trait Editor {
    fn edit(&self, path: &Path) -> std::io::Result<ExitStatus>;
}

struct EnvEditor;

impl Editor for EnvEditor {
    fn edit(&self, path: &Path) -> std::io::Result<ExitStatus> {
        let ed = std::env::var("EDITOR").unwrap_or("vi".to_owned());
        return Command::new(ed)
            .arg(path)
            .status();
    }
}

fn ere(path: impl AsRef<Path>, editor: impl Editor) -> Result<(), Error> {
    let path = path.as_ref();

    let file_names = read_file_names_from_dir(path)?;

    let tmp = NamedTempFile::new_in(path)?;
    let mut writer = BufWriter::new(tmp);

    writeln!(writer, "# Rename the files below.")?;
    writeln!(writer, "# Do not delete or move lines as the order is used for the rename.")?;
    writeln!(writer, "")?;

    for file_name in &file_names {
        writeln!(writer, "{}", file_name.clone().encode_newlines())?;
    }

    let tmp = writer.into_inner()
        .map_err(|e| e.into_error())?;

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
        let new_file_name = new_file_names.remove(new_file_names.len() - 1).decode_newlines();
        if file_name == new_file_name { continue; }

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

fn read_file_names_from_dir(path: &Path) -> std::io::Result<Vec<String>> {
    let mut file_names = Vec::new();
    for entry in read_dir(path)? {
        let file_name = entry?.file_name();
        file_names.push(file_name.to_string_lossy().to_string());
    }
    Ok(file_names)
}

fn parse_file_names(path: &Path) -> std::io::Result<Vec<NewlineEncoded>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    return reader.lines()
        .filter(|line| line.as_ref().map_or(false, |line| !line.is_empty() && !line.starts_with("#")))
        .map(|line| line.map(|line| line.encode_newlines()))
        .collect::<Result<Vec<_>, _>>();
}

#[cfg(test)]
mod test {
    use std::fs::read_to_string;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;

    use tempfile::TempDir;

    use crate::*;

    struct TestEditor(fn(Vec<NewlineEncoded>) -> Vec<NewlineEncoded>);

    impl Editor for TestEditor {
        fn edit(&self, path: &Path) -> std::io::Result<ExitStatus> {
            let file_names = parse_file_names(path)?;
            let new_file_names = self.0(file_names);
            let mut writer = File::create(path)?;

            for new_file_name in new_file_names {
                writeln!(writer, "{}", new_file_name)?;
            }

            Ok(ExitStatus::from_raw(0))
        }
    }

    #[test]
    fn renames_a_file() -> Result<(), Error> {
        let test_dir = TempDir::new()?;

        File::create(test_dir.path().join("a"))?;

        ere(test_dir.path(), TestEditor(|_names| vec!["b".encode_newlines()]))?;

        let file_names = read_file_names_from_dir(test_dir.path())?;

        assert_eq!(file_names, vec!["b".to_string()]);

        Ok(())
    }

    #[test]
    fn renames_a_file_with_a_newline_in_filename() -> Result<(), Error> {
        let test_dir = TempDir::new()?;

        File::create(test_dir.path().join("a\nb"))?;

        ere(test_dir.path(), TestEditor(|_names| vec!["a\nc".encode_newlines()]))?;

        let file_names = read_file_names_from_dir(test_dir.path())?;

        assert_eq!(file_names, vec!["a\nc".to_string()]);

        Ok(())
    }

    #[test]
    fn renames_two_files_to_each_other() -> Result<(), Error> {
        let test_dir = TempDir::new()?;

        {
            let mut file = File::create(test_dir.path().join("a"))?;
            write!(file, "a")?;
        }
        {
            let mut file = File::create(test_dir.path().join("b"))?;
            write!(file, "b")?;
        }

        ere(test_dir.path(), TestEditor(|file_names| {
            let mut new_file_names = file_names;
            new_file_names.reverse();
            new_file_names
        }))?;

        assert_eq!(read_to_string(test_dir.path().join("a"))?, "b".to_string());
        assert_eq!(read_to_string(test_dir.path().join("b"))?, "a".to_string());

        Ok(())
    }
}
