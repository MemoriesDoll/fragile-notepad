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
`identifier`, `qualified-identifier`, `template-type-tail`, `array-type-tail`, or
`pointer-type-tail`
before a method name while still admitting constructors. Array and generic
suffixes use the configured syntax roles; qualified names use
`qualified-separators`. Java uses these constraints to exclude enum constant
arguments from the function list.
Pointer and reference suffixes use repeated `type-suffix` syntax roles.

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
`type-prefix` identifies punctuation preceding signature type literals;
`type-suffix` identifies pointer/reference punctuation.
`attribute-prefix` identifies an attribute marker before a grouped attribute,
which can precede a declaration at a statement boundary.
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
identifiers are accepted. Unicode uses the XID start/continue character classes,
including combining marks in identifier continuations. These settings govern
both tokenization and name capture. `lexical` may set `identifier-prefix` for an
escaped identifier prefix. The prefix and identifier form one token, so a raw
identifier whose name matches a keyword cannot introduce a declaration.

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

Additional lexical elements shield literal or directive contents:

- `regex-literal` sets `open`, `close`, optional `escape`, paired
  `character-class-open`/`character-class-close`, and an optional `prefix-pattern`.
  The prefix pattern matches the previous significant token; a closing control
  condition is represented by its keyword followed by `()`. A completed block
  includes its preceding owner and `{}`, and member tokens retain their member
  prefix. The bundled
  JavaScript rule distinguishes expression starts from division operands.
- `heredoc` supplies a `prefix-pattern` with a named `delimiter` capture, or
  alternative captures prefixed `delimiter_`. `indented="true"` allows whitespace
  before the closing delimiter. An optional participating `indent` capture
  controls that allowance per opener, as in Ruby's `<<-` and `<<~` forms.
- Lexical `line-skip-pattern` elements have a `value` regex and consume complete
  directive lines at their first significant position. The C/C++ rule includes
  escaped newlines, shielding macro bodies without evaluating conditional branches.
- `opaque-block` supplies a `prefix-pattern` and balanced `open`/`close`
  delimiters. Its contents are shielded after comments and strings have been
  masked. Rust configures `macro_rules!` definition templates with brace,
  parenthesis, and bracket delimiters, so unused generated function templates do
  not become declarations. Macro invocation bodies remain available to existing
  item recognition.

## Declaration rules

Existing keyword, name-capture, method-container, terminator, and callable filters
remain supported. A language can set `signature-modifiers` to include preceding
modifiers in navigation ranges, including modifiers with grouped arguments.
A callable rule can set `assignment-arrow` to recognize assigned functions with
block bodies. The bundled JavaScript/TypeScript rule sets it to `=&gt;`.
Callable recognition retains its structural signature and expression scanning;
these rules are not a parser-generator interface for arbitrary grammars.

Keyword rules can specify `name-pattern`, an anchored regex with `name` or
`name_` captures. Initial generic parameters are skipped with the configured
generic delimiters before matching it. The bundled rules use this for Rust
implementation headers, Kotlin receivers/backtick names, and Ruby singleton,
setter, and operator names. Patterns can read a masked name's original spelling.
Callable rules can also use `name-pattern` to match a name immediately before its
parameter group, with ordinary callable names as fallback. The JavaScript rule
captures quoted names and simple computed names using this form.
`compact-constructor-containers` lists container keywords admitting constructor
bodies without parameter groups; Java configures `record`.
`keyword-reject-previous` and `keyword-reject-next` exclude adjacent tokens;
values beginning with `@` name a syntax role, for example `@generics-open`.
`require-statement-start="true"` restricts declaration context, as for Rust
implementation blocks rather than parameter or return types.

`signature-type-braces="true"` skips type literals in signatures.
`signature-brace-prefix-pattern` skips a brace group when the preceding
signature matches the configured regex, as for constructor initializers or
annotation array defaults. `nextline-body="true"` permits a body to start after
a newline when the rule otherwise uses a line terminator. `expression-body`
sets the marker for a declaration whose body ends on its declaration line;
Ruby configures `=` for endless methods, including inside classes/modules.

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
attribute markers; Java and Kotlin configure annotation names.
Members keep their source ranges and can themselves own nested declarations.

Additional anchored patterns configure member syntax without language tokens in
the scanner:

- `name-pattern` captures quoted/escaped names in a named group `name`. Alternative
  quote styles can use additional groups prefixed `name_`; the first participating
  group supplies the name. The whole match consumes the surrounding quotes, while
  the captured source spelling supplies the label and navigation start. Ordinary
  identifiers remain the fallback. Comments are skipped; literal starts remain
  available here so TypeScript string names and Kotlin backtick names are visible.
- `line-skip-pattern` consumes directive lines, including configured continuations.
  It only applies at the first non-whitespace/non-comment position on a line.
  C/C++ configure preprocessor directives; members in all conditional branches are
  listed without evaluating build conditions.
- `generic-open-pattern` consumes the expression prefix through an opening generic
  delimiter. `generic-suffix-pattern` validates what follows its balanced closing
  delimiter. These patterns must be supplied together with family `generics-open`
  and `generics-close` syntax tokens. Rust configures turbofish syntax; C++ configures
  template expressions. Context checks preserve comparisons and
  shifts instead of treating every angle token as a delimiter.

Empty separators, empty terminators, invalid or empty-matching patterns, name
patterns without a name capture, and incomplete generic configurations are rejected.
An unrecognized member recovers at the next separator instead of discarding the
rest of the list.

## Validation and cache

Malformed or empty lexical delimiters produce registry diagnostics and exclude
the affected language plan. The compiled registry cache is stored in
`outline-registry.xml`. A source XML hash mismatch or unreadable cache structure
triggers a rebuild. Bundled definitions and cache round-trip tests cover the
supported rule fields.

Run `cargo test --locked --test outline_parsing --test outline_configuration_regressions --test outline_performance` for
cross-language regressions, custom XML rules, and large-document checks. Existing
outline coverage also lives in `tests/editor_model.rs` and the outline unit tests.
