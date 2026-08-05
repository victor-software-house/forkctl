use std::env;
use std::io::{self, IsTerminal};
use std::time::Duration;
use update_informer::{Check, registry};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

pub fn available_notice() -> Option<String> {
    if !io::stderr().is_terminal() || env::var_os("FORKCTL_NO_UPDATE_CHECK").is_some() {
        return None;
    }

    let informer = update_informer::new(
        registry::Crates,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )
    .timeout(REQUEST_TIMEOUT);

    informer
        .check_version()
        .ok()
        .flatten()
        .map(|version| update_message(&version.to_string()))
}

fn update_message(version: &str) -> String {
    format!(
        "forkctl {version} is available; run `cargo install forkctl --locked` or update the pinned mise catalog"
    )
}

#[cfg(test)]
mod tests {
    use super::update_message;

    #[test]
    fn update_notice_is_short_and_actionable() {
        assert_eq!(
            update_message("1.2.3"),
            "forkctl 1.2.3 is available; run `cargo install forkctl --locked` or update the pinned mise catalog"
        );
    }
}
