//! Console API — abstraksi output untuk userspace


pub struct Style;
impl Style {
    pub fn color(name: &str) -> &'static str {
        match name {
            "red"    => "\x1b[31m",
            "green"  => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue"   => "\x1b[34m",
            "cyan"   => "\x1b[36m",
            "white"  => "\x1b[37m",
            "lime"   => "\x1b[92m",
            _        => "",
        }
    }
    pub fn reset() -> &'static str { "\x1b[0m" }
    pub fn bold()  -> &'static str { "\x1b[1m" }
}
