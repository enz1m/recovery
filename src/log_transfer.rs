//! Offline QR transfer of the latest output. A phone can copy plain text directly.
use iced::widget::qr_code;
pub(crate) struct Transfer {
    pub(crate) text: String,
    pub(crate) code: qr_code::Data,
}
fn latest(log: &str, secrets: &[&str]) -> String {
    let mut clean = log.to_owned();
    let mut secrets: Vec<_> = secrets.iter().copied().filter(|s| !s.is_empty()).collect();
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for secret in secrets {
        clean = clean.replace(secret, "[redacted]");
    }
    clean.retain(|c| !c.is_control() || c == '\n' || c == '\t');
    let mut start = clean.len().saturating_sub(900);
    while !clean.is_char_boundary(start) {
        start += 1;
    }
    if start > 0
        && let Some(i) = clean[start..].find('\n').filter(|i| *i < 450)
    {
        start += i + 1;
    }
    format!(
        "enzimOS - latest boot output\n{}",
        if clean.is_empty() {
            crate::i18n::label("No output yet.")
        } else {
            &clean[start..]
        }
    )
}
impl Transfer {
    pub(crate) fn new(log: &str, secrets: &[&str]) -> Self {
        let text = latest(log, secrets);
        let code = qr_code::Data::new(&text).expect("bounded UTF-8 QR payload");
        Self { text, code }
    }
    pub(crate) fn refresh(&mut self, log: &str, secrets: &[&str]) {
        let text = latest(log, secrets);
        if text != self.text {
            self.code = qr_code::Data::new(&text).expect("bounded UTF-8 QR payload");
            self.text = text;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_unicode_output_is_bounded_and_refreshes() {
        let log = "Installing — Dublin 日本語\n".repeat(200);
        let mut t = Transfer::new(&log, &[]);
        assert!(t.text.len() < 1000);
        assert!(t.text.ends_with("日本語\n"));
        t.refresh(&(log + "LATEST LINE"), &[]);
        assert!(t.text.ends_with("LATEST LINE"));
    }
    #[test]
    fn secrets_are_removed_before_tail() {
        let t = Transfer::new(
            &format!("{}my-secret longer-secret", "x".repeat(895)),
            &["my-secret", "longer-secret"],
        );
        assert!(!t.text.contains("my-secret"));
        assert!(!t.text.contains("longer-secret"));
    }
}
