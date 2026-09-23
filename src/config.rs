use color_eyre::eyre::{bail, eyre};

pub const TAX_FREE_STATES: [&str; 5] = ["AK", "DE", "MT", "NH", "OR"];
pub const STATES: [(&str, &str); 57] = [
    ("Alabama", "AL"),
    ("Alaska", "AK"),
    ("Arizona", "AZ"),
    ("Arkansas", "AR"),
    ("California", "CA"),
    ("Colorado", "CO"),
    ("Connecticut", "CT"),
    ("Delaware", "DE"),
    ("District of Columbia", "DC"),
    ("Florida", "FL"),
    ("Georgia", "GA"),
    ("Hawaii", "HI"),
    ("Idaho", "ID"),
    ("Illinois", "IL"),
    ("Indiana", "IN"),
    ("Iowa", "IA"),
    ("Kansas", "KS"),
    ("Kentucky", "KY"),
    ("Louisiana", "LA"),
    ("Maine", "ME"),
    ("Maryland", "MD"),
    ("Massachusetts", "MA"),
    ("Michigan", "MI"),
    ("Minnesota", "MN"),
    ("Mississippi", "MS"),
    ("Missouri", "MO"),
    ("Montana", "MT"),
    ("Nebraska", "NE"),
    ("Nevada", "NV"),
    ("New Hampshire", "NH"),
    ("New Jersey", "NJ"),
    ("New Mexico", "NM"),
    ("New York", "NY"),
    ("North Carolina", "NC"),
    ("North Dakota", "ND"),
    ("Ohio", "OH"),
    ("Oklahoma", "OK"),
    ("Oregon", "OR"),
    ("Pennsylvania", "PA"),
    ("Rhode Island", "RI"),
    ("South Carolina", "SC"),
    ("South Dakota", "SD"),
    ("Tennessee", "TN"),
    ("Texas", "TX"),
    ("Utah", "UT"),
    ("Vermont", "VT"),
    ("Virginia", "VA"),
    ("Washington", "WA"),
    ("West Virginia", "WV"),
    ("Wisconsin", "WI"),
    ("Wyoming", "WY"),
    ("American Samoa", "AS"),
    ("Guam", "GU"),
    ("Northern Mariana Islands", "MP"),
    ("Puerto Rico", "PR"),
    ("US Virgin Islands", "VI"),
    ("US Minor Outlying Islands", "UM"),
];

pub fn state_code(value: &str) -> Option<&'static str> {
    STATES
        .iter()
        .find(|(name, code)| {
            name.eq_ignore_ascii_case(value.trim()) || code.eq_ignore_ascii_case(value.trim())
        })
        .map(|(_, code)| *code)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    TaxFreeNv,
    TaxFree,
    All,
}
impl Scope {
    pub fn parse(value: &str) -> color_eyre::Result<Self> {
        match value {
            "tax-free-nv" => Ok(Self::TaxFreeNv),
            "tax-free" => Ok(Self::TaxFree),
            "all" => Ok(Self::All),
            _ => bail!("ADDRESS_SCOPE must be tax-free-nv, tax-free or all"),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::TaxFreeNv => "tax-free-nv",
            Self::TaxFree => "tax-free",
            Self::All => "all",
        }
    }
    pub fn includes(self, state: &str) -> bool {
        state_code(state).is_some_and(|code| {
            self == Self::All
                || TAX_FREE_STATES.contains(&code)
                || (self == Self::TaxFreeNv && code == "NV")
        })
    }
}

pub fn bounded_env(
    name: &str,
    default: usize,
    min: usize,
    max: usize,
) -> color_eyre::Result<usize> {
    let value = std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<usize>()
        .map_err(|_| eyre!("{name} must be an integer"))?;
    if !(min..=max).contains(&value) {
        bail!("{name} must be between {min} and {max}");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_and_abbreviations() {
        for state in TAX_FREE_STATES {
            assert!(Scope::TaxFree.includes(state));
            assert!(Scope::TaxFreeNv.includes(state));
        }
        assert!(Scope::TaxFree.includes("Oregon"));
        assert!(!Scope::TaxFree.includes("NV"));
        assert!(Scope::TaxFreeNv.includes("Nevada"));
        assert!(Scope::TaxFreeNv.includes("NV"));
        assert!(!Scope::TaxFreeNv.includes("CA"));
        assert_eq!(Scope::parse("tax-free-nv").unwrap().name(), "tax-free-nv");
        assert!(Scope::All.includes("Nevada"));
        assert!(Scope::All.includes("Guam"));
        assert!(!Scope::TaxFree.includes("PR"));
        assert!(!Scope::All.includes("Ontario"));
        assert_eq!(state_code(" nevada "), Some("NV"));
        assert!(Scope::parse("typo").is_err());
    }
}
