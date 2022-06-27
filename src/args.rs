use crate::ArgError;
use pico_args::Arguments;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

pub struct Args {
    pub all: bool,
    pub recursive: bool,
    pub path: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            all: false,
            recursive: false,
            path: PathBuf::from("."),
        }
    }
}

impl TryFrom<Arguments> for Args {
    type Error = ArgError;

    fn try_from(mut args: Arguments) -> Result<Self, ArgError> {
        let parsed_args = Args {
            all: args.contains(["-a", "--all"]),
            recursive: args.contains(["-r", "--recursive"]),
            path: args
                .free_from_str::<PathBuf>()
                .unwrap_or_else(|_| PathBuf::from(".")),
        };

        if !parsed_args.path.exists()
            && parsed_args.path.as_os_str().as_bytes().starts_with(&[b'-'])
        {
            // If we don't actually have a path starting with '-' report as a unrecognized arg.
            return Err(ArgError::UnknownArg(
                parsed_args.path.as_os_str().to_string_lossy().to_string(),
            ));
        }

        if let Some(unknown_arg) = args.finish().first() {
            return Err(ArgError::UnknownArg(
                unknown_arg.to_string_lossy().to_string(),
            ));
        }

        Ok(parsed_args)
    }
}
