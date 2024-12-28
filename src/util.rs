pub(crate) trait StringExt {
    fn push_string(&mut self, string: String);
    fn push_newline(&mut self);
}

impl StringExt for String {
    fn push_newline(&mut self) {
        self.push('\n');
    }

    fn push_string(&mut self, string: String) {
        self.push_str(&string);
    }
}

