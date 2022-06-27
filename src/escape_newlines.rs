use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewlineEscaped {
    str: String,
}

impl NewlineEscaped {
    pub fn new(str: impl Into<String>) -> NewlineEscaped {
        NewlineEscaped { str: str.into() }
    }
}

impl Display for NewlineEscaped {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.str.fmt(f)
    }
}

pub trait EscapeNewlines {
    fn escape_newlines(self) -> NewlineEscaped;
}

impl EscapeNewlines for String {
    fn escape_newlines(self) -> NewlineEscaped {
        NewlineEscaped {
            str: if self.contains('\n') || self.contains(r"\n") {
                self.replace(r"\n", r"\\n").replace("\n", r"\n")
            } else {
                self
            },
        }
    }
}

impl EscapeNewlines for &str {
    fn escape_newlines(self) -> NewlineEscaped {
        self.to_owned().escape_newlines()
    }
}

impl NewlineEscaped {
    pub fn unescape(self) -> String {
        let bytes = self.str.as_bytes();
        let mut backslash_count = 0;
        let mut out: Option<Vec<u8>> = None;
        for (i, byte) in bytes.iter().enumerate() {
            match byte {
                b'\\' => {
                    backslash_count += 1;
                }
                b'n' => {
                    match backslash_count {
                        0 => {
                            if let Some(ref mut out) = out {
                                out.push(b'n');
                            }
                        }
                        1 => {
                            let out =
                                out.get_or_insert_with(|| bytes[0..i - backslash_count].to_vec());
                            out.push(b'\n');
                        }
                        _ => {
                            let out =
                                out.get_or_insert_with(|| bytes[0..i - backslash_count].to_vec());
                            for _ in 0..(backslash_count - 1) {
                                out.push(b'\\');
                            }
                            out.push(b'n');
                        }
                    }
                    backslash_count = 0;
                }
                _ => {
                    if let Some(ref mut out) = out {
                        out.push(*byte);
                    }
                    backslash_count = 0;
                }
            }
        }
        out.map(|bytes| unsafe { String::from_utf8_unchecked(bytes) })
            .unwrap_or(self.str)
    }
}

#[cfg(test)]
mod test {
    use crate::{EscapeNewlines, NewlineEscaped};

    #[test]
    fn escapes_newline() {
        assert_eq!("a\nb".escape_newlines().to_string(), r"a\nb");
    }

    #[test]
    fn unescapes_newline() {
        assert_eq!(NewlineEscaped::new(r"a\nb").unescape(), "a\nb");
    }

    #[test]
    fn roundtrips_newline() {
        assert_eq!("a\nb".escape_newlines().unescape(), "a\nb");
    }

    #[test]
    fn escapes_newline_literal() {
        assert_eq!(r"a\nb".escape_newlines().to_string(), r"a\\nb");
    }

    #[test]
    fn unescapes_newline_literal() {
        assert_eq!(NewlineEscaped::new(r"a\\nb").unescape(), r"a\nb");
    }

    #[test]
    fn roundtrips_newline_literal() {
        assert_eq!(r"a\nb".escape_newlines().unescape(), r"a\nb");
    }

    #[test]
    fn escapes_extra_backslashs() {
        assert_eq!(r"n\\n".escape_newlines().to_string(), r"n\\\n");
    }

    #[test]
    fn unescapes_extra_backslashes() {
        assert_eq!(NewlineEscaped::new(r"n\\\n").unescape(), r"n\\n");
    }
}
