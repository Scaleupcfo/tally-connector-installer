//! `List of Loaded Companies` — name + books period for every company
//! currently open in Tally Prime.

use quick_xml::{Reader, events::Event};
use serde::Serialize;

use super::TallyError;
use super::client;
use super::dates;

/// A company currently loaded in Tally.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct Company {
    /// Exact internal name — feed back as `SVCURRENTCOMPANY` when querying.
    pub name: String,
    /// First date the company's books cover, ISO 8601 (`YYYY-MM-DD`) or null.
    pub books_start: Option<String>,
    /// Last date the company's books cover, ISO 8601 (`YYYY-MM-DD`) or null.
    pub books_end: Option<String>,
}

const ENVELOPE: &str = r#"<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>ListOfLoadedCompanies</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="ListOfLoadedCompanies" ISINITIALIZE="Yes">
            <TYPE>Company</TYPE>
            <FETCH>Name, StartingFrom, EndingAt</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>"#;

pub fn list_companies() -> Result<Vec<Company>, TallyError> {
    let body = client::post_xml(ENVELOPE)?;
    parse(&body)
}

#[derive(Copy, Clone)]
enum Field {
    Name,
    StartingFrom,
    EndingAt,
}

fn parse(xml: &str) -> Result<Vec<Company>, TallyError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut companies: Vec<Company> = Vec::new();
    let mut current: Option<Company> = None;
    let mut current_field: Option<Field> = None;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"COMPANY" => {
                    let mut new_co = Company::default();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"NAME" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                new_co.name = v.trim().to_string();
                            }
                        }
                    }
                    current = Some(new_co);
                }
                b"NAME" if current.is_some() => current_field = Some(Field::Name),
                b"STARTINGFROM" if current.is_some() => current_field = Some(Field::StartingFrom),
                b"ENDINGAT" if current.is_some() => current_field = Some(Field::EndingAt),
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let (Some(c), Some(field)) = (current.as_mut(), current_field) {
                    let text = t.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        match field {
                            Field::Name => {
                                if c.name.is_empty() {
                                    c.name = text;
                                }
                            }
                            Field::StartingFrom => c.books_start = dates::from_yyyymmdd(&text),
                            Field::EndingAt => c.books_end = dates::from_yyyymmdd(&text),
                        }
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"COMPANY" => {
                    if let Some(c) = current.take() {
                        if !c.name.is_empty() && !companies.iter().any(|x| x.name == c.name) {
                            companies.push(c);
                        }
                    }
                    current_field = None;
                }
                b"NAME" | b"STARTINGFROM" | b"ENDINGAT" => current_field = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(TallyError::BadXml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(companies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_form() {
        let xml = r#"<ENVELOPE>
            <COMPANY NAME="SHEP LIMITED">
                <STARTINGFROM>20230401</STARTINGFROM>
                <ENDINGAT>20250331</ENDINGAT>
            </COMPANY>
            <COMPANY NAME="HPC LTD">
                <STARTINGFROM>20220401</STARTINGFROM>
                <ENDINGAT>20260331</ENDINGAT>
            </COMPANY>
        </ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "SHEP LIMITED");
        assert_eq!(got[0].books_start.as_deref(), Some("2023-04-01"));
        assert_eq!(got[1].name, "HPC LTD");
    }

    #[test]
    fn element_form() {
        let xml = r#"<ENVELOPE><COMPANY>
            <NAME>HPC LTD</NAME>
            <STARTINGFROM>20220401</STARTINGFROM>
            <ENDINGAT>20260331</ENDINGAT>
        </COMPANY></ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "HPC LTD");
    }

    #[test]
    fn dedupes_by_name() {
        let xml = r#"<ENVELOPE>
            <COMPANY NAME="X"></COMPANY>
            <COMPANY NAME="X"></COMPANY>
        </ENVELOPE>"#;
        assert_eq!(parse(xml).unwrap().len(), 1);
    }

    #[test]
    fn ignores_name_outside_company() {
        // <COLLECTION NAME="..."> must not leak into our list.
        let xml = r#"<ENVELOPE>
            <COLLECTION NAME="foo"><NAME>bar</NAME></COLLECTION>
            <COMPANY NAME="HPC LTD"></COMPANY>
        </ENVELOPE>"#;
        let got = parse(xml).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "HPC LTD");
    }
}
