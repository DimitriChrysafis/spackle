//! Glob -> segment matcher. Patterns and paths are split on '/' and
//! matched segment by segment; `**` consumes one or more whole segments,
//! `*`/`?`/classes apply within a single segment.

#[derive(Debug)]
pub struct Matcher {
    segs: Vec<Seg>,
}

#[derive(Debug)]
enum Seg {
    StarStar,
    Piece(Vec<Piece>),
}

#[derive(Debug)]
enum Piece {
    Lit(char),
    Star,
    Question,
    Class { negate: bool, ranges: Vec<(char, char)> },
}

impl Matcher {
    pub fn is_match(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        match_segs(&self.segs, &parts)
    }
}

pub fn compile(glob: &str) -> Result<Matcher, String> {
    if glob.is_empty() {
        return Err("empty pattern".into());
    }
    let mut segs = Vec::new();
    for raw in glob.split('/') {
        if raw == "**" {
            segs.push(Seg::StarStar);
            continue;
        }
        segs.push(Seg::Piece(parse_segment(raw)?));
    }
    Ok(Matcher { segs })
}

fn parse_segment(seg: &str) -> Result<Vec<Piece>, String> {
    let mut out = Vec::new();
    let mut chars = seg.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => out.push(Piece::Star),
            '?' => out.push(Piece::Question),
            '[' => {
                let mut ranges = Vec::new();
                let mut negate = false;
                if chars.peek() == Some(&'!') || chars.peek() == Some(&'^') {
                    chars.next();
                    negate = true;
                }
                let mut prev: Option<char> = None;
                let mut closed = false;
                while let Some(c2) = chars.next() {
                    if c2 == ']' {
                        if let Some(p) = prev.take() {
                            ranges.push((p, p));
                        }
                        closed = true;
                        break;
                    }
                    if c2 == '-' && prev.is_some() && chars.peek() != Some(&']') {
                        let lo = prev.take().unwrap();
                        let hi = chars.next().ok_or("unterminated range")?;
                        ranges.push((lo, hi));
                        continue;
                    }
                    if let Some(p) = prev.replace(c2) {
                        ranges.push((p, p));
                    }
                }
                if !closed {
                    return Err("unterminated character class".into());
                }
                out.push(Piece::Class { negate, ranges });
            }
            c => out.push(Piece::Lit(c)),
        }
    }
    Ok(out)
}

fn match_segs(segs: &[Seg], parts: &[&str]) -> bool {
    if segs.is_empty() {
        return parts.is_empty();
    }
    match &segs[0] {
        Seg::StarStar => {
            // `**` must consume at least one directory segment
            for take in 1..=parts.len() {
                if match_segs(&segs[1..], &parts[take..]) {
                    return true;
                }
            }
            false
        }
        Seg::Piece(pieces) => {
            if parts.is_empty() {
                return false;
            }
            match_piece(pieces, parts[0]) && match_segs(&segs[1..], &parts[1..])
        }
    }
}

fn match_piece(pieces: &[Piece], text: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    piece_at(pieces, 0, &t, 0)
}

fn piece_at(pieces: &[Piece], pi: usize, t: &[char], ti: usize) -> bool {
    if pi == pieces.len() {
        return ti == t.len();
    }
    match &pieces[pi] {
        Piece::Star => {
            for k in (0..=(t.len() - ti)).rev() {
                if piece_at(pieces, pi + 1, t, ti + k) {
                    return true;
                }
            }
            false
        }
        Piece::Question => ti < t.len() && piece_at(pieces, pi + 1, t, ti + 1),
        Piece::Lit(c) => ti < t.len() && t[ti] == *c && piece_at(pieces, pi + 1, t, ti + 1),
        Piece::Class { negate, ranges } => {
            if ti >= t.len() {
                return false;
            }
            let hit = ranges.iter().any(|(lo, hi)| lo <= &t[ti] && &t[ti] <= hi);
            (hit != *negate) && piece_at(pieces, pi + 1, t, ti + 1)
        }
    }
}
