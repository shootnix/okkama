# Immutable XML values and serialization

`xml/name::XmlName(local)` or `XmlName(namespace,local,prefix)` separates
namespace identity from lexical prefix. NCNames follow XML 1.0 fifth-edition
Unicode ranges, exclude colon and are bounded to 256 scalars. Namespace URIs
are bounded to 4,096 scalars. Empty local names, an unbound prefix, a reserved
xmlns URI/prefix and incorrect xml URI/prefix bindings throw InvalidXml.

`xml/node` defines XmlNode.xml()/depth(), XmlText(value), XmlComment(value),
XmlProcessingInstruction(target,data), XmlAttribute(name,value),
XmlElement(name,children), XmlElement(name,attributes,children) and
XmlDocument(root is XmlElement). Attributes use Sequencing<XmlAttribute>,
revising the unsupported nominal-key map candidate. Attribute and child accessors
return immutable ordered snapshots. Text stays in child nodes; there is no
inner_text or raw-markup constructor. Custom XmlNode implementations must obey
the immutable valid-fragment/depth contract.

Attribute namespaces require explicit prefixes; unprefixed attributes remain
namespace-free. Duplicate expanded attribute names are rejected even if their
prefixes differ. Prefix conflicts between element and attributes, or between
attributes, fail. Each element declares its own namespace bindings, including
an empty default declaration where needed, so child fragments cannot accidentally
inherit a parent's default namespace. Declarations follow the element binding,
then first attribute use; attribute and child order remain unchanged. Prefix is
serialization metadata and is excluded from duplicate-name identity.

`xml/write::EscapedXml(text,attribute).string()` validates XML 1.0 characters and
escapes text separately from attributes. Text CR uses a character reference;
attribute TAB/LF/CR also use references, preserving XML normalization-sensitive
data. XML-forbidden controls and U+FFFE/U+FFFF throw InvalidXml with scalar
offset. Comments reject `--` and a trailing hyphen; processing instructions use
NCName targets, reject case-insensitive xml and `?>` data. Comments/PI are literal
source content; parsing's line-ending normalization is not reversed.

Limits: 256 attributes, 8,192 direct children, element depth 64, comment/PI data
65,536 scalars, 32 Mi scalars per escaped value/element output. Aggregate sizes
are checked before joins. Construction binds a coherent validated representation;
xml() repeats it without rewalking the tree. Deep trees retain representations
at each level; allocation investigation.
XmlDocument adds an XML 1.0 / UTF-8 declaration and one nominal element root.
No pretty-printing whitespace, DTD, entity declarations, file or network access.

XML parsing is still a separate roadmap task. These modules neither claim parser
conformance nor accept untrusted source XML as an already-validated tree.

[XML 1.0 fifth edition](https://www.w3.org/TR/xml/),
[Namespaces in XML 1.0](https://www.w3.org/TR/xml-names/),
independent parser and boundary evidence.
