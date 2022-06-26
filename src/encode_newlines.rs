use std::fmt::{Display, Formatter};

pub struct NewlineEncoded {
    str: String,
}

impl Display for NewlineEncoded {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.str.fmt(f)
    }
}

pub trait EncodeNewlines {
    fn encode_newlines(self) -> NewlineEncoded;
}

impl EncodeNewlines for String {
    fn encode_newlines(self) -> NewlineEncoded {
        NewlineEncoded {
            str: if self.contains("\n") {
                self.replace("\n", "\\n")
            } else {
                self
            }
        }
    }
}

impl EncodeNewlines for &str {
    fn encode_newlines(self) -> NewlineEncoded {
        self.to_owned().encode_newlines()
    }
}

impl NewlineEncoded {
    pub fn decode_newlines(self) -> String {
        let str = self.str;
        if str.contains("\\n") {
            str.replace("\\n", "\n")
        } else {
            str
        }
    }
}