use crate::{
    atmb::ATMBCrawl,
    config::{bounded_env, Scope},
    record::Record,
    smarty::SmartyClient,
    usps::{UspsClient, UspsResult},
};
use color_eyre::eyre::bail;
use log::info;
use std::{io::Write, path::Path};
mod atmb;
mod config;
mod record;
mod smarty;
mod usps;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("info")
        .init();
    if let Err(error) = run().await {
        log::error!("{error}");
        std::process::exit(1);
    }
}
async fn run() -> color_eyre::Result<()> {
    let scope =
        Scope::parse(&std::env::var("ADDRESS_SCOPE").unwrap_or_else(|_| "tax-free".into()))?;
    let max_addresses = bounded_env("MAX_ADDRESSES", 0, 0, 100_000)?;
    let interval = bounded_env("USPS_INTERVAL_MS", 1000, 0, 60_000)?;
    let usps_enabled = match std::env::var("USPS_ENABLED")
        .unwrap_or_else(|_| "true".into())
        .as_str()
    {
        "true" => true,
        "false" => false,
        _ => bail!("USPS_ENABLED must be true or false"),
    };
    let mut records = Vec::new();
    let mut smarty_errors = 0;
    let mut usps_errors = 0;
    let mut detail_errors = 0;
    if let Ok(path) = std::env::var("USPS_INPUT_CSV") {
        if !usps_enabled {
            bail!("USPS_INPUT_CSV requires USPS_ENABLED=true");
        }
        records = load_records(&path, scope)?;
        if max_addresses > 0 {
            records.truncate(max_addresses);
        }
        let mut usps = UspsClient::new(interval)?;
        for (index, record) in records.iter_mut().enumerate() {
            info!("USPS checking #{}: {}", index + 1, record.name);
            let result = usps.inquire(&record.address()).await;
            usps_errors += usize::from(result.service_error());
            detail_errors += usize::from(record.detail_status != "fetched");
            record.set_usps(result);
        }
    } else {
        // Validate credentials before spending time crawling.
        let mut smarty = SmartyClient::new()?;
        let mut usps = UspsClient::new(interval)?;
        let mailboxes = ATMBCrawl::new()?.fetch(scope, max_addresses).await?;
        info!("Fetched {} mailboxes ({})", mailboxes.len(), scope.name());
        for (index, mailbox) in mailboxes.into_iter().enumerate() {
            info!("Checking #{}: {}", index + 1, mailbox.name);
            if mailbox.detail_status != "fetched" {
                detail_errors += 1;
            }
            let smarty_info = smarty.inquire(&mailbox.address).await;
            let usps_info = if usps_enabled {
                usps.inquire(&mailbox.address).await
            } else {
                UspsResult::status("disabled")
            };
            smarty_errors += usize::from(smarty_info.failed());
            usps_errors += usize::from(usps_info.service_error());
            records.push(Record::new(mailbox, smarty_info, usps_info));
        }
    }
    records.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    // Limited smoke tests never replace a full scope's published output.
    let out_dir = if max_addresses > 0 {
        format!("result/smoke/{}", scope.name())
    } else {
        format!("result/{}", scope.name())
    };
    save_records(records.iter(), &format!("{out_dir}/checks.csv"))?;
    save_records(
        records.iter().filter(|r| r.non_cmra()),
        &format!("{out_dir}/mailboxes.csv"),
    )?;
    let non_cmra = records.iter().filter(|r| r.non_cmra()).count();
    let mut summary = format!("# Address check: {}\n\n- Checked: {}\n- Smarty non-CMRA: {}\n- Smarty errors: {}\n- USPS errors/unavailable: {}\n- Limited smoke test: {}\n\nFull per-address results, including USPS fields and raw JSON, are in `checks.csv`. USPS failures are not validation passes.\n", scope.name(), records.len(), non_cmra, smarty_errors, usps_errors, max_addresses > 0);
    if let Ok(source) = std::env::var("SOURCE_RUN_ID") {
        if !source.is_empty() && source.chars().all(|c| c.is_ascii_digit()) {
            summary.push_str(&format!(
                "\n- ATMB / Smarty data reused from prior run: {source} (not a fresh crawl)\n"
            ));
        }
    }
    summary.push_str(&format!(
        "\n- State-listing addresses used (detail enrichment unavailable): {detail_errors}\n"
    ));
    for (service, statuses) in [
        (
            "Smarty",
            records
                .iter()
                .map(|r| r.smarty_status.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "USPS",
            records
                .iter()
                .map(|r| r.usps_status.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        let mut counts = std::collections::BTreeMap::new();
        for status in statuses {
            *counts.entry(status).or_insert(0) += 1;
        }
        summary.push_str(&format!(
            "\n## {service} statuses\n\n| Status | Count |\n| --- | --- |\n"
        ));
        for (status, count) in counts {
            summary.push_str(&format!("| {status} | {count} |\n"));
        }
    }
    if records
        .iter()
        .any(|r| r.smarty_status == "http_402_subscription_required")
    {
        summary.push_str("\nSmarty returned HTTP 402: activate a US Street Address API subscription or select another credential Secret, then rerun.\n");
    }
    std::fs::write(format!("{out_dir}/summary.md"), &summary)?;
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(summary.as_bytes())?;
    }
    if usps_errors > 0 {
        bail!("USPS unavailable for {usps_errors} addresses; partial results saved for diagnosis, publication blocked");
    }
    if smarty_errors > 0 {
        bail!("Smarty failed for {smarty_errors} addresses; partial results saved for diagnosis, publication blocked");
    }
    Ok(())
}
fn load_records(path: &str, scope: Scope) -> color_eyre::Result<Vec<Record>> {
    let records = csv::Reader::from_path(path)?
        .deserialize()
        .collect::<Result<Vec<Record>, _>>()?;
    if records.is_empty()
        || records.iter().any(|r| {
            !scope.includes(&r.state)
                || r.street.trim().is_empty()
                || !matches!(r.smarty_status.as_str(), "matched" | "no_match")
        })
    {
        bail!("USPS input must contain complete, in-scope Smarty results");
    }
    Ok(records)
}
fn save_records<'a>(
    records: impl Iterator<Item = &'a Record>,
    path: &str,
) -> color_eyre::Result<()> {
    let path = Path::new(path);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temp = path.with_extension("csv.tmp");
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_path(&temp)?;
    writer.write_record(record::HEADERS)?;
    for record in records {
        writer.serialize(record)?;
    }
    writer.flush()?;
    std::fs::rename(temp, path)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_results_still_have_headers() {
        let path = std::env::temp_dir().join(format!("atmb-empty-{}.csv", std::process::id()));
        save_records(std::iter::empty(), path.to_str().unwrap()).unwrap();
        let mut reader = csv::Reader::from_path(&path).unwrap();
        assert_eq!(reader.headers().unwrap().len(), 21);
        assert_eq!(reader.records().count(), 0);
        std::fs::remove_file(path).unwrap();
    }
}
