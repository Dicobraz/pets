//! Сообщение второго экземпляра первому: аргументы в UTF-8 через `\0`. Пустое сообщение — нет аргументов
//! (`[""]` неотличим от `[]` — из командной строки пустой аргумент не нужен).

use crate::ShellError;

/// Предел сообщения: больше — отказ у отправителя и отбрасывание у получателя.
pub(crate) const MAX_LEN: usize = 64 * 1024;

pub(crate) fn encode(args: &[String]) -> Result<Vec<u8>, ShellError> {
    if args.iter().any(|a| a.contains('\0')) {
        return Err(ShellError::Nul);
    }
    let bytes = args.join("\0").into_bytes();
    if bytes.len() > MAX_LEN {
        return Err(ShellError::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<String>, ShellError> {
    if bytes.len() > MAX_LEN {
        return Err(ShellError::TooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ShellError::NotUtf8)?;
    if text.is_empty() {
        return Ok(Vec::new());
    }
    Ok(text.split('\0').map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn round_trip() {
        for args in [
            strings(&[]),
            strings(&["mailto:a@b.c"]),
            strings(&["--open", "C:\\Письма\\отчёт.eml", "", "x y"]),
            strings(&["", ""]),
        ] {
            let bytes = encode(&args).unwrap();
            assert_eq!(decode(&bytes).unwrap(), args);
        }
    }

    #[test]
    fn format_is_nul_joined_utf8() {
        assert_eq!(encode(&strings(&["a", "б"])).unwrap(), "a\0б".as_bytes());
        assert_eq!(decode(b"x\0\0y").unwrap(), strings(&["x", "", "y"]));
        assert!(decode(b"").unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(encode(&strings(&["a\0b"])), Err(ShellError::Nul)));
        assert!(matches!(
            encode(&["x".repeat(MAX_LEN + 1)]),
            Err(ShellError::TooLarge(_))
        ));
        assert!(encode(&["x".repeat(MAX_LEN)]).is_ok());
        assert!(matches!(decode(&[0xff, 0xfe]), Err(ShellError::NotUtf8)));
        assert!(matches!(decode(&vec![b'x'; MAX_LEN + 1]), Err(ShellError::TooLarge(_))));
    }
}
