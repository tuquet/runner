//! Unified Terminal Notification Dispatcher for Specter Runner Daemon
//! Standardizes console output, error logging, and process lifecycle events.

pub mod colors {
    pub const CYAN: &str = "\x1b[38;2;56;189;248m";
    pub const PURPLE: &str = "\x1b[38;2;168;85;247m";
    pub const GREEN: &str = "\x1b[38;2;74;222;128m";
    pub const AMBER: &str = "\x1b[38;2;251;191;36m";
    pub const RED: &str = "\x1b[38;2;248;113;113m";
    pub const MUTED: &str = "\x1b[38;2;148;163;184m";
    pub const BOLD: &str = "\x1b[1m";
    pub const BOLD_WHITE: &str = "\x1b[1;37m";
    pub const RESET: &str = "\x1b[0m";
}

pub struct Notify;

impl Notify {
    pub fn success(msg: impl std::fmt::Display) {
        println!("{}[SUCCESS]{} {}", colors::GREEN, colors::RESET, msg);
    }

    pub fn info(msg: impl std::fmt::Display) {
        println!("{}[INFO]{} {}", colors::CYAN, colors::RESET, msg);
    }

    pub fn warn(msg: impl std::fmt::Display) {
        eprintln!("{}[WARN]{} {}", colors::AMBER, colors::RESET, msg);
    }

    pub fn error(msg: impl std::fmt::Display) {
        eprintln!("{}[ERROR]{} {}", colors::RED, colors::RESET, msg);
    }

    pub fn failed(msg: impl std::fmt::Display) {
        eprintln!("{}[FAILED]{} {}", colors::RED, colors::RESET, msg);
    }

    pub fn cancelled(msg: impl std::fmt::Display) {
        eprintln!("{}[CANCELLED]{} {}", colors::AMBER, colors::RESET, msg);
    }

    pub fn shutdown(msg: impl std::fmt::Display) {
        println!("{}[SHUTDOWN]{} {}", colors::AMBER, colors::RESET, msg);
    }

    pub fn shutdown_clean(msg: impl std::fmt::Display) {
        println!("{}[SHUTDOWN]{} {}", colors::GREEN, colors::RESET, msg);
    }

    pub fn revocation(msg: impl std::fmt::Display) {
        eprintln!("{}[REVOCATION DETECTED]{} {}", colors::AMBER, colors::RESET, msg);
    }

    pub fn self_healing(msg: impl std::fmt::Display) {
        println!("{}[SELF-HEALING RECOVERED]{} {}", colors::GREEN, colors::RESET, msg);
    }

    pub fn self_healing_failed(msg: impl std::fmt::Display) {
        eprintln!("{}[SELF-HEALING FAILED]{} {}", colors::RED, colors::RESET, msg);
    }

    pub fn header(title: impl std::fmt::Display) {
        println!(
            "{BOLD}============================================================{RESET}\n {BOLD}{}{RESET}\n{BOLD}============================================================{RESET}",
            title,
            BOLD = colors::BOLD,
            RESET = colors::RESET
        );
    }

    pub fn divider() {
        println!(
            "{MUTED}------------------------------------------------------------{RESET}",
            MUTED = colors::MUTED,
            RESET = colors::RESET
        );
    }

    pub fn key_val(key: impl std::fmt::Display, val: impl std::fmt::Display) {
        println!(" {:<15} {}", format!("{}:", key), val);
    }
}
