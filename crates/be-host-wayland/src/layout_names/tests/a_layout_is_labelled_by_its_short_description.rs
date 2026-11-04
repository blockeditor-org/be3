use super::*;

const RULES: &str = "
<xkbConfigRegistry>
  <layoutList>
    <layout>
      <configItem>
        <name>us</name>
        <shortDescription>en</shortDescription>
        <description>English (US)</description>
      </configItem>
      <variantList>
        <variant>
          <configItem>
            <name>chr</name>
            <shortDescription>chr</shortDescription>
            <description>Cherokee</description>
          </configItem>
        </variant>
        <variant>
          <configItem>
            <name>intl</name>
            <description>English (US, intl., with dead keys)</description>
          </configItem>
        </variant>
      </variantList>
    </layout>
    <layout>
      <configItem>
        <name>ch</name>
        <shortDescription>de</shortDescription>
        <description>German (Switzerland)</description>
      </configItem>
      <variantList>
        <variant>
          <configItem>
            <name>us</name>
            <description>Swiss US</description>
          </configItem>
        </variant>
      </variantList>
    </layout>
  </layoutList>
</xkbConfigRegistry>
";

#[test]
fn a_layout_is_labelled_by_its_short_description() {
    assert_eq!(short_label(RULES, "us", ""), "EN");
    assert_eq!(
        short_label(RULES, "us", "chr"),
        "CHR",
        "a variant with a short description of its own is labelled by it"
    );
    assert_eq!(
        short_label(RULES, "us", "intl"),
        "EN",
        "a variant without one is labelled by its layout"
    );
    assert_eq!(
        short_label(RULES, "ch", ""),
        "DE",
        "a layout is found by its own name, not a variant's of the same name"
    );
    assert_eq!(
        short_label(RULES, "xyz", ""),
        "XYZ",
        "a layout the rules do not describe is labelled by its code"
    );
    assert_eq!(short_label("", "de", ""), "DE", "nor do missing rules");
}
