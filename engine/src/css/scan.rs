pub(crate) fn matching_brace(tail: &str) -> Option<usize> {
    let mut depth = 0_usize;
    let mut state = State::Code;
    let mut bytes = tail.char_indices().peekable();
    while let Some((index, character)) = bytes.next() {
        match state {
            State::Code => match character {
                '"' => state = State::Quoted('"'),
                '\'' => state = State::Quoted('\''),
                '/' if bytes.peek().is_some_and(|(_, next)| *next == '*') => {
                    bytes.next();
                    state = State::Comment;
                }
                '{' => depth = depth.saturating_add(1),
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
            State::Quoted(quote) => match character {
                '\\' => {
                    bytes.next();
                }
                other if other == quote => state = State::Code,

                other if ends_a_bad_string(other) => state = State::Code,
                _ => {}
            },
            State::Comment => {
                if character == '*' && bytes.peek().is_some_and(|(_, next)| *next == '/') {
                    bytes.next();
                    state = State::Code;
                }
            }
        }
    }
    None
}

pub(crate) fn find_outside_strings(text: &str, needle: char) -> Option<usize> {
    let mut state = State::Code;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        match state {
            State::Code => {
                if character == needle {
                    return Some(index);
                }
                match character {
                    '"' | '\'' => state = State::Quoted(character),
                    '/' if characters.peek().is_some_and(|(_, next)| *next == '*') => {
                        characters.next();
                        state = State::Comment;
                    }
                    _ => {}
                }
            }
            State::Quoted(quote) => match character {
                '\\' => {
                    characters.next();
                }
                other if other == quote => state = State::Code,
                other if ends_a_bad_string(other) => state = State::Code,
                _ => {}
            },
            State::Comment => {
                if character == '*' && characters.peek().is_some_and(|(_, next)| *next == '/') {
                    characters.next();
                    state = State::Code;
                }
            }
        }
    }
    None
}

pub(crate) fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut characters = source.char_indices().peekable();
    while let Some((_, character)) = characters.next() {
        match state {
            State::Code => match character {
                '/' if characters.peek().is_some_and(|(_, next)| *next == '*') => {
                    characters.next();
                    state = State::Comment;

                    out.push(' ');
                }
                '"' | '\'' => {
                    state = State::Quoted(character);
                    out.push(character);
                }
                _ => out.push(character),
            },
            State::Quoted(quote) => {
                out.push(character);
                match character {
                    '\\' => {
                        if let Some((_, escaped)) = characters.next() {
                            out.push(escaped);
                        }
                    }
                    other if other == quote => state = State::Code,

                    other if ends_a_bad_string(other) => state = State::Code,
                    _ => {}
                }
            }
            State::Comment => {
                if character == '*' && characters.peek().is_some_and(|(_, next)| *next == '/') {
                    characters.next();
                    state = State::Code;
                }
            }
        }
    }
    out
}

fn ends_a_bad_string(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\u{c}')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Code,
    Quoted(char),
    Comment,
}
