use crate::{atmb::model::Mailbox, smarty::AdditionalInfo, usps::UspsResult};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Record {
    pub name: String,
    pub street: String,
    pub street2: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub price: String,
    pub link: String,
    pub rdi: String,
    #[serde(rename = "CMRA")]
    pub cmra: String,
    pub smarty_status: String,
    pub detail_status: String,
    pub usps_status: String,
    pub usps_address: String,
    pub usps_zip5: String,
    pub usps_zip4: String,
    pub usps_dpv_confirmation: String,
    pub usps_cmra: String,
    pub usps_business: String,
    pub usps_carrier_route: String,
    pub usps_raw: String,
}
pub const HEADERS: [&str; 21] = [
    "name",
    "street",
    "street2",
    "city",
    "state",
    "zip",
    "price",
    "link",
    "rdi",
    "CMRA",
    "smarty_status",
    "detail_status",
    "usps_status",
    "usps_address",
    "usps_zip5",
    "usps_zip4",
    "usps_dpv_confirmation",
    "usps_cmra",
    "usps_business",
    "usps_carrier_route",
    "usps_raw",
];
impl Record {
    pub fn new(mailbox: Mailbox, info: AdditionalInfo, usps: UspsResult) -> Self {
        Self {
            zip: mailbox.address.full_zip(),
            name: mailbox.name,
            street: mailbox.address.line1,
            street2: mailbox.address.line2,
            city: mailbox.address.city,
            state: mailbox.address.state,
            price: mailbox.price,
            link: mailbox.link,
            rdi: info.rdi,
            cmra: info.cmra,
            smarty_status: info.status,
            detail_status: mailbox.detail_status,
            usps_status: usps.status,
            usps_address: usps.address,
            usps_zip5: usps.zip5,
            usps_zip4: usps.zip4,
            usps_dpv_confirmation: usps.dpv_confirmation,
            usps_cmra: usps.cmra,
            usps_business: usps.business,
            usps_carrier_route: usps.carrier_route,
            usps_raw: usps.raw,
        }
    }
    pub fn non_cmra(&self) -> bool {
        matches!(self.detail_status.as_str(), "fetched" | "listing_fallback")
            && self.smarty_status == "matched"
            && self.cmra == "N"
    }
    pub fn sort_key(&self) -> (u8, &str, &str, &str) {
        (
            match self.rdi.to_lowercase().as_str() {
                "residential" => 0,
                "commercial" => 1,
                _ => 2,
            },
            &self.state,
            &self.city,
            &self.link,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmb::model::Address;
    #[test]
    fn only_verified_non_cmra_enters_preferred_list() {
        let mailbox = Mailbox {
            name: "Example".into(),
            address: Address {
                line1: "1 Main St".into(),
                line2: "Unit 2".into(),
                city: "Concord".into(),
                state: "NH".into(),
                zip: "03301".into(),
                zip4: None,
            },
            price: "$9.99".into(),
            link: "https://example.com".into(),
            detail_status: "fetched".into(),
        };
        let mut record = Record::new(
            mailbox,
            AdditionalInfo {
                status: "matched".into(),
                cmra: "N".into(),
                rdi: "Residential".into(),
            },
            UspsResult::status("http_302"),
        );
        assert!(record.non_cmra());
        record.detail_status = "listing_fallback".into();
        assert!(
            record.non_cmra(),
            "A verified listing address must not be silently dropped"
        );
        record.cmra = "Unknown".into();
        assert!(!record.non_cmra());
        record.cmra = "Y".into();
        assert!(!record.non_cmra());
        record.cmra = "N".into();
        record.detail_status = "unavailable".into();
        assert!(!record.non_cmra());
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.serialize(&record).unwrap();
        let bytes = writer.into_inner().unwrap();
        let mut reader = csv::Reader::from_reader(bytes.as_slice());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            HEADERS
        );
    }
}
