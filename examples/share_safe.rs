//! Build share-safe transformed sources from scan results.
//!
//! Run with:
//!
//! ```text
//! cargo run --example share_safe
//! ```

use cribra::{
    Rule, Scanner, Severity,
    transform::{ShareBundle, ShareMode},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "acme.api-key",
            "acme_live_",
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "internal.marker",
            "PRIVATE_VALUE",
            Severity::High,
        ))
        .build()?;

    let sources = ["ACME_API_KEY=acme_live_7f3a91\n", "marker=PRIVATE_VALUE\n"];

    let results = scanner.scan([("config.env", sources[0]), ("settings.env", sources[1])]);

    let bundle = ShareBundle::builder()
        .mode(ShareMode::Redact)
        .build(&results, sources)?;

    println!("{}", bundle.manifest());

    for source in &bundle {
        println!("--- {} ---", source.key());
        print!("{}", source.content());
    }

    Ok(())
}
