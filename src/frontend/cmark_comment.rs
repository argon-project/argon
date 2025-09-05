use pulldown_cmark;

pub const CMARK_OPTIONS: pulldown_cmark::Options = pulldown_cmark::Options::empty()
    .union(pulldown_cmark::Options::ENABLE_DEFINITION_LIST)
    .union(pulldown_cmark::Options::ENABLE_FOOTNOTES)
    .union(pulldown_cmark::Options::ENABLE_GFM)
    .union(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES)
    .union(pulldown_cmark::Options::ENABLE_MATH)
    .union(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION)
    .union(pulldown_cmark::Options::ENABLE_STRIKETHROUGH)
    .union(pulldown_cmark::Options::ENABLE_SUBSCRIPT)
    .union(pulldown_cmark::Options::ENABLE_SUPERSCRIPT)
    .union(pulldown_cmark::Options::ENABLE_TABLES);

pub const CMARK_LINE_COMMENT: pulldown_cmark::Options = 
    pulldown_cmark::Options::from_bits(1 << 19).unwrap();

pub const CMARK_INLINE_COMMENT: pulldown_cmark::Options = 
    pulldown_cmark::Options::from_bits(1 << 20).unwrap();

pub const CMARK_COMMENT_USES_EXCLAMATION_MARK: pulldown_cmark::Options = 
    pulldown_cmark::Options::from_bits(1 << 21).unwrap();

pub const CMARK_COMMENT_EXTRA_ANGLED_BRACKET: pulldown_cmark::Options = 
    pulldown_cmark::Options::from_bits(1 << 22).unwrap();

pub fn options_from_prefix(prefix: &'static str) -> pulldown_cmark::Options {
    match prefix {
        "///" => CMARK_LINE_COMMENT,
        "///<" => CMARK_LINE_COMMENT.union(CMARK_COMMENT_EXTRA_ANGLED_BRACKET),
        "//!" => CMARK_LINE_COMMENT.union(CMARK_COMMENT_USES_EXCLAMATION_MARK),
        "//!<" => CMARK_LINE_COMMENT.union(CMARK_COMMENT_USES_EXCLAMATION_MARK).union(CMARK_COMMENT_EXTRA_ANGLED_BRACKET),
        "/**" => CMARK_INLINE_COMMENT,
        "/**<" => CMARK_INLINE_COMMENT.union(CMARK_COMMENT_EXTRA_ANGLED_BRACKET),
        "/*!" => CMARK_INLINE_COMMENT.union(CMARK_COMMENT_USES_EXCLAMATION_MARK),
        "/*!<" => CMARK_INLINE_COMMENT.union(CMARK_COMMENT_USES_EXCLAMATION_MARK).union(CMARK_COMMENT_EXTRA_ANGLED_BRACKET),
        _ => pulldown_cmark::Options::empty()
    }
}

#[unsafe(no_mangle)]
extern "Rust" fn externally_scan_line(text: &[u8], options: pulldown_cmark::Options) -> usize {
    assert!(!options.contains(CMARK_INLINE_COMMENT) || !options.contains(CMARK_LINE_COMMENT));

    // Eat leading whitespace.
    let mut ix = text
        .iter()
        .take_while(|&&c| (c as char).is_whitespace())
        .count();

    if ix >= text.len() {
        return 0;
    }

    if options.contains(CMARK_LINE_COMMENT) {
        if matches!(text[ix..], [b'/', b'/', ..]) {
            ix += 2;
        } else {
            return 0;
        }

        let next_char = if options.contains(CMARK_COMMENT_USES_EXCLAMATION_MARK) {
            b'!'
        } else {
            b'/'
        };

        if matches!(text[ix..], [next_char, ..]) {
            ix += 1;
        } else {
            return 0;
        }

        if options.contains(CMARK_COMMENT_EXTRA_ANGLED_BRACKET) {
            if matches!(text[ix..], [b'<', ..]) {
                ix += 1;
            }
        }
    } else if options.contains(CMARK_INLINE_COMMENT) {
        if matches!(text[ix..], [b'/', b'*', ..]) {
            let next_char = if options.contains(CMARK_COMMENT_USES_EXCLAMATION_MARK) {
                b'!'
            } else {
                b'*'
            }; 

            if matches!(text[ix..], [next_char, ..]) {
                ix += 1;
            } else {
                return 0;
            }

            if options.contains(CMARK_COMMENT_EXTRA_ANGLED_BRACKET) {
                if matches!(text[ix..], [b'<', ..]) {
                    ix += 1;
                }
            }
        } else if matches!(text[ix..], [b'*', ..]) {
            ix += 1;
        }
        else {
            return 0;
        }
    }

    fn is_whitespace(c: u8) -> bool {
        c == b'\t' || c == 0x0b || c == 0x0c || c == b' '
    }

    // Eat one code unit of whitespace after line prefix.
    if ix < text.len() && is_whitespace(text[ix]) {
        ix += 1;
    }

    ix
}