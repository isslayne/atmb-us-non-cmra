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
pub const HEADERS: [&str; 20] = [
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
        self.smarty_status == "matched" && self.cmra == "N"
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
