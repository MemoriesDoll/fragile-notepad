# Syntax configuration

`outline-parsers.xml` supplies the function list and previous/next/containing
function navigation. `folding-hints.xml` configures folding separately. These
assets are embedded at build time; editing the outline XML requires rebuilding.

The outline parser is a structural recognizer, not a complete language grammar.
Its shared scanner consumes compiled XML plans. Language-specific keywords and
lexical formats belong here rather than in language switches in Rust code.

The sidebar displays the outline tree, including containers without methods.
Its count and filter include both functions and types. Matching a container name
shows its children; matching a child retains its ancestor rows as context.
Enum declarations use `container kind="enum"` rules (Rust, TypeScript, Java,
Kotlin, C and C++). Their variants/constants are child symbols, separate from the
callable entries used by previous/next-function navigation.
Overlapping container rules sharing a body prefer the earliest header and the
longest keyword prefix, so a scoped enum is not also emitted as a class.
Callable rules can use `require-non-container-previous-kind` to require an
`identifier`, `qualified-identifier`, `template-type-tail`, or `array-type-tail`
before a method name while still admitting constructors. Array and generic
suffixes use the configured syntax roles; qualified names use
`qualified-separators`. Java uses these constraints to exclude enum constant
arguments from the function list.

## Families and bodies

A language selects a family through `use-family`. Existing adapter names remain
validated metadata; recognition uses the compiled rules and body kind.

Families may define `delimiter` elements with `open` and `close` attributes for
signature grouping. The scanner indexes matching pairs once, excluding comments
and literals. Brace bodies add their own configured pair to that index.

Callable and signature punctuation is configured through family `syntax-token`
elements, each with a `role` and a one-character `value` (Unicode is supported).
The roles are `parameters-open`, `parameters-close`, `brackets-open`,
`brackets-close`, `generics-open`, `generics-close`, `assignment`, `separator`,
and `statement-end`. `assignment-reject-before` and `assignment-reject-after`
may be repeated to exclude compound operators from plain assignments. Parameter
and bracket pairs also need corresponding `delimiter` elements for matching.
For example:

```xml
<syntax-token role="parameters-open" value="(" />
<syntax-token role="parameters-close" value=")" />
<delimiter open="(" close=")" />
```

Body attributes:

| Kind | Attributes | Behavior |
| --- | --- | --- |
| `brace` | `open`, `close` | Match the configured body delimiters, including multi-byte delimiters. |
| `indent` | `header-end`, `line-continuation` | Find the header terminator outside grouped expressions; follow the indented body across comments, multiline literals, and grouped continuation lines. |
| `end-keyword` | `end-keyword`, `block-openers` | Count configured block openers until the corresponding closing keyword. |

End-keyword bodies also support:

- `conditional-openers`: openers that only start blocks at statement boundaries,
  so postfix conditions do not consume another closing keyword.
- `loop-openers` and `loop-body-keyword`: a loop header and its optional body
  keyword count as one block.
- `statement-boundaries`: tokens that can precede a new statement; line breaks
  also establish a boundary.
- `member-prefixes`: prevent member names or symbols from being treated as block
  keywords.

Lists use the existing comma-separated attribute convention. Opening and closing
delimiters must be nonempty and distinct. All positions and ranges refer to the
original text; a range ends immediately after its closing delimiter or keyword.

## Lexical rules

`word-characters` supplies extra identifier characters and whether Unicode
letters/digits are accepted. These settings govern both tokenization and name
capture. `lexical` may set `identifier-prefix` for an escaped identifier prefix.

Line comments, nested block comments, and strings retain their existing XML
elements. The longest matching string opener wins regardless of XML order.
The entire closing delimiter and escape marker are consumed, including markers
with multiple characters. `requires-closing-on-line` only recognizes a string
when its unescaped closer appears on that line. `single-quote-literals="true"`
restricts a configured string rule to character literals; the Rust rule uses
this to distinguish character literals from lifetimes.

Raw strings now describe their syntax explicitly rather than selecting a
hardcoded language by `kind`:

```xml
<raw-string prefixes="r,br,cr" repeat="#" open="&quot;" close="&quot;" />
<raw-string prefixes="R&quot;,u8R&quot;" open="(" close=")"
            suffix="&quot;" max-delimiter-length="16"
            forbidden-delimiter-characters=")\" />
```

The scanner reads a prefix, captures a delimiter, then consumes `open`. The
closer is `close` + the captured delimiter + `suffix`. With `repeat`, the
captured delimiter is zero or more repetitions of that marker. Without it, the
delimiter extends to `open` and excludes whitespace and any configured forbidden
characters. `max-delimiter-length` limits its UTF-8 byte length. Unterminated
recognized literals remain shielded through EOF.

## Declaration rules

Existing keyword, name-capture, method-container, terminator, and callable filters
remain supported. A language can set `signature-modifiers` to include preceding
modifiers in navigation ranges, including modifiers with grouped arguments.
A callable rule can set `assignment-arrow` to recognize assigned functions with
block bodies. The bundled JavaScript/TypeScript rule sets it to `=&gt;`.
Callable recognition retains its structural signature and expression scanning;
these rules are not a parser-generator interface for arbitrary grammars.

Overlapping rules that identify the same declaration are merged before nesting
is computed. Function-list depth counts enclosing declarations and containers;
control blocks and indentation widths do not introduce additional function nodes.
Containers are retained in the outline tree even when they have no functions.

## Member lists

Languages can add a member rule for a container kind:

```xml
<members kind="enum-member" within="enum" separator="," terminator=";"
         prefix-pattern="#\s*!?" />
```

The scanner reads identifiers at the top level of each matching brace body.
Separators inside comments, strings, or configured nested delimiters are ignored,
so tuple/struct payloads and initializer arguments do not become extra members.
The optional terminator stops the list before constructors and regular methods.
The optional anchored regular expression `prefix-pattern` skips annotation
markers/names and their following balanced argument group. Rust configures
attribute markers; Java and Kotlin configure annotation names. Empty separators,
empty terminators, and invalid or empty-matching prefix patterns are rejected.
Members keep their source ranges and can themselves own nested declarations.

## Validation and cache

Malformed or empty lexical delimiters produce registry diagnostics and exclude
the affected language plan. The compiled registry cache uses version 3; older
caches are rebuilt. Source XML remains schema version 1 with explicit rule
extensions. Older `raw-string kind` entries must be migrated to the attributes
above. Updated bundled definitions and cache round-trip tests cover these fields.

Run `cargo test --locked --test outline_parsing --test outline_performance` for
cross-language regressions, custom XML rules, and large-document checks. Existing
outline coverage also lives in `tests/editor_model.rs` and the outline unit tests.
