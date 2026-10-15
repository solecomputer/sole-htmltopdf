use super::Token;

pub(super) type Piece = (usize, String, Option<crate::font::PositionedRun>, f32);

pub(super) fn assemble(line: &[&Token]) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for (position, token) in line.iter().enumerate() {
        if position > 0 && token.space_before {
            append(
                &mut pieces,
                token.space_run.unwrap_or(token.run),
                " ",
                token.space_positioned.clone(),
                token.space_advance,
            );
        }
        append(
            &mut pieces,
            token.run,
            &token.word,
            token.word_positioned.clone(),
            token.word_advance,
        );
    }
    pieces
}

fn append(
    pieces: &mut Vec<Piece>,
    run: usize,
    text: &str,
    positioned: Option<crate::font::PositionedRun>,
    width: f32,
) {
    if let Some((last_run, last_text, last_positioned, last_width)) = pieces.last_mut()
        && *last_run == run
        && last_positioned.is_some() == positioned.is_some()
    {
        last_text.push_str(text);
        *last_width += width;
        if let (Some(existing), Some(next)) = (last_positioned, positioned) {
            existing.append(next);
        }
    } else {
        pieces.push((run, text.to_owned(), positioned, width));
    }
}
