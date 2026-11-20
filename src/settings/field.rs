const MAX_CHARS: usize = 300;

pub struct Field {
    pub text: Vec<char>,
    pub caret: usize,
    pub all: bool,
}

impl Field {
    pub fn new(text: &str) -> Self {
        let text: Vec<char> = text.chars().collect();
        Self {
            caret: text.len(),
            text,
            all: false,
        }
    }

    pub fn value(&self) -> String {
        self.text.iter().collect()
    }

    fn take_selection(&mut self) -> bool {
        if !self.all {
            return false;
        }
        self.text.clear();
        self.caret = 0;
        self.all = false;
        true
    }

    pub fn insert(&mut self, text: &str) {
        self.take_selection();
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.text.len() >= MAX_CHARS {
                break;
            }
            self.text.insert(self.caret, c);
            self.caret += 1;
        }
    }

    pub fn backspace(&mut self, word: bool) {
        if self.take_selection() {
            return;
        }
        let start = if word {
            word_start(&self.text, self.caret)
        } else {
            self.caret.saturating_sub(1)
        };
        self.text.drain(start..self.caret);
        self.caret = start;
    }

    pub fn delete(&mut self, word: bool) {
        if self.take_selection() {
            return;
        }
        let end = if word {
            word_end(&self.text, self.caret)
        } else {
            (self.caret + 1).min(self.text.len())
        };
        self.text.drain(self.caret..end);
    }

    pub fn left(&mut self, word: bool) {
        if std::mem::take(&mut self.all) {
            self.caret = 0;
        } else if word {
            self.caret = word_start(&self.text, self.caret);
        } else {
            self.caret = self.caret.saturating_sub(1);
        }
    }

    pub fn right(&mut self, word: bool) {
        if std::mem::take(&mut self.all) {
            self.caret = self.text.len();
        } else if word {
            self.caret = word_end(&self.text, self.caret);
        } else {
            self.caret = (self.caret + 1).min(self.text.len());
        }
    }

    pub fn home(&mut self) {
        self.all = false;
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.all = false;
        self.caret = self.text.len();
    }

    pub fn select_all(&mut self) {
        self.all = !self.text.is_empty();
        self.caret = self.text.len();
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn word_start(text: &[char], caret: usize) -> usize {
    let mut i = caret;
    while i > 0 && !is_word(text[i - 1]) {
        i -= 1;
    }
    while i > 0 && is_word(text[i - 1]) {
        i -= 1;
    }
    i
}

fn word_end(text: &[char], caret: usize) -> usize {
    let mut i = caret;
    while i < text.len() && !is_word(text[i]) {
        i += 1;
    }
    while i < text.len() && is_word(text[i]) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_text() {
        let mut field = Field::new("https://x.org");
        assert_eq!(field.caret, 13);
        field.backspace(true);
        assert_eq!(field.value(), "https://x.");
        field.home();
        field.insert("a\nb");
        assert_eq!(field.value(), "abhttps://x.");
        field.right(true);
        assert_eq!(field.caret, 7);
        field.delete(false);
        assert_eq!(field.value(), "abhttps//x.");
    }

    #[test]
    fn select_all_is_replaced() {
        let mut field = Field::new("old");
        field.select_all();
        field.insert("new");
        assert_eq!(field.value(), "new");
        field.select_all();
        field.backspace(false);
        assert_eq!(field.value(), "");
        assert_eq!(field.caret, 0);
    }

    #[test]
    fn length_is_limited() {
        let mut field = Field::new("");
        field.insert(&"x".repeat(MAX_CHARS + 10));
        assert_eq!(field.text.len(), MAX_CHARS);
    }
}
