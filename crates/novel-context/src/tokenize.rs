use std::cmp::Reverse;

#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    pub aliases: Vec<String>,
}

pub fn tokenize(text: &str, lexicon: &Lexicon) -> Vec<String> {
    let mut aliases = lexicon.aliases.clone();
    aliases.sort_by_key(|alias| Reverse(alias.chars().count()));
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() || is_punct(ch) {
            index += 1;
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_alphanumeric() {
                index += 1;
            }
            tokens.push(chars[start..index].iter().collect());
            continue;
        }
        if is_cjk(ch) {
            if let Some(alias) = aliases.iter().find(|alias| {
                let alias_chars: Vec<char> = alias.chars().collect();
                chars[index..].starts_with(alias_chars.as_slice())
            }) {
                tokens.push(alias.clone());
                index += alias.chars().count();
                continue;
            }
            if index + 1 < chars.len() && is_cjk(chars[index + 1]) {
                tokens.push(format!("{}{}", chars[index], chars[index + 1]));
                index += 1;
                continue;
            }
        }
        index += 1;
    }
    tokens
}

pub fn fts_query(tokens: &[String]) -> String {
    tokens
        .iter()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn is_cjk(ch: char) -> bool {
    matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
}

fn is_punct(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '！'
            | '？'
            | '；'
            | '：'
            | ','
            | '.'
            | '!'
            | '?'
            | ';'
            | ':'
            | '"'
            | '\''
            | '「'
            | '」'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recalls_bigrams_and_proper_names() {
        let lexicon = Lexicon {
            aliases: vec!["青翼蝠王".into(), "魂兽".into()],
        };
        let tokens = tokenize("林动走出森林，遇见青翼蝠王这种魂兽。", &lexicon);
        assert!(tokens.iter().any(|token| token == "林动"));
        assert!(tokens.iter().any(|token| token == "青翼蝠王"));
        assert!(tokens.iter().any(|token| token == "魂兽"));
        assert!(!tokens.iter().any(|token| token == "青翼"));
    }
}
