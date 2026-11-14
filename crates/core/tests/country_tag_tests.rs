use engine_core::CountryTag;
use std::convert::TryFrom;

#[test]
fn test_valid_country_tags() {
    let tag_usa = CountryTag::try_from("USA").expect("USA should be valid 3-letter tag");
    assert_eq!(tag_usa.as_str(), "USA");
    assert_eq!(format!("{tag_usa}"), "USA");

    let tag_tex = CountryTag::new("TEX");
    assert_eq!(tag_tex.as_str(), "TEX");

    assert_eq!(tag_usa, CountryTag::new("USA"));
    assert_ne!(tag_usa, tag_tex);
}

#[test]
fn test_invalid_length_country_tags() {
    assert!(CountryTag::try_from("").is_err(), "Empty string must fail");
    assert!(CountryTag::try_from("U").is_err(), "1-char tag must fail");
    assert!(CountryTag::try_from("US").is_err(), "2-char tag must fail");
    assert!(
        CountryTag::try_from("USAA").is_err(),
        "4-char tag must fail"
    );
    assert!(
        CountryTag::try_from("UNITED_STATES").is_err(),
        "Long tag must fail"
    );
}
