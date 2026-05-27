//! Master ledger list for a specific company.
//!
//! Returns one entry per ledger account (Vendor X, Bank Y, Sales A/c, GST...)
//! with opening/closing balance and party-identity fields (GSTIN, mailing
//! address, etc.).
//!
//! Phase 6b ships the most-used fields. Phase 6c can add: BILLCREDITPERIOD,
//! INCOMETAXNUMBER, EMAIL/PHONE/MOBILE, MAILINGNAME, LEDSTATENAME, COUNTRYNAME.

use quick_xml::{Reader, events::Event};
use serde::Serialize;

use super::TallyError;
use super::client;
use super::sanitize::xml_escape;

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Amount {
    /// As Tally emitted it (preserve the sign — negative = debit in Tally's convention).
    pub raw: String,
    /// Best-effort float parse. `None` if the raw string didn't parse.
    pub value: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Ledger {
    pub name: String,
    pub parent_group: Option<String>,
    pub opening_balance: Option<Amount>,
    pub closing_balance: Option<Amount>,
    pub is_bill_wise: bool,
    pub is_deemed_positive: bool,
    pub gst_registration_type: Option<String>,
    pub party_gstin: Option<String>,
}

const ENVELOPE_TEMPLATE: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>LedgerMasters</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>{company}</SVCURRENTCOMPANY>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="LedgerMasters" ISINITIALIZE="Yes">
            <TYPE>Ledger</TYPE>
            <FETCH>NAME, PARENT, OPENINGBALANCE, CLOSINGBALANCE, ISBILLWISEON,
                   ISDEEMEDPOSITIVE, GSTREGISTRATIONTYPE, PARTYGSTIN</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>"#;

pub fn list_ledgers(company: &str) -> Result<Vec<Ledger>, TallyError> {
    if company.trim().is_empty() {
        return Err(TallyError::BadRequest("company name is required".into()));
    }
    let envelope = ENVELOPE_TEMPLATE.replace("{company}", &xml_escape(company));
    let body = client::post_xml(&envelope)?;
    parse(&body)
}

#[derive(Copy, Clone)]
enum Field {
    Parent,
    OpeningBalance,
    ClosingBalance,
    IsBillWise,
    IsDeemedPositive,
    GstRegType,
    PartyGstin,
    Name,
}

fn parse_amount(raw: &str) -> Amount {
    let trimmed = raw.trim().to_string();
    let value = trimmed.parse::<f64>().ok();
    Amount {
        raw: trimmed,
        value,
    }
}

fn yes_no(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("yes")
}

fn parse(xml: &str) -> Result<Vec<Ledger>, TallyError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut ledgers: Vec<Ledger> = Vec::new();
    let mut current: Option<Ledger> = None;
    let mut current_field: Option<Field> = None;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"LEDGER" => {
                    let mut new_l = Ledger::default();
                    // Name commonly arrives as an attribute on <LEDGER NAME="..."> .
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"NAME" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                new_l.name = v.trim().to_string();
                            }
                        }
                    }
                    current = Some(new_l);
                }
                b"NAME" if current.is_some() => current_field = Some(Field::Name),
                b"PARENT" if current.is_some() => current_field = Some(Field::Parent),
                b"OPENINGBALANCE" if current.is_some() => {
                    current_field = Some(Field::OpeningBalance)
                }
                b"CLOSINGBALANCE" if current.is_some() => {
                    current_field = Some(Field::ClosingBalance)
                }
                b"ISBILLWISEON" if current.is_some() => current_field = Some(Field::IsBillWise),
                b"ISDEEMEDPOSITIVE" if current.is_some() => {
                    current_field = Some(Field::IsDeemedPositive)
                }
                b"GSTREGISTRATIONTYPE" if current.is_some() => {
                    current_field = Some(Field::GstRegType)
                }
                b"PARTYGSTIN" if current.is_some() => current_field = Some(Field::PartyGstin),
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let (Some(l), Some(field)) = (current.as_mut(), current_field) {
                    let text = t.unescape().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        // Skip — keeps Option fields as None instead of Some("").
                    } else {
                        match field {
                            Field::Name => {
                                if l.name.is_empty() {
                                    l.name = text;
                                }
                            }
                            Field::Parent => l.parent_group = Some(text),
                            Field::OpeningBalance => l.opening_balance = Some(parse_amount(&text)),
                            Field::ClosingBalance => l.closing_balance = Some(parse_amount(&text)),
                            Field::IsBillWise => l.is_bill_wise = yes_no(&text),
                            Field::IsDeemedPositive => l.is_deemed_positive = yes_no(&text),
                            Field::GstRegType => l.gst_registration_type = Some(text),
                            Field::PartyGstin => l.party_gstin = Some(text),
                        }
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"LEDGER" => {
                    if let Some(l) = current.take() {
                        if !l.name.is_empty() {
                            ledgers.push(l);
                        }
                    }
                    current_field = None;
                }
                _ => current_field = None,
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(TallyError::BadXml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(ledgers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_ledger() {
        let xml = r#"<ENVELOPE><LEDGER NAME="Vendor A">
            <PARENT>Sundry Creditors</PARENT>
            <OPENINGBALANCE>1000.50</OPENINGBALANCE>
            <CLOSINGBALANCE>-2500.00</CLOSINGBALANCE>
            <ISBILLWISEON>Yes</ISBILLWISEON>
            <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
            <GSTREGISTRATIONTYPE>Regular</GSTREGISTRATIONTYPE>
            <PARTYGSTIN>27AAAPL1234C1Z5</PARTYGSTIN>
        </LEDGER></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got.len(), 1);
        let l = &got[0];
        assert_eq!(l.name, "Vendor A");
        assert_eq!(l.parent_group.as_deref(), Some("Sundry Creditors"));
        assert_eq!(l.opening_balance.as_ref().unwrap().value, Some(1000.50));
        assert_eq!(l.closing_balance.as_ref().unwrap().value, Some(-2500.00));
        assert!(l.is_bill_wise);
        assert!(l.is_deemed_positive);
        assert_eq!(l.gst_registration_type.as_deref(), Some("Regular"));
        assert_eq!(l.party_gstin.as_deref(), Some("27AAAPL1234C1Z5"));
    }

    #[test]
    fn parses_name_as_element() {
        let xml = r#"<ENVELOPE><LEDGER>
            <NAME>Bank A/c</NAME>
            <PARENT>Bank Accounts</PARENT>
        </LEDGER></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got[0].name, "Bank A/c");
        assert_eq!(got[0].parent_group.as_deref(), Some("Bank Accounts"));
    }

    #[test]
    fn skips_ledger_without_name() {
        let xml = r#"<ENVELOPE>
            <LEDGER><PARENT>Orphan</PARENT></LEDGER>
            <LEDGER NAME="OK"></LEDGER>
        </ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "OK");
    }

    #[test]
    fn amount_preserves_raw_and_parses_value() {
        let xml = r#"<ENVELOPE><LEDGER NAME="X"><OPENINGBALANCE>  -1500.25  </OPENINGBALANCE></LEDGER></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        let amt = got[0].opening_balance.as_ref().unwrap();
        assert_eq!(amt.raw, "-1500.25");
        assert_eq!(amt.value, Some(-1500.25));
    }
}
