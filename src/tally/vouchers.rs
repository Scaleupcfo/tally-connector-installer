//! Vouchers for one company, filtered by date range.
//!
//! Phase 6b: ships voucher headers + ledger entries (the bookkeeping core).
//! Phase 6c will add: bill_allocations, bank_allocations, inventory_entries,
//! dispatch_details, gst_info, tax_summary. Those exist in the Python at
//! tally-integration/fetch_tally_data.py and can be ported field-for-field.
//!
//! Approach: walk the XML one event at a time. We track:
//!   * `path` — stack of currently-open element names
//!   * `current_voucher` — the Voucher being assembled (if any)
//!   * `current_ledger` — the LedgerEntry being assembled (if any)
//! The "what field does this text belong to?" decision is made by looking
//! at the most recent open element AND whether we're inside a ledger entry.

use quick_xml::{Reader, events::Event};
use serde::Serialize;

use super::TallyError;
use super::client;
use super::dates;
use super::ledgers::Amount;
use super::sanitize::xml_escape;

// ---------- Structs ---------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Voucher {
    pub guid: Option<String>,
    pub alter_id: Option<String>,
    pub master_id: Option<String>,
    pub vch_key: Option<String>,
    /// ISO 8601 `YYYY-MM-DD`, or null if Tally didn't supply one.
    pub date: Option<String>,
    pub voucher_type: Option<String>,
    pub voucher_number: Option<String>,
    pub reference: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub party_ledger_name: Option<String>,
    pub is_cancelled: bool,
    pub is_optional: bool,
    pub entered_by: Option<String>,
    pub ledger_entries: Vec<LedgerEntry>,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct LedgerEntry {
    pub ledger_name: String,
    pub amount: Option<Amount>,
    pub is_deemed_positive: bool,
    pub is_party_ledger: bool,
}

// ---------- Public API ------------------------------------------------------

/// Pull all vouchers for `company` between `from` and `to` (ISO dates).
pub fn list_vouchers(company: &str, from: &str, to: &str) -> Result<Vec<Voucher>, TallyError> {
    if company.trim().is_empty() {
        return Err(TallyError::BadRequest("company name is required".into()));
    }
    let from_tally = dates::to_yyyymmdd(from)
        .ok_or_else(|| TallyError::BadRequest(format!("invalid from date: {from:?} (need YYYY-MM-DD)")))?;
    let to_tally = dates::to_yyyymmdd(to)
        .ok_or_else(|| TallyError::BadRequest(format!("invalid to date: {to:?} (need YYYY-MM-DD)")))?;

    let envelope = ENVELOPE_TEMPLATE
        .replace("{company}", &xml_escape(company))
        .replace("{from_date}", &from_tally)
        .replace("{to_date}", &to_tally);
    let body = client::post_xml(&envelope)?;
    parse(&body)
}

// ---------- XML envelope ----------------------------------------------------

const ENVELOPE_TEMPLATE: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>RangeVouchers</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>{company}</SVCURRENTCOMPANY>
        <SVFROMDATE TYPE="Date">{from_date}</SVFROMDATE>
        <SVTODATE TYPE="Date">{to_date}</SVTODATE>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="RangeVouchers" ISINITIALIZE="Yes">
            <TYPE>Voucher</TYPE>
            <FETCH>Date, VoucherTypeName, VoucherNumber, Narration, Reference, ReferenceDate,
                   PartyLedgerName, PartyName, IsCancelled, IsOptional, GUID, AlterID, MasterID, VoucherKey,
                   EnteredBy,
                   AllLedgerEntries.List, LedgerEntries.List</FETCH>
            <FILTER>InDateRange</FILTER>
          </COLLECTION>
          <SYSTEM TYPE="Formulae" NAME="InDateRange"><![CDATA[$Date >= $$Date:##SVFROMDATE AND $Date <= $$Date:##SVTODATE]]></SYSTEM>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>"#;

// ---------- Parser ----------------------------------------------------------

fn yes_no(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("yes")
}

fn parse_amount(raw: &str) -> Amount {
    let trimmed = raw.trim().to_string();
    let value = trimmed.parse::<f64>().ok();
    Amount {
        raw: trimmed,
        value,
    }
}

/// Names of XML elements that mark a new ledger-entry sub-structure.
fn is_ledger_entry_open(name: &[u8]) -> bool {
    name == b"ALLLEDGERENTRIES.LIST" || name == b"LEDGERENTRIES.LIST"
}

fn parse(xml: &str) -> Result<Vec<Voucher>, TallyError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut vouchers: Vec<Voucher> = Vec::new();
    let mut current_voucher: Option<Voucher> = None;
    let mut current_ledger: Option<LedgerEntry> = None;
    // Stack of currently-open element names — last() == "what element are we directly inside?"
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"VOUCHER" => {
                        let mut v = Voucher::default();
                        // VCHTYPE on the opening tag is the most common way Tally signals voucher type.
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"VCHTYPE" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    v.voucher_type = Some(s.trim().to_string());
                                }
                            }
                        }
                        current_voucher = Some(v);
                    }
                    n if is_ledger_entry_open(n) && current_voucher.is_some() => {
                        current_ledger = Some(LedgerEntry::default());
                    }
                    _ => {}
                }
                path.push(name);
            }
            Ok(Event::Text(t)) => {
                let Some(parent) = path.last() else {
                    continue;
                };
                let text = t.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }

                // Branch 1: we're inside a ledger entry.
                if let Some(le) = current_ledger.as_mut() {
                    match parent.as_slice() {
                        b"LEDGERNAME" => le.ledger_name = text,
                        b"AMOUNT" => le.amount = Some(parse_amount(&text)),
                        b"ISDEEMEDPOSITIVE" => le.is_deemed_positive = yes_no(&text),
                        b"ISPARTYLEDGER" => le.is_party_ledger = yes_no(&text),
                        _ => {}
                    }
                    continue;
                }

                // Branch 2: we're inside a voucher (but not a ledger entry).
                if let Some(v) = current_voucher.as_mut() {
                    match parent.as_slice() {
                        b"GUID" => v.guid = Some(text),
                        b"ALTERID" => v.alter_id = Some(text),
                        b"MASTERID" => v.master_id = Some(text),
                        // Python read VCHKEY; correct element is VOUCHERKEY per REVIEW_QUEUE.md.
                        b"VOUCHERKEY" | b"VCHKEY" => v.vch_key = Some(text),
                        b"DATE" => v.date = dates::from_yyyymmdd(&text),
                        b"VOUCHERTYPENAME" => {
                            if v.voucher_type.is_none() {
                                v.voucher_type = Some(text);
                            }
                        }
                        b"VOUCHERNUMBER" => v.voucher_number = Some(text),
                        b"REFERENCE" => v.reference = Some(text),
                        b"REFERENCEDATE" => v.reference_date = dates::from_yyyymmdd(&text),
                        b"NARRATION" => v.narration = Some(text),
                        // Tally puts the party name in PARTYLEDGERNAME, sometimes PARTYNAME.
                        b"PARTYLEDGERNAME" => v.party_ledger_name = Some(text),
                        b"PARTYNAME" => {
                            if v.party_ledger_name.is_none() {
                                v.party_ledger_name = Some(text);
                            }
                        }
                        b"ISCANCELLED" => v.is_cancelled = yes_no(&text),
                        b"ISOPTIONAL" => v.is_optional = yes_no(&text),
                        b"ENTEREDBY" => v.entered_by = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                // Pop the path stack regardless of which element this is.
                if path.last() == Some(&name) {
                    path.pop();
                }
                match name.as_slice() {
                    n if is_ledger_entry_open(n) => {
                        if let (Some(le), Some(v)) =
                            (current_ledger.take(), current_voucher.as_mut())
                        {
                            // Filter out empty entries (CMPINFO-style metadata).
                            if !le.ledger_name.is_empty() || le.amount.is_some() {
                                v.ledger_entries.push(le);
                            }
                        }
                    }
                    b"VOUCHER" => {
                        if let Some(v) = current_voucher.take() {
                            // Skip the phantom CMPINFO voucher (REVIEW_QUEUE.md #1):
                            // it carries no GUID, no date, no voucher type.
                            if v.guid.is_some() || v.date.is_some() || v.voucher_type.is_some() {
                                vouchers.push(v);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(TallyError::BadXml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(vouchers)
}

// ---------- Tests -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<ENVELOPE><BODY><DATA><COLLECTION>
        <VOUCHER VCHTYPE="Purchase">
            <DATE>20240501</DATE>
            <GUID>abc-123</GUID>
            <MASTERID>900</MASTERID>
            <VOUCHERKEY>vk-1</VOUCHERKEY>
            <VOUCHERNUMBER>P/001</VOUCHERNUMBER>
            <NARRATION>Office stationery</NARRATION>
            <PARTYLEDGERNAME>Vendor A</PARTYLEDGERNAME>
            <ISCANCELLED>No</ISCANCELLED>
            <ISOPTIONAL>No</ISOPTIONAL>
            <ALLLEDGERENTRIES.LIST>
                <LEDGERNAME>Vendor A</LEDGERNAME>
                <AMOUNT>10000.00</AMOUNT>
                <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                <ISPARTYLEDGER>Yes</ISPARTYLEDGER>
            </ALLLEDGERENTRIES.LIST>
            <ALLLEDGERENTRIES.LIST>
                <LEDGERNAME>Office Stationery A/c</LEDGERNAME>
                <AMOUNT>-10000.00</AMOUNT>
                <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
                <ISPARTYLEDGER>No</ISPARTYLEDGER>
            </ALLLEDGERENTRIES.LIST>
        </VOUCHER>
        <VOUCHER VCHTYPE="Receipt">
            <DATE>20240502</DATE>
            <GUID>def-456</GUID>
            <VOUCHERNUMBER>R/001</VOUCHERNUMBER>
            <ISCANCELLED>Yes</ISCANCELLED>
        </VOUCHER>
    </COLLECTION></DATA></BODY></ENVELOPE>"#;

    #[test]
    fn parses_two_vouchers() {
        let got = parse(SAMPLE).unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn pulls_voucher_header_fields() {
        let got = parse(SAMPLE).unwrap();
        let v = &got[0];
        assert_eq!(v.guid.as_deref(), Some("abc-123"));
        assert_eq!(v.master_id.as_deref(), Some("900"));
        assert_eq!(v.vch_key.as_deref(), Some("vk-1"));
        assert_eq!(v.date.as_deref(), Some("2024-05-01"));
        assert_eq!(v.voucher_type.as_deref(), Some("Purchase"));
        assert_eq!(v.voucher_number.as_deref(), Some("P/001"));
        assert_eq!(v.narration.as_deref(), Some("Office stationery"));
        assert_eq!(v.party_ledger_name.as_deref(), Some("Vendor A"));
        assert!(!v.is_cancelled);
        assert!(!v.is_optional);
    }

    #[test]
    fn detects_cancelled_voucher() {
        let got = parse(SAMPLE).unwrap();
        assert!(got[1].is_cancelled);
    }

    #[test]
    fn pulls_ledger_entries_with_signs() {
        let got = parse(SAMPLE).unwrap();
        let v = &got[0];
        assert_eq!(v.ledger_entries.len(), 2);
        assert_eq!(v.ledger_entries[0].ledger_name, "Vendor A");
        assert_eq!(v.ledger_entries[0].amount.as_ref().unwrap().value, Some(10000.00));
        assert!(v.ledger_entries[0].is_party_ledger);
        assert!(!v.ledger_entries[0].is_deemed_positive);
        assert_eq!(v.ledger_entries[1].ledger_name, "Office Stationery A/c");
        assert_eq!(v.ledger_entries[1].amount.as_ref().unwrap().value, Some(-10000.00));
        assert!(!v.ledger_entries[1].is_party_ledger);
        assert!(v.ledger_entries[1].is_deemed_positive);
    }

    #[test]
    fn empty_voucher_with_no_signals_is_dropped() {
        // CMPINFO-style phantom: VOUCHER element with literally nothing in it.
        let xml = r#"<ENVELOPE><VOUCHER></VOUCHER></ENVELOPE>"#;
        assert_eq!(parse(xml).unwrap().len(), 0);
    }

    #[test]
    fn voucher_type_from_element_when_attribute_missing() {
        let xml = r#"<ENVELOPE><VOUCHER>
            <DATE>20240601</DATE>
            <GUID>g</GUID>
            <VOUCHERTYPENAME>Journal</VOUCHERTYPENAME>
        </VOUCHER></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got[0].voucher_type.as_deref(), Some("Journal"));
    }

    #[test]
    fn falls_back_to_partyname_when_partyledgername_absent() {
        let xml = r#"<ENVELOPE><VOUCHER>
            <GUID>g</GUID>
            <DATE>20240601</DATE>
            <PARTYNAME>Walk-in Customer</PARTYNAME>
        </VOUCHER></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got[0].party_ledger_name.as_deref(), Some("Walk-in Customer"));
    }
}
