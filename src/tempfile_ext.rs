// Copyright (c) 2015 Steven Allen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.
//

const NUM_RETRIES: u32 = 1 << 31;
const NUM_RAND_CHARS: usize = 6;

pub fn new_tmp_file_name(file_names: &[String]) -> std::io::Result<String> {
    for _ in 0..NUM_RETRIES {
        let temp_file_name = tmpname(".tmp", NUM_RAND_CHARS);
        // unlikely but ensure there aren't conflicts.
        if !file_names.contains(&temp_file_name) {
            return Ok(temp_file_name);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "too many temporary files exist",
    ))
}

fn tmpname(prefix: &str, rand_len: usize) -> String {
    let mut buf = String::with_capacity(prefix.len() + rand_len);
    buf.push_str(prefix);
    let mut char_buf = [0u8; 4];
    for c in std::iter::repeat_with(fastrand::alphanumeric).take(rand_len) {
        buf.push_str(c.encode_utf8(&mut char_buf));
    }
    buf
}
