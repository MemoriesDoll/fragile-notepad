# Syntax configuration

`outline-parsers.xml` configures the function list and function navigation;
`folding-hints.xml` configures folding. Both are embedded; changes require rebuilding.
The shared structural recognizer uses compiled XML plans with original UTF-8 byte
offsets and exclusive range ends. It does not implement a complete language grammar.

The sidebar retains containers, including empty types and enum members. Filters
retain ancestors and show children of matching containers. Function navigation
uses callable entries. Overlapping rules merge before containment is computed.

## Families and bodies

A language selects `use-family`; adapter names are validated metadata.
Family `delimiter` elements index matching `open`/`close` pairs outside literals
and comments. Brace bodies add their configured pair.

`syntax-token` elements specify a `role` and one-character `value`:

| Roles | Purpose |
| --- | --- |
| `parameters-open`, `parameters-close`, `brackets-open`, `brackets-close` | Signature groups; also need matching `delimiter` elements |
| `generics-open`, `generics-close` | Generic groups |
| `assignment`, `separator`, `statement-end` | Statement segmentation |
| `assignment-reject-before`, `assignment-reject-after` | Repeated tokens excluding compound assignments |
| `type-prefix`, `type-suffix`, `attribute-prefix` | Type literals, pointer/reference suffixes, grouped attributes |

```xml
<syntax-token role="parameters-open" value="(" />
<syntax-token role="parameters-close" value=")" />
<delimiter open="(" close=")" />
```

| Body kind | Attributes |
| --- | --- |
| `brace` | Nonempty distinct `open`, `close`; multi-byte delimiters allowed |
| `indent` | `header-end`, `line-continuation`; bodies follow indentation across comments, literals, and grouped continuations |
| `end-keyword` | `end-keyword`, `block-openers`; count block openers until their closer |

End-keyword bodies additionally use `conditional-openers` for statement-only
conditions, `loop-openers`/`loop-block-keywords` to avoid counting a loop's `do`
twice, `statement-separators` for new statements, and `member-prefixes` to exclude
member names. Lists use comma-separated attributes.

## Lexical rules

`word-characters` configures extra identifier characters and Unicode XID
start/continue classes. `lexical identifier-prefix` makes escaped identifiers
atomic tokens, preventing keyword-like names from introducing declarations.

Line comments, nested block comments, and strings use their XML elements.
Longest string openers win; scanners consume full closing and escape markers.
`requires-closing-on-line` requires an unescaped closer on that line.
`single-quote-literals="true"` restricts a string rule to character literals.

Raw strings specify prefixes, captured delimiters, and closing syntax:

```xml
<raw-string prefixes="r,br,cr" repeat="#" open="&quot;" close="&quot;" />
<raw-string prefixes="R&quot;,u8R&quot;" open="(" close=")"
            suffix="&quot;" max-delimiter-length="16" />
```

The closer is `close` + captured delimiter + `suffix`. `repeat` restricts the
delimiter to repetitions of a marker; otherwise it extends to `open`, excluding
whitespace and `forbidden-delimiter-characters`. Length limits count UTF-8 bytes.
Recognized unterminated literals remain shielded through EOF.

| Lexical element | Configuration |
| --- | --- |
| `regex-literal` | `open`, `close`, optional `escape`, paired character-class delimiters, optional `prefix-pattern` matching the preceding significant token |
| `heredoc` | `prefix-pattern` with `delimiter` or `delimiter_*` capture; `indented` or a participating `indent` capture permits whitespace before the closer |
| `line-skip-pattern` | `value` regex consumes directive lines at their first significant position, including configured continuations |
| `opaque-block` | `prefix-pattern` and balanced `open`/`close` delimiters shield bodies after comment/string masking |

Regex context represents closed control conditions as their keyword plus `()`;
completed blocks retain their owner plus `{}`, and members retain their prefix.
Bundled rules distinguish JavaScript division, Ruby shifts/heredocs, C/C++
preprocessor lines, and Rust macro definitions. Conditional branches remain listed.

## Declarations

Keyword, name-capture, method-container, terminator, and callable filters configure
recognition. Language `signature-modifiers` includes preceding modifiers in ranges.

| Rule field | Behavior |
| --- | --- |
| `name-pattern` | Anchored regex with `name` or `name_*` captures; initial generics are skipped. Callable patterns precede parameters, with ordinary names as fallback |
| `assignment-arrow` | Assigned block-body functions, such as JavaScript `=>` |
| `compact-constructor-containers` | Containers allowing constructors without parameters, such as Java `record` |
| `keyword-reject-previous`, `keyword-reject-next` | Exclude adjacent tokens; `@role` references syntax roles |
| `require-statement-start` | Restrict declaration context |
| `require-non-container-previous-kind` | Callable prefix types: `identifier`, `qualified-identifier`, `template-type-tail`, `array-type-tail`, `pointer-type-tail`; constructors remain allowed |
| `qualified-separators` | Qualified-name punctuation |
| `signature-type-braces` | Skip type literals in signatures |
| `signature-brace-prefix-pattern` | Skip brace groups after matching signature prefixes |
| `nextline-body` | Permit bodies after newline terminators |
| `expression-body` | Marker for a body ending on the declaration line |

## Member lists and cache

Languages add `members kind="enum-member" within="enum"` to list variants/constants.
`separator` splits top-level entries; optional `terminator` stops before methods.
Nested payloads and initializer arguments stay grouped. Optional `prefix-pattern`
skips annotation markers/names and their balanced argument groups. `name-pattern`
captures quoted names with `name` or `name_*` groups; ordinary identifiers remain
the fallback. Members retain source ranges and may own nested declarations.
`line-skip-pattern` consumes directive lines, including continuations, at their
first significant position.

`generic-open-pattern` consumes a prefix through its generic opener;
`generic-suffix-pattern` validates the balanced group's suffix. Supply both
patterns and family `generics-open`/`generics-close` tokens. Context checks retain
comparisons and shifts. Unrecognized members recover at the next separator.

Empty delimiters, invalid or empty-matching patterns, missing name captures,
and incomplete generic configurations produce diagnostics. Invalid lexical
rules exclude the affected language plan. `outline-registry.xml` caches compiled
plans; source hash mismatches or unreadable structure trigger a rebuild.
