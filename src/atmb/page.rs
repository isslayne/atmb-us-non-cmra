use crate::atmb::model::{Address, Mailbox};
use color_eyre::eyre::{bail, eyre};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static STATE_LIST_REG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<a class='theme-simple-link' href='(.*?)'>(.*?)</a>"#).unwrap());

static LOCATION_CONTAINER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"div[class="theme-location-item"]"#).unwrap());
static LOCATION_TITLE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"h3[class="t-title"]"#).unwrap());
static LOCATION_PRICE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"div[class="t-price"]"#).unwrap());
static LOCATION_ADDRESS_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"div[class="t-addr"]"#).unwrap());
static LOCATION_PLAN_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"a[class~="gt-plan"]"#).unwrap());
static LOCATION_DETAIL_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"div[class="t-sec1"] div[class="t-text"]"#).unwrap());

/// ATMB country page. i.e. https://www.anytimemailbox.com/l/usa
#[derive(Debug)]
pub struct CountryPage<'a> {
    pub states: Vec<StateHtmlInfo<'a>>,
}

#[derive(Debug)]
pub struct StateHtmlInfo<'a> {
    sub_url: &'a str,
    name: &'a str,
}

impl StateHtmlInfo<'_> {
    pub fn url(&self) -> &str {
        self.sub_url
    }

    pub fn name(&self) -> &str {
        self.name
    }
}

impl<'a> CountryPage<'a> {
    /// get state list from the country page
    pub fn parse_html(html: &'a str) -> color_eyre::Result<Self> {
        let mut states = Vec::new();

        for caps in STATE_LIST_REG.captures_iter(html) {
            if caps.len() != 3 {
                bail!(
                    "Unexpected capture length: {}, page structure might be changed",
                    caps.len()
                );
            }
            if !caps[1].starts_with("/l/usa/") {
                continue;
            }
            states.push(StateHtmlInfo {
                sub_url: caps.get(1).unwrap().as_str(),
                name: caps.get(2).unwrap().as_str(),
            });
        }
        if states.is_empty() {
            bail!("No state found, page structure might be changed");
        }
        Ok(Self { states })
    }
}

/// ATMB state page. i.e. https://www.anytimemailbox.com/l/usa/alabama
pub struct StatePage {
    locations: Vec<LocationHtmlInfo>,
}

impl StatePage {
    pub fn len(&self) -> usize {
        self.locations.len()
    }
}

#[derive(Debug, Clone)]
pub struct LocationHtmlInfo {
    name: String,
    /// street address
    line1: String,
    /// city, state, zip
    line2: String,
    price: String,
    link: String,
}

impl StatePage {
    pub fn parse_html(html: &str) -> color_eyre::Result<Self> {
        let mut locations = Vec::new();

        let document = Html::parse_document(html);
        let location_container = document.select(&LOCATION_CONTAINER_SELECTOR);

        for location_fragment in location_container {
            let title = location_fragment
                .select(&LOCATION_TITLE_SELECTOR)
                .next()
                .ok_or_else(|| eyre!("No title found - {}", location_fragment.html()))?
                .text()
                .collect::<String>();
            let price = location_fragment
                .select(&LOCATION_PRICE_SELECTOR)
                .next()
                .ok_or_else(|| eyre!("No price found - {}", location_fragment.html()))?
                .text()
                .collect::<String>();
            let address = location_fragment
                .select(&LOCATION_ADDRESS_SELECTOR)
                .next()
                .ok_or_else(|| eyre!("No address found - {}", location_fragment.html()))?
                .inner_html();
            let (line1, line2) = Self::split_address(&address)
                .ok_or_else(|| eyre!("Failed to split address - {}", address))?;
            let plan_link = location_fragment
                .select(&LOCATION_PLAN_SELECTOR)
                .next()
                .ok_or_else(|| eyre!("No plan button found - {}", location_fragment.html()))?
                .value()
                .attr("href")
                .ok_or_else(|| eyre!("No plan link found - {}", location_fragment.html()))?;

            let location_link = format!("{}{}", super::BASE_URL, plan_link);
            locations.push(LocationHtmlInfo {
                name: title,
                line1: line1.to_string(),
                line2: line2.to_string(),
                price,
                link: location_link,
            });
        }

        if locations.is_empty() {
            bail!("No locations found on state page; page structure might have changed");
        }
        Ok(Self { locations })
    }

    pub fn to_mailboxes(&self) -> color_eyre::Result<Vec<Mailbox>> {
        self.locations
            .iter()
            .map(|location| location.clone().try_into())
            .collect()
    }

    fn split_address(address: &str) -> Option<(&str, &str)> {
        let mut segments = address.split("<br>").take(2);
        Some((segments.next()?, segments.next()?))
    }
}

impl LocationHtmlInfo {
    fn parse_city(&self) -> Option<&str> {
        self.line2.split(',').next()
    }

    fn parse_state(&self) -> Option<&str> {
        self.line2
            .split(',')
            .nth(1)
            .map(|s| s.trim())
            .and_then(|s| s.split_whitespace().next())
    }

    fn parse_zip(&self) -> Option<(&str, Option<&str>)> {
        fn try_split_zip(zip_str: &str) -> Option<(&str, Option<&str>)> {
            let mut segments = zip_str.split("-");
            let zip = segments.next()?;
            let zip4 = segments.next();
            Some((zip, zip4))
        }

        self.line2
            .split(',')
            .nth(1)
            .map(|s| s.trim())
            .and_then(|s| s.split_whitespace().nth(1))
            .and_then(|s| try_split_zip(s))
    }

    fn price(&self) -> String {
        self.price.replace("Starting from", "").replace(" ", "")
    }
}

/// ATMB location detail page. i.e. https://www.anytimemailbox.com/s/birmingham-120-19th-street-north
pub struct LocationDetailPage {
    /// street address
    pub line1: String,
    /// unit, suite, etc.
    pub line2: Option<String>,
}

impl LocationDetailPage {
    pub fn parse_html(html: &str) -> color_eyre::Result<Self> {
        let document = Html::parse_document(html);
        let address_container = document
            .select(&LOCATION_DETAIL_SELECTOR)
            .next()
            .ok_or_else(|| eyre!("Address detail container not found"))?;
        let div_selector = Selector::parse(":scope > div").unwrap();
        let placeholder_selector = Selector::parse(".t-placeholder").unwrap();
        let lines = address_container
            .select(&div_selector)
            .map(|div| {
                let mut text = div.text().collect::<String>();
                let placeholders = div.select(&placeholder_selector).collect::<Vec<_>>();
                for placeholder in &placeholders {
                    text = text.replace(&placeholder.text().collect::<String>(), "");
                }
                if !placeholders.is_empty() {
                    text = text.trim().trim_end_matches('#').trim().to_owned();
                    for suffix in ["PMB", "Suite", "Ste", "Unit", "Apt"] {
                        if text.eq_ignore_ascii_case(suffix) {
                            text.clear();
                            break;
                        }
                        if text
                            .to_lowercase()
                            .ends_with(&format!(" {}", suffix.to_lowercase()))
                        {
                            text.truncate(text.len() - suffix.len());
                            break;
                        }
                    }
                }
                text.trim().to_owned()
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        // Omit only the unassigned mailbox placeholder, preserving real suites and units.
        let city_index = lines
            .iter()
            .position(|line| CITY_STATE_ZIP.is_match(line))
            .ok_or_else(|| eyre!("City/state/ZIP line not found in address detail"))?;
        if city_index == 0 {
            bail!("Street line not found in address detail");
        }
        Ok(Self {
            line1: lines[0].clone(),
            line2: (city_index > 1).then(|| lines[1..city_index].join(" ")),
        })
    }
}

static CITY_STATE_ZIP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r",\s*[A-Z]{2}\s+\d{5}(?:-\d{4})?\s*$").unwrap());

impl TryInto<Address> for LocationHtmlInfo {
    type Error = color_eyre::eyre::Error;

    fn try_into(self) -> Result<Address, Self::Error> {
        let (zip, zip4) = self
            .parse_zip()
            .ok_or_else(|| eyre!("Failed to parse zip code from: {}", self.line2))?;
        Ok(Address {
            city: self
                .parse_city()
                .ok_or_else(|| eyre!("Failed to parse city from: {}", self.line2))?
                .to_string(),
            state: crate::config::state_code(
                self.parse_state().ok_or_else(|| eyre!("Missing state"))?,
            )
            .ok_or_else(|| eyre!("Invalid US state"))?
            .to_owned(),
            zip: zip.to_owned(),
            zip4: zip4.map(|s| s.to_owned()),
            line1: self.line1,
            line2: String::new(),
        })
    }
}

impl TryInto<Mailbox> for LocationHtmlInfo {
    type Error = color_eyre::eyre::Error;

    fn try_into(self) -> Result<Mailbox, Self::Error> {
        Ok(Mailbox {
            address: self.clone().try_into()?,
            price: self.price(),
            name: self.name,
            link: self.link,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detail_keeps_real_unit_and_removes_unassigned_mailbox() {
        let page = LocationDetailPage::parse_html(include_str!("../../tests/fixtures/detail.html"))
            .unwrap();
        assert_eq!(page.line1, "5953 Mabel Rd");
        assert_eq!(page.line2.as_deref(), Some("Unit 236"));
        assert!(LocationDetailPage::parse_html("<html>blocked</html>").is_err());
    }
    #[test]
    fn mailbox_placeholder_on_street_line_preserves_street() {
        let html = "<div class='t-sec1'><div class='t-text'><div><span class='t-placeholder'>YOUR NAME</span></div><div>243 E 5th Ave #<span class='t-placeholder'>MAILBOX</span></div><div>Anchorage, AK 99501</div><div>United States</div></div></div>";
        let page = LocationDetailPage::parse_html(html).unwrap();
        assert_eq!(page.line1, "243 E 5th Ave");
        assert!(page.line2.is_none());
    }
    #[test]
    fn only_us_state_links_are_collected() {
        let page = CountryPage::parse_html("<a class='theme-simple-link' href='/l/usa/oregon'>Oregon</a><a class='theme-simple-link' href='/l/canada/ontario'>Ontario</a>").unwrap();
        assert_eq!(page.states.len(), 1);
    }
    #[test]
    fn state_listing_preserves_leading_zero_zip_and_rejects_empty_page() {
        let html = r#"<div class="theme-location-item"><h3 class="t-title">Test</h3><div class="t-price">Starting from US$ 9.99</div><div class="t-addr">10 Main St<br/>Concord, NH 03301-1234<br/></div><a class="gt-plan" href="/s/test">Select</a></div>"#;
        let boxes = StatePage::parse_html(html).unwrap().to_mailboxes().unwrap();
        assert_eq!(boxes[0].address.full_zip(), "03301-1234");
        assert!(StatePage::parse_html("<html>blocked</html>").is_err());
    }
}
