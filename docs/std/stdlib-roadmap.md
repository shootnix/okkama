# Okkama Standard Library — EO-first TODO

> Python = coverage. Go = package boundaries. Elegant Objects = API shape.
> Work top-to-bottom. `[ ]` -> `[x]`.

## Global rules

- [ ] No `Reader`, `Writer`, `Parser`, `Encoder`, `Decoder`, `Formatter`, `Manager`, `Controller`, `Handler`, `Helper`, `Builder`, `Factory`, `Validator`, `Router`, `Dispatcher`, `Listener`, `Observer`, `Client`.
- [ ] Name objects by what they **are**, not by what they **do**.
- [ ] Role names prefer natural `-ing` forms; if no gerund preserves the semantics, keep the noun and record ordered `-ing` candidates beside the role.
- [ ] Prefer declarative/decorator objects: `DataFile`, `Sorted<T>`, `Filtered<T>`, `Gzip`, `Base64`, `Json`, `PrettyJson`, `RemoteResponse`.
- [ ] No utility/service objects whose identity is "performs operation X".
- [ ] No static/free utility functions where an object can represent the result.
- [ ] Immutable objects.
- [ ] Constructors bind only; no I/O/computation in constructors.
- [ ] No `null`, generic optional, setters, `with_*`, implementation inheritance.
- [ ] Typed errors; fail fast.
- [ ] Small contracts and small public surfaces.
- [ ] Effects belong to real resources/entities (`Stdout`, `File`, `Process`, `TcpConnection`, `Logging`), not worker objects.
- [ ] Use decorators instead of flags/setters where practical.
- [ ] Tests use fakes, not mocks.

---

# Stage 00 — language substrate

## - [ ] builtin/Error

```okkama
# naming candidates, best -> worst: Failing, Faulting, Erring, Erroring
role Error {
    text() is string;
}
```

## - [ ] builtin conversions

```okkama
int.string() is string
int.float() is float
int.byte() is byte
float.string() is string
float.int() is int
byte.int() is int
byte.char() is char
char.string() is string
char.byte() is byte
bool.string() is string
```

## - [ ] builtin arrays/maps

```okkama
@T.size() is int
@T.empty() is bool
@T.at(index is int) is T

%K:V.size() is int
%K:V.empty() is bool
%K:V.contains(key is K) is bool
%K:V.at(key is K) is V
%K:V.keys() is @K
%K:V.values() is @V
```

---

# Stage 01 — tiny contracts

## - [x] core/compare

```okkama
enum Ordering {
    less;
    equal;
    greater;
}

role Comparing<T> {
    compare(other is T) is Ordering;
}
```

## - [x] core/size

```okkama
role Sizing {
    size() is int;
    empty() is bool;
}
```

## - [x] repr/byte

Central binary representation contract. Replaces generic Reader/Writer plumbing.

```okkama
role ByteRepresenting {
    bytes() is @byte;
}
```

## - [x] repr/text

```okkama
role TextRepresenting {
    text() is string;
}
```

---

# Stage 02 — iteration and sequences

## - [ ] collections/iteration

```okkama
role Iterating<T> {
    cursor() is Cursor<T>;
}

object Cursor<T> {
    has_next() is bool;
    next() is T;                  # throws IterationEnded
}
```

## - [x] collections/sequence

```okkama
role Sequencing<T> does Iterating<T>, Sizing {
    at(index is int) is T;
    first() is T;                 # throws EmptySequence
    last() is T;                  # throws EmptySequence
}
```

## - [ ] collections/range

```okkama
object Range does Sequencing<int> {
    Range(stop is int);
    Range(start is int, stop is int);
    Range(start is int, stop is int, step is int);

    at(index is int) is int;
    first() is int;
    last() is int;
    size() is int;
    empty() is bool;
    cursor() is Cursor<int>;
}
```

## - [ ] collections/enumerated

```okkama
object Indexed<T> {
    index() is int;
    item() is T;
}

object Enumerated<T> does Iterating<Indexed<T>> {
    Enumerated(origin is Iterating<T>);
    cursor() is Cursor<Indexed<T>>;
}
```

## - [ ] collections/zipped

```okkama
object Pair<A,B> {
    first() is A;
    second() is B;
}

object Zipped<A,B> does Iterating<Pair<A,B>> {
    Zipped(left is Iterating<A>, right is Iterating<B>);
    cursor() is Cursor<Pair<A,B>>;
}
```

## - [ ] collections/chained

```okkama
object Chained<T> does Iterating<T> {
    Chained(origins is @Iterating<T>);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/repeated

```okkama
object Repeated<T> does Iterating<T> {
    Repeated(value is T);
    cursor() is Cursor<T>;
}

object RepeatedTimes<T> does Sequencing<T> {
    RepeatedTimes(value is T, count is int);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/cycled

```okkama
object Cycled<T> does Iterating<T> {
    Cycled(origin is Sequencing<T>);
    cursor() is Cursor<T>;
}
```

---

# Stage 03 — collection decorators

## - [ ] collections/mapped

```okkama
object Mapped<A,B> does Iterating<B> {
    Mapped(origin is Iterating<A>, mapping is action(A) is B);
    cursor() is Cursor<B>;
}
```

## - [ ] collections/filtered

```okkama
object Filtered<T> does Iterating<T> {
    Filtered(origin is Iterating<T>, condition is action(T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/taken

```okkama
object Taken<T> does Iterating<T> {
    Taken(origin is Iterating<T>, count is int);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/dropped

```okkama
object Dropped<T> does Iterating<T> {
    Dropped(origin is Iterating<T>, count is int);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/taken-while

```okkama
object TakenWhile<T> does Iterating<T> {
    TakenWhile(origin is Iterating<T>, condition is action(T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/dropped-while

```okkama
object DroppedWhile<T> does Iterating<T> {
    DroppedWhile(origin is Iterating<T>, condition is action(T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/chunked

```okkama
object Chunked<T> does Iterating<@T> {
    Chunked(origin is Iterating<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/flattened

```okkama
object Flattened<T> does Iterating<T> {
    Flattened(origin is Iterating<Iterating<T>>);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/pairwise

```okkama
object Pairwise<T> does Iterating<Pair<T,T>> {
    Pairwise(origin is Iterating<T>);
    cursor() is Cursor<Pair<T,T>>;
}
```

## - [ ] collections/accumulated

```okkama
object Accumulated<T,R> does Iterating<R> {
    Accumulated(origin is Iterating<T>, initial is R, accumulation is action(R,T) is R);
    cursor() is Cursor<R>;
}
```

## - [ ] collections/reduced

```okkama
object Reduced<T,R> does Projecting<R> {
    Reduced(origin is Iterating<T>, initial is R, reduction is action(R,T) is R);
    value() is R;
}
```

---

# Stage 04 — concrete collections

## - [ ] collections/list

```okkama
# naming candidates, best -> worst: Linking<T>, Chaining<T>, Consing<T>, Prepending<T>, Listing<T>
role List<T> does Sequencing<T> {
    head() is T;
    tail() is List<T>;
    prepended(value is T) is List<T>;
}

object EmptyList<T> does List<T> { ... }
object LinkedList<T> does List<T> {
    LinkedList(value is T, following is List<T>);
    ...
}
```

## - [ ] collections/array

```okkama
object Array<T> does Sequencing<T> {
    Array(origin is @T);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/set

```okkama
# naming candidates, best -> worst: Containing<T>, Collecting<T>, Grouping<T>, MembershipTesting<T>, Setting<T>
role Set<T> does Iterating<T>, Sizing {
    contains(value is T) is bool;
    added(value is T) is Set<T>;
    removed(value is T) is Set<T>;
}

object HashSet<T> does Set<T> {
    HashSet(values is Iterating<T>);
    ...
}
```

## - [ ] collections/union

```okkama
object Union<T> does Set<T> {
    Union(left is Set<T>, right is Set<T>);
    ...
}
```

## - [ ] collections/intersection

```okkama
object Intersection<T> does Set<T> {
    Intersection(left is Set<T>, right is Set<T>);
    ...
}
```

## - [ ] collections/difference

```okkama
object Difference<T> does Set<T> {
    Difference(left is Set<T>, right is Set<T>);
    ...
}
```

## - [ ] collections/deque

```okkama
object Deque<T> does Sequencing<T> {
    Deque(values is Sequencing<T>);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

- [ ] No separate `Stack`; list/deque already cover the use case.

## - [ ] collections/heap

```okkama
object Heap<T> does Sequencing<T> {
    Heap(values is Iterating<T>, order is Comparing<T>);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/counter

```okkama
object Count<T> {
    item() is T;
    amount() is int;
}

object Counter<T> does Iterating<Count<T>>, Sizing {
    Counter(values is Iterating<T>);
    amount(value is T) is int;
    total() is int;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Count<T>>;
}
```

## - [ ] collections/sorted

```okkama
object Sorted<T> does Sequencing<T> {
    Sorted(origin is Iterating<T>, order is Comparing<T>);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/sorted-by

```okkama
object SortedBy<T,K> does Sequencing<T> {
    SortedBy(origin is Iterating<T>, key is action(T) is K, order is Comparing<K>);
    ...
}
```

## - [ ] collections/binary-search

```okkama
object BinarySearch<T> does Projecting<int> {
    BinarySearch(origin is Sequencing<T>, value is T, order is Comparing<T>);
    value() is int;                     # throws NotFound
}

object LowerBound<T> does Projecting<int> { ... }
object UpperBound<T> does Projecting<int> { ... }
```

## - [ ] collections/combinations

```okkama
object Combinations<T> does Iterating<@T> {
    Combinations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}

object RepeatedCombinations<T> does Iterating<@T> {
    RepeatedCombinations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/permutations

```okkama
object Permutations<T> does Iterating<@T> {
    Permutations(origin is Sequencing<T>);
    Permutations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/product

```okkama
object Product<T> does Iterating<@T> {
    Product(origins is @Sequencing<T>);
    cursor() is Cursor<@T>;
}
```

---

# Stage 05 — strings and text

## - [ ] string builtin behavior

```okkama
string.size() is int
string.empty() is bool
string.at(index is int) is char
string.slice(start is int, end is int) is string
string.contains(part is string) is bool
string.starts_with(prefix is string) is bool
string.ends_with(suffix is string) is bool
string.index(part is string) is int
string.last_index(part is string) is int
string.count(part is string) is int
string.lower() is string
string.upper() is string
string.title() is string
string.capitalize() is string
string.casefold() is string
string.trim() is string
string.trim(chars is string) is string
string.trim_left() is string
string.trim_right() is string
string.split(separator is string) is @string
string.lines() is @string
string.words() is @string
string.replace(old is string, new is string) is string
string.repeated(count is int) is string
string.reversed() is string
```

## - [ ] strings/joined

```okkama
object Joined does TextRepresenting {
    Joined(separator is string, values is Iterating<TextRepresenting>);
    text() is string;
}
```

## - [ ] strings/concatenated

```okkama
object Concatenated does TextRepresenting {
    Concatenated(parts is Iterating<TextRepresenting>);
    text() is string;
}
```

## - [ ] strings/indented

```okkama
object Indented does TextRepresenting {
    Indented(origin is TextRepresenting, prefix is string);
    text() is string;
}
```

## - [ ] strings/dedented

```okkama
object Dedented does TextRepresenting {
    Dedented(origin is TextRepresenting);
    text() is string;
}
```

## - [ ] strings/wrapped

```okkama
object Wrapped does TextRepresenting {
    Wrapped(origin is TextRepresenting, width is int);
    text() is string;
}
```

## - [ ] strings/shortened

```okkama
object Shortened does TextRepresenting {
    Shortened(origin is TextRepresenting, width is int, placeholder is string);
    text() is string;
}
```

## - [ ] strings/integer

```okkama
object IntegerText does Projecting<int> {
    IntegerText(origin is TextRepresenting);
    IntegerText(origin is TextRepresenting, base is int);
    value() is int;                  # throws InvalidInteger
}
```

## - [ ] strings/float

```okkama
object FloatText does Projecting<float> {
    FloatText(origin is TextRepresenting);
    value() is float;                # throws InvalidFloat
}
```

## - [ ] strings/boolean

```okkama
object BooleanText does Projecting<bool> {
    BooleanText(origin is TextRepresenting);
    value() is bool;                 # throws InvalidBoolean
}
```

## - [ ] strings/diff

```okkama
object UnifiedDiff does TextRepresenting {
    UnifiedDiff(left is Sequencing<string>, right is Sequencing<string>);
    text() is string;
}

object Similarity does Projecting<float> {
    Similarity(left is TextRepresenting, right is TextRepresenting);
    value() is float;
}
```

---

# Stage 06 — Unicode and byte representations

## - [ ] unicode/character

```okkama
object UnicodeCharacter {
    UnicodeCharacter(value is char);
    name() is string;
    category() is UnicodeCategory;
    combining() is int;
    numeric() is float;             # throws NotNumeric
}
```

## - [ ] unicode/normalized

```okkama
object Normalized does TextRepresenting {
    Normalized(origin is TextRepresenting, form is NormalizationForm);
    text() is string;
}
```

## - [ ] unicode/utf8

```okkama
object Utf8 does ByteRepresenting {
    Utf8(origin is TextRepresenting);
    bytes() is @byte;
}

object Utf8Text does TextRepresenting {
    Utf8Text(origin is ByteRepresenting);
    text() is string;               # throws InvalidUtf8
}
```

## - [ ] unicode/utf16

```okkama
object Utf16 does ByteRepresenting {
    Utf16(origin is TextRepresenting, order is ByteOrder);
    bytes() is @byte;
}

object Utf16Text does TextRepresenting {
    Utf16Text(origin is ByteRepresenting, order is ByteOrder);
    text() is string;
}
```

## - [ ] encoding/hex

```okkama
object Hex does TextRepresenting {
    Hex(origin is ByteRepresenting);
    text() is string;
}

object Unhexed does ByteRepresenting {
    Unhexed(origin is TextRepresenting);
    bytes() is @byte;
}
```

## - [ ] encoding/base32

```okkama
object Base32 does TextRepresenting { Base32(origin is ByteRepresenting); text() is string; }
object Unbase32 does ByteRepresenting { Unbase32(origin is TextRepresenting); bytes() is @byte; }
```

## - [ ] encoding/base64

```okkama
object Base64 does TextRepresenting { Base64(origin is ByteRepresenting); text() is string; }
object UrlBase64 does TextRepresenting { UrlBase64(origin is ByteRepresenting); text() is string; }
object Unbase64 does ByteRepresenting { Unbase64(origin is TextRepresenting); bytes() is @byte; }
object UnurlBase64 does ByteRepresenting { UnurlBase64(origin is TextRepresenting); bytes() is @byte; }
```

## - [ ] encoding/base85

```okkama
object Base85 does TextRepresenting { Base85(origin is ByteRepresenting); text() is string; }
object Unbase85 does ByteRepresenting { Unbase85(origin is TextRepresenting); bytes() is @byte; }
```

## - [ ] encoding/quotedprintable

```okkama
object QuotedPrintable does ByteRepresenting { QuotedPrintable(origin is ByteRepresenting); bytes() is @byte; }
object UnquotedPrintable does ByteRepresenting { UnquotedPrintable(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] encoding/pem

```okkama
object Pem does TextRepresenting {
    Pem(kind is string, origin is ByteRepresenting);
    text() is string;
}

object PemBlock does ByteRepresenting {
    PemBlock(origin is TextRepresenting);
    kind() is string;
    bytes() is @byte;
}
```

## - [ ] encoding/binary

```okkama
enum ByteOrder { big; little; native; }

object Uint16 does Projecting<int> { Uint16(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Uint32 does Projecting<int> { Uint32(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Uint64 does Projecting<int> { Uint64(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Int16 does Projecting<int> { Int16(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Int32 does Projecting<int> { Int32(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Int64 does Projecting<int> { Int64(origin is ByteRepresenting, order is ByteOrder); value() is int; }
object Float32 does Projecting<float> { Float32(origin is ByteRepresenting, order is ByteOrder); value() is float; }
object Float64 does Projecting<float> { Float64(origin is ByteRepresenting, order is ByteOrder); value() is float; }

object BinaryInt16 does ByteRepresenting { BinaryInt16(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryInt32 does ByteRepresenting { BinaryInt32(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryInt64 does ByteRepresenting { BinaryInt64(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryFloat32 does ByteRepresenting { BinaryFloat32(value is float, order is ByteOrder); bytes() is @byte; }
object BinaryFloat64 does ByteRepresenting { BinaryFloat64(value is float, order is ByteOrder); bytes() is @byte; }
```

---

# Stage 07 — actual I/O resources

## - [ ] io/stdout

```okkama
object Stdout {
    write(data is ByteRepresenting) is int;
    write(text is TextRepresenting) is int;
    writeln(text is TextRepresenting) is int;
    flush();
}
```

## - [ ] io/stderr

```okkama
object Stderr {
    write(data is ByteRepresenting) is int;
    write(text is TextRepresenting) is int;
    writeln(text is TextRepresenting) is int;
    flush();
}
```

## - [ ] io/stdin

```okkama
object Stdin {
    bytes(size is int) is @byte;
    line() is string;
    text() is string;
}
```

## - [ ] data/buffered

```okkama
object Buffered does ByteRepresenting {
    Buffered(origin is ByteRepresenting, size is int);
    bytes() is @byte;
}
```

## - [ ] data/limited

```okkama
object Limited does ByteRepresenting {
    Limited(origin is ByteRepresenting, limit is int);
    bytes() is @byte;
}
```

## - [ ] data/concatenated

```okkama
object ConcatenatedData does ByteRepresenting {
    ConcatenatedData(origins is Iterating<ByteRepresenting>);
    bytes() is @byte;
}
```

## - [ ] data/slice

```okkama
object DataSlice does ByteRepresenting {
    DataSlice(origin is ByteRepresenting, offset is int, size is int);
    bytes() is @byte;
}
```

---

# Stage 08 — math

## - [ ] math/abs

```okkama
object AbsInt does Projecting<int> { AbsInt(origin is int); value() is int; }
object AbsFloat does Projecting<float> { AbsFloat(origin is float); value() is float; }
```

## - [ ] math/min

```okkama
object MinInt does Projecting<int> { MinInt(first is int, rest is ...int); value() is int; }
object MinFloat does Projecting<float> { MinFloat(first is float, rest is ...float); value() is float; }
```

## - [ ] math/max

```okkama
object MaxInt does Projecting<int> { MaxInt(first is int, rest is ...int); value() is int; }
object MaxFloat does Projecting<float> { MaxFloat(first is float, rest is ...float); value() is float; }
```

## - [ ] math/rounding

```okkama
object Floor does Projecting<int> { Floor(origin is float); value() is int; }
object Ceiling does Projecting<int> { Ceiling(origin is float); value() is int; }
object Truncated does Projecting<int> { Truncated(origin is float); value() is int; }
object Rounded does Projecting<int> { Rounded(origin is float); value() is int; }
```

## - [ ] math/power

```okkama
object Power does Projecting<float> { Power(base is float, exponent is float); value() is float; }
object SquareRoot does Projecting<float> { SquareRoot(origin is float); value() is float; }
object CubeRoot does Projecting<float> { CubeRoot(origin is float); value() is float; }
```

## - [ ] math/logarithm

```okkama
object NaturalLog does Projecting<float> { NaturalLog(origin is float); value() is float; }
object Log2 does Projecting<float> { Log2(origin is float); value() is float; }
object Log10 does Projecting<float> { Log10(origin is float); value() is float; }
```

## - [ ] math/trigonometry

```okkama
object Sine does Projecting<float> { Sine(origin is float); value() is float; }
object Cosine does Projecting<float> { Cosine(origin is float); value() is float; }
object Tangent does Projecting<float> { Tangent(origin is float); value() is float; }
object ArcSine does Projecting<float> { ArcSine(origin is float); value() is float; }
object ArcCosine does Projecting<float> { ArcCosine(origin is float); value() is float; }
object ArcTangent does Projecting<float> { ArcTangent(origin is float); value() is float; }
object ArcTangent2 does Projecting<float> { ArcTangent2(y is float, x is float); value() is float; }
```

## - [ ] math/integer

```okkama
object Gcd does Projecting<int> { Gcd(values is ...int); value() is int; }
object Lcm does Projecting<int> { Lcm(values is ...int); value() is int; }
object Factorial does Projecting<int> { Factorial(origin is int); value() is int; }
object IntegerSquareRoot does Projecting<int> { IntegerSquareRoot(origin is int); value() is int; }
object FloorDivision does Projecting<int> { FloorDivision(left is int, right is int); value() is int; }
object FloorModulo does Projecting<int> { FloorModulo(left is int, right is int); value() is int; }
```

## - [ ] math/rational

```okkama
object Rational does Comparing<Rational> {
    Rational(numerator is int, denominator is int);
    plus(other is Rational) is Rational;
    minus(other is Rational) is Rational;
    multiplied(other is Rational) is Rational;
    divided(other is Rational) is Rational;
    compare(other is Rational) is Ordering;
}
```

## - [ ] math/decimal

```okkama
object Decimal does Comparing<Decimal>, TextRepresenting {
    Decimal(origin is TextRepresenting);
    plus(other is Decimal) is Decimal;
    minus(other is Decimal) is Decimal;
    multiplied(other is Decimal) is Decimal;
    divided(other is Decimal) is Decimal;
    compare(other is Decimal) is Ordering;
    text() is string;
}
```

- [ ] Add specialized decimal decorators (`Quantized`, `RoundedDecimal`, `SquareRootDecimal`) instead of bloating `Decimal`.

## - [ ] math/complex

```okkama
object Complex {
    Complex(real is float, imaginary is float);
    plus(other is Complex) is Complex;
    minus(other is Complex) is Complex;
    multiplied(other is Complex) is Complex;
    divided(other is Complex) is Complex;
}

object Magnitude does Projecting<float> { Magnitude(origin is Complex); value() is float; }
object Phase does Projecting<float> { Phase(origin is Complex); value() is float; }
object Conjugate does Projecting<Complex> { Conjugate(origin is Complex); value() is Complex; }
```

## - [ ] math/statistics

```okkama
object Mean does Projecting<float> { Mean(values is Iterating<float>); value() is float; }
object GeometricMean does Projecting<float> { GeometricMean(values is Iterating<float>); value() is float; }
object HarmonicMean does Projecting<float> { HarmonicMean(values is Iterating<float>); value() is float; }
object Median does Projecting<float> { Median(values is Iterating<float>); value() is float; }
object Mode<T> does Projecting<T> { Mode(values is Iterating<T>); value() is T; }
object Variance does Projecting<float> { Variance(values is Iterating<float>); value() is float; }
object PopulationVariance does Projecting<float> { PopulationVariance(values is Iterating<float>); value() is float; }
object Deviation does Projecting<float> { Deviation(values is Iterating<float>); value() is float; }
object Covariance does Projecting<float> { Covariance(left is Iterating<float>, right is Iterating<float>); value() is float; }
object Correlation does Projecting<float> { Correlation(left is Iterating<float>, right is Iterating<float>); value() is float; }
```

---

# Stage 09 — randomness / UUID

## - [ ] random

```okkama
object Random {
    Random();
    Random(seed is int);

    int(minimum is int, maximum is int) is int;
    float() is float;
    bytes(size is int) is @byte;
    choice(values is Sequencing<T>) is T;
}
```

## - [ ] random/distributions

```okkama
object NormalRandom does Projecting<float> {
    NormalRandom(origin is Random, mean is float, deviation is float);
    value() is float;
}

object ExponentialRandom does Projecting<float> {
    ExponentialRandom(origin is Random, rate is float);
    value() is float;
}

object Sample<T> does Sequencing<T> {
    Sample(origin is Random, values is Sequencing<T>, count is int);
    ...
}
```

## - [ ] uuid

```okkama
object UUID does Comparing<UUID>, TextRepresenting, ByteRepresenting {
    UUID(origin is TextRepresenting);
    compare(other is UUID) is Ordering;
    text() is string;
    bytes() is @byte;
}

object UUIDv4 does TextRepresenting, ByteRepresenting {
    UUIDv4(random is Random);
    text() is string;
    bytes() is @byte;
}

object UUIDv7 does TextRepresenting, ByteRepresenting {
    UUIDv7(clock is Timing, random is Random);
    text() is string;
    bytes() is @byte;
}
```

---

# Stage 10 — time

## - [ ] time/duration

```okkama
object Duration does Comparing<Duration>, TextRepresenting {
    nanoseconds() is int;
    plus(other is Duration) is Duration;
    minus(other is Duration) is Duration;
    compare(other is Duration) is Ordering;
    text() is string;
}

object Nanoseconds does Projecting<Duration> { Nanoseconds(value is int); value() is Duration; }
object Microseconds does Projecting<Duration> { Microseconds(value is int); value() is Duration; }
object Milliseconds does Projecting<Duration> { Milliseconds(value is int); value() is Duration; }
object Seconds does Projecting<Duration> { Seconds(value is float); value() is Duration; }
object Minutes does Projecting<Duration> { Minutes(value is float); value() is Duration; }
object Hours does Projecting<Duration> { Hours(value is float); value() is Duration; }
object Days does Projecting<Duration> { Days(value is float); value() is Duration; }
```

## - [ ] time/instant

```okkama
object Instant does Comparing<Instant> {
    plus(duration is Duration) is Instant;
    minus(other is Instant) is Duration;
    compare(other is Instant) is Ordering;
}
```

## - [ ] time/timing

```okkama
role Timing {
    now() is DateTime;
    monotonic() is Instant;
}

object SystemClock does Timing {
    now() is DateTime;
    monotonic() is Instant;
}
```

## - [ ] time/date

```okkama
object Date does Comparing<Date>, TextRepresenting {
    Date(year is int, month is int, day is int);
    plus(duration is Duration) is Date;
    compare(other is Date) is Ordering;
    text() is string;
}

object Year does Projecting<int> { Year(origin is Date); value() is int; }
object Month does Projecting<int> { Month(origin is Date); value() is int; }
object Day does Projecting<int> { Day(origin is Date); value() is int; }
object WeekdayOf does Projecting<Weekday> { WeekdayOf(origin is Date); value() is Weekday; }
```

## - [ ] time/datetime

```okkama
object DateTime does Comparing<DateTime>, TextRepresenting {
    plus(duration is Duration) is DateTime;
    minus(other is DateTime) is Duration;
    compare(other is DateTime) is Ordering;
    text() is string;
}
```

## - [ ] time/zone

```okkama
object Zone does TextRepresenting {
    Zone(name is string);
    text() is string;
}

object ZonedDateTime does Projecting<DateTime> {
    ZonedDateTime(origin is DateTime, zone is Zone);
    value() is DateTime;
}
```

## - [ ] time/iso

```okkama
object IsoDate does TextRepresenting { IsoDate(origin is Date); text() is string; }
object IsoDateTime does TextRepresenting { IsoDateTime(origin is DateTime); text() is string; }

object DateFromIso does Projecting<Date> {
    DateFromIso(origin is TextRepresenting);
    value() is Date;
}

object DateTimeFromIso does Projecting<DateTime> {
    DateTimeFromIso(origin is TextRepresenting);
    value() is DateTime;
}
```

## - [ ] time/rfc3339

```okkama
object Rfc3339 does TextRepresenting { Rfc3339(origin is DateTime); text() is string; }

object DateTimeFromRfc3339 does Projecting<DateTime> {
    DateTimeFromRfc3339(origin is TextRepresenting);
    value() is DateTime;
}
```

---

# Stage 11 — filesystem

## - [ ] fs/path

```okkama
object Path does Comparing<Path>, TextRepresenting {
    Path(origin is TextRepresenting);

    parent() is Path;
    joined(part is TextRepresenting) is Path;
    compare(other is Path) is Ordering;
    text() is string;
}

object PathName does TextRepresenting { PathName(origin is Path); text() is string; }
object PathStem does TextRepresenting { PathStem(origin is Path); text() is string; }
object PathSuffix does TextRepresenting { PathSuffix(origin is Path); text() is string; }

object AbsolutePath does Projecting<Path> { AbsolutePath(origin is Path); value() is Path; }
object NormalizedPath does Projecting<Path> { NormalizedPath(origin is Path); value() is Path; }
object ResolvedPath does Projecting<Path> { ResolvedPath(origin is Path); value() is Path; }
object RelativePath does Projecting<Path> { RelativePath(origin is Path, base is Path); value() is Path; }
```

## - [ ] fs/file

```okkama
object File {
    File(path is Path);

    exists() is bool;
    stat() is FileStat;
    remove();
    move(destination is Path) is File;
}
```

## - [ ] fs/data-file

```okkama
object DataFile does ByteRepresenting {
    DataFile(file is File);
    bytes() is @byte;
}
```

## - [ ] fs/text-file

```okkama
object TextFile does TextRepresenting {
    TextFile(file is File, encoding is Encoding);
    text() is string;
}
```

## - [ ] fs/written-file

```okkama
object WrittenFile does Projecting<File> {
    WrittenFile(file is File, data is ByteRepresenting);
    value() is File;
}

object AppendedFile does Projecting<File> {
    AppendedFile(file is File, data is ByteRepresenting);
    value() is File;
}
```

## - [ ] fs/directory

```okkama
object Directory does Iterating<Path> {
    Directory(path is Path);

    exists() is bool;
    cursor() is Cursor<Path>;
    remove();
    move(destination is Path) is Directory;
}
```

## - [ ] fs/files

```okkama
object Files does Iterating<File> {
    Files(directory is Directory);
    cursor() is Cursor<File>;
}

object Directories does Iterating<Directory> {
    Directories(directory is Directory);
    cursor() is Cursor<Directory>;
}
```

## - [ ] fs/globbed

```okkama
object Globbed does Iterating<Path> {
    Globbed(root is Directory, pattern is string);
    cursor() is Cursor<Path>;
}
```

## - [ ] fs/tree

```okkama
object Tree does Iterating<Path> {
    Tree(root is Directory);
    cursor() is Cursor<Path>;
}
```

## - [ ] fs/temp

```okkama
object TemporaryFile does Projecting<File> {
    TemporaryFile(directory is Directory, prefix is string);
    value() is File;
}

object TemporaryDirectory does Projecting<Directory> {
    TemporaryDirectory(parent is Directory, prefix is string);
    value() is Directory;
}
```

## - [ ] fs/stat

```okkama
object FileStat {
    size() is int;
    mode() is FileMode;
    modified() is DateTime;
    kind() is FileKind;
}
```

## - [ ] fs/compare

```okkama
object SameFile does Projecting<bool> {
    SameFile(left is File, right is File);
    value() is bool;
}

object SameContent does Projecting<bool> {
    SameContent(left is ByteRepresenting, right is ByteRepresenting);
    value() is bool;
}
```

## - [ ] fs/mmap

```okkama
object MappedFile does ByteRepresenting {
    MappedFile(file is File);
    bytes() is @byte;
}
```

## - [ ] fs/watch

```okkama
object FileWatch does Iterating<FileEvent> {
    FileWatch(path is Path);
    cursor() is Cursor<FileEvent>;
}

object FileEvent {
    path() is Path;
    kind() is FileEventKind;
}
```

---

# Stage 12 — OS / process

## - [ ] os/environment

```okkama
object Environment {
    contains(name is string) is bool;
    value(name is string) is string;
    variables() is %string:string;
}

object HomeDirectory does Projecting<Directory> {
    HomeDirectory(environment is Environment);
    value() is Directory;
}

object SearchPath does Sequencing<Path> {
    SearchPath(environment is Environment);
    ...
}
```

## - [ ] os/system

```okkama
object System {
    operating_system() is OperatingSystem;
    architecture() is string;
    hostname() is string;
    cpu_count() is int;
}
```

## - [ ] os/user

```okkama
object User {
    name() is string;
    home() is Directory;
    id() is int;
}

object CurrentUser {
    name() is string;
    home() is Directory;
    id() is int;
}
```

## - [ ] os/group

```okkama
object Group {
    name() is string;
    id() is int;
    users() is Sequencing<User>;
}
```

## - [ ] os/process

```okkama
object Command does TextRepresenting {
    Command(program is Path, arguments is Sequencing<string>);
    text() is string;
}

object Process {
    Process(command is Command);

    id() is int;
    running() is bool;
    wait() is Exit;
    terminate();
}
```

## - [ ] os/output-process

```okkama
object OutputProcess does ByteRepresenting {
    OutputProcess(command is Command);
    bytes() is @byte;
}

object TextProcess does TextRepresenting {
    TextProcess(command is Command, encoding is Encoding);
    text() is string;
}
```

## - [ ] os/pipeline

```okkama
object Pipeline does ByteRepresenting {
    Pipeline(commands is Sequencing<Command>);
    bytes() is @byte;
}
```

## - [ ] os/signal

```okkama
enum Signal {
    interrupt;
    terminate;
    kill;
    hangup;
    quit;
}
```

## - [ ] os/executable

```okkama
object Executable does Projecting<Path> {
    Executable(name is string, environment is Environment);
    value() is Path;
}
```

## - [ ] os/disk

```okkama
object Disk {
    Disk(path is Path);
    total() is int;
    free() is int;
    used() is int;
}
```

---

# Stage 13 — structured serialization

## - [ ] encoding/json

```okkama
role JsonRepresenting {
    json() is string;
}

object JsonNull does JsonRepresenting {
    json() is string;
}

object JsonBoolean does JsonRepresenting {
    JsonBoolean(value is bool);
    json() is string;
}

object JsonNumber does JsonRepresenting {
    JsonNumber(value is float);
    json() is string;
}

object JsonString does JsonRepresenting {
    JsonString(value is string);
    json() is string;
}

object JsonArray does JsonRepresenting, Sequencing<JsonRepresenting> {
    JsonArray(values is Sequencing<JsonRepresenting>);
    json() is string;
    at(index is int) is JsonRepresenting;
    first() is JsonRepresenting;
    last() is JsonRepresenting;
    size() is int;
    empty() is bool;
    cursor() is Cursor<JsonRepresenting>;
}

object JsonObject does JsonRepresenting {
    JsonObject(values is %string:JsonRepresenting);
    json() is string;
    contains(name is string) is bool;
    value(name is string) is JsonRepresenting;
}

object Json does Projecting<JsonRepresenting> {
    Json(origin is TextRepresenting);
    value() is JsonRepresenting;
}

object JsonData does ByteRepresenting {
    JsonData(origin is JsonRepresenting);
    bytes() is @byte;
}

object PrettyJson does TextRepresenting {
    PrettyJson(origin is JsonRepresenting, indent is int);
    text() is string;
}
```

## - [ ] encoding/csv

```okkama
object Csv does Iterating<@string> {
    Csv(origin is TextRepresenting, delimiter is char);
    cursor() is Cursor<@string>;
}

object CsvText does TextRepresenting {
    CsvText(rows is Iterating<@string>, delimiter is char);
    text() is string;
}
```

## - [ ] encoding/toml

```okkama
object Toml does Projecting<TomlDocument> {
    Toml(origin is TextRepresenting);
    value() is TomlDocument;
}

object TomlText does TextRepresenting {
    TomlText(origin is TomlDocument);
    text() is string;
}
```

## - [ ] encoding/ini

```okkama
object Ini does Projecting<IniDocument> {
    Ini(origin is TextRepresenting);
    value() is IniDocument;
}

object IniText does TextRepresenting {
    IniText(origin is IniDocument);
    text() is string;
}
```

## - [ ] encoding/xml

```okkama
object Xml does Projecting<XmlDocument> {
    Xml(origin is TextRepresenting);
    value() is XmlDocument;
}

object XmlText does TextRepresenting {
    XmlText(origin is XmlDocument);
    text() is string;
}
```

## - [ ] encoding/html

```okkama
object Html does Projecting<HtmlDocument> {
    Html(origin is TextRepresenting);
    value() is HtmlDocument;
}

object EscapedHtml does TextRepresenting {
    EscapedHtml(origin is TextRepresenting);
    text() is string;
}
```

## - [ ] encoding/plist

```okkama
object Plist does Projecting<PlistDocument> {
    Plist(origin is ByteRepresenting);
    value() is PlistDocument;
}

object PlistData does ByteRepresenting {
    PlistData(origin is PlistDocument, format is PlistFormat);
    bytes() is @byte;
}
```

---

# Stage 14 — regex / lexical text

## - [ ] regex/pattern

```okkama
object Pattern {
    Pattern(expression is string);

    matches(text is TextRepresenting) is bool;
    first(text is TextRepresenting) is Match;
    all(text is TextRepresenting) is Sequencing<Match>;
}
```

## - [ ] regex/match

```okkama
object Match {
    text() is string;
    span() is Span;
    group(name is string) is string;
}
```

## - [ ] regex/replaced

```okkama
object Replaced does TextRepresenting {
    Replaced(origin is TextRepresenting, pattern is Pattern, replacement is TextRepresenting);
    text() is string;
}
```

## - [ ] regex/split

```okkama
object SplitByPattern does Sequencing<string> {
    SplitByPattern(origin is TextRepresenting, pattern is Pattern);
    ...
}
```

## - [ ] text/tokens

```okkama
object TextTokens does Sequencing<TextToken> {
    TextTokens(origin is TextRepresenting);
    ...
}
```

---

# Stage 15 — checksums / digests

## - [ ] hash/adler32

```okkama
object Adler32 does Projecting<int> {
    Adler32(origin is ByteRepresenting);
    value() is int;
}
```

## - [ ] hash/crc32

```okkama
object Crc32 does Projecting<int> {
    Crc32(origin is ByteRepresenting);
    value() is int;
}
```

## - [ ] hash/crc64

```okkama
object Crc64 does Projecting<int> {
    Crc64(origin is ByteRepresenting);
    value() is int;
}
```

## - [ ] hash/fnv

```okkama
object Fnv1a32 does Projecting<int> { Fnv1a32(origin is ByteRepresenting); value() is int; }
object Fnv1a64 does Projecting<int> { Fnv1a64(origin is ByteRepresenting); value() is int; }
```

## - [ ] crypto/md5

```okkama
object Md5 does ByteRepresenting {
    Md5(origin is ByteRepresenting);
    bytes() is @byte;
}
```

## - [ ] crypto/sha1

```okkama
object Sha1 does ByteRepresenting {
    Sha1(origin is ByteRepresenting);
    bytes() is @byte;
}
```

## - [ ] crypto/sha2

```okkama
object Sha224 does ByteRepresenting { Sha224(origin is ByteRepresenting); bytes() is @byte; }
object Sha256 does ByteRepresenting { Sha256(origin is ByteRepresenting); bytes() is @byte; }
object Sha384 does ByteRepresenting { Sha384(origin is ByteRepresenting); bytes() is @byte; }
object Sha512 does ByteRepresenting { Sha512(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] crypto/sha3

```okkama
object Sha3_224 does ByteRepresenting { Sha3_224(origin is ByteRepresenting); bytes() is @byte; }
object Sha3_256 does ByteRepresenting { Sha3_256(origin is ByteRepresenting); bytes() is @byte; }
object Sha3_384 does ByteRepresenting { Sha3_384(origin is ByteRepresenting); bytes() is @byte; }
object Sha3_512 does ByteRepresenting { Sha3_512(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] crypto/hmac

```okkama
object Hmac does ByteRepresenting {
    Hmac(origin is ByteRepresenting, key is ByteRepresenting, algorithm is DigestAlgorithm);
    bytes() is @byte;
}

object ValidHmac does Projecting<bool> {
    ValidHmac(origin is ByteRepresenting, key is ByteRepresenting, signature is ByteRepresenting, algorithm is DigestAlgorithm);
    value() is bool;
}
```

---

# Stage 16 — compression

## - [ ] compression/gzip

```okkama
object Gzip does ByteRepresenting {
    Gzip(origin is ByteRepresenting);
    Gzip(origin is ByteRepresenting, level is int);
    bytes() is @byte;
}

object Gunzipped does ByteRepresenting {
    Gunzipped(origin is ByteRepresenting);
    bytes() is @byte;
}
```

## - [ ] compression/zlib

```okkama
object Zlib does ByteRepresenting { Zlib(origin is ByteRepresenting); bytes() is @byte; }
object Unzlib does ByteRepresenting { Unzlib(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] compression/deflate

```okkama
object Deflated does ByteRepresenting { Deflated(origin is ByteRepresenting); bytes() is @byte; }
object Inflated does ByteRepresenting { Inflated(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] compression/bzip2

```okkama
object Bzip2 does ByteRepresenting { Bzip2(origin is ByteRepresenting); bytes() is @byte; }
object Unbzip2 does ByteRepresenting { Unbzip2(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] compression/xz

```okkama
object Xz does ByteRepresenting { Xz(origin is ByteRepresenting); bytes() is @byte; }
object Unxz does ByteRepresenting { Unxz(origin is ByteRepresenting); bytes() is @byte; }
```

## - [ ] compression/zstd

```okkama
object Zstd does ByteRepresenting { Zstd(origin is ByteRepresenting); bytes() is @byte; }
object Unzstd does ByteRepresenting { Unzstd(origin is ByteRepresenting); bytes() is @byte; }
```

---

# Stage 17 — archives

## - [ ] archive/entry

```okkama
# naming candidates, best -> worst: ArchivingEntry, CarryingArchiveData, ContainingArchiveData, ProvidingArchiveData, Archiving
role ArchiveEntry {
    name() is string;
    data() is ByteRepresenting;
}
```

## - [ ] archive/tar

```okkama
object Tar does ByteRepresenting {
    Tar(entries is Iterating<ArchiveEntry>);
    bytes() is @byte;
}

object Untarred does Iterating<ArchiveEntry> {
    Untarred(origin is ByteRepresenting);
    cursor() is Cursor<ArchiveEntry>;
}
```

## - [ ] archive/zip

```okkama
object Zip does ByteRepresenting {
    Zip(entries is Iterating<ArchiveEntry>);
    bytes() is @byte;
}

object Unzipped does Iterating<ArchiveEntry> {
    Unzipped(origin is ByteRepresenting);
    cursor() is Cursor<ArchiveEntry>;
}
```

## - [ ] archive/file-entry

```okkama
object FileEntry does ArchiveEntry {
    FileEntry(name is string, file is File);
    name() is string;
    data() is ByteRepresenting;
}

object DataEntry does ArchiveEntry {
    DataEntry(name is string, data is ByteRepresenting);
    name() is string;
    data() is ByteRepresenting;
}
```

---

# Stage 18 — cryptography

## - [ ] crypto/random

```okkama
object SecureRandom {
    bytes(size is int) is @byte;
    int(maximum is int) is int;
}
```

## - [ ] crypto/kdf

```okkama
object Hkdf does ByteRepresenting {
    Hkdf(secret is ByteRepresenting, salt is ByteRepresenting, info is ByteRepresenting, size is int, algorithm is DigestAlgorithm);
    bytes() is @byte;
}

object Pbkdf2 does ByteRepresenting {
    Pbkdf2(password is ByteRepresenting, salt is ByteRepresenting, iterations is int, size is int, algorithm is DigestAlgorithm);
    bytes() is @byte;
}
```

## - [ ] crypto/aead

```okkama
object AesGcmCiphertext does ByteRepresenting {
    AesGcmCiphertext(key is ByteRepresenting, nonce is ByteRepresenting, plaintext is ByteRepresenting, associated is ByteRepresenting);
    bytes() is @byte;
}

object AesGcmPlaintext does ByteRepresenting {
    AesGcmPlaintext(key is ByteRepresenting, nonce is ByteRepresenting, ciphertext is ByteRepresenting, associated is ByteRepresenting);
    bytes() is @byte;
}

object ChaChaCiphertext does ByteRepresenting {
    ChaChaCiphertext(key is ByteRepresenting, nonce is ByteRepresenting, plaintext is ByteRepresenting, associated is ByteRepresenting);
    bytes() is @byte;
}

object ChaChaPlaintext does ByteRepresenting {
    ChaChaPlaintext(key is ByteRepresenting, nonce is ByteRepresenting, ciphertext is ByteRepresenting, associated is ByteRepresenting);
    bytes() is @byte;
}
```

## - [ ] crypto/ed25519

```okkama
object Ed25519Keys {
    public() is Ed25519PublicKey;
    private() is Ed25519PrivateKey;
}

object Ed25519Signature does ByteRepresenting {
    Ed25519Signature(key is Ed25519PrivateKey, message is ByteRepresenting);
    bytes() is @byte;
}

object ValidEd25519 does Projecting<bool> {
    ValidEd25519(key is Ed25519PublicKey, message is ByteRepresenting, signature is ByteRepresenting);
    value() is bool;
}
```

## - [ ] crypto/ecdh

```okkama
object SharedSecret does ByteRepresenting {
    SharedSecret(private is EcdhPrivateKey, public is EcdhPublicKey);
    bytes() is @byte;
}
```

## - [ ] crypto/ecdsa

```okkama
object EcdsaSignature does ByteRepresenting {
    EcdsaSignature(key is EcdsaPrivateKey, digest is ByteRepresenting);
    bytes() is @byte;
}

object ValidEcdsa does Projecting<bool> {
    ValidEcdsa(key is EcdsaPublicKey, digest is ByteRepresenting, signature is ByteRepresenting);
    value() is bool;
}
```

## - [ ] crypto/rsa

```okkama
object RsaCiphertext does ByteRepresenting {
    RsaCiphertext(key is RsaPublicKey, plaintext is ByteRepresenting);
    bytes() is @byte;
}

object RsaPlaintext does ByteRepresenting {
    RsaPlaintext(key is RsaPrivateKey, ciphertext is ByteRepresenting);
    bytes() is @byte;
}

object RsaSignature does ByteRepresenting {
    RsaSignature(key is RsaPrivateKey, digest is ByteRepresenting);
    bytes() is @byte;
}
```

## - [ ] encoding/asn1

```okkama
object Asn1 does Projecting<Asn1Value> {
    Asn1(origin is ByteRepresenting);
    value() is Asn1Value;
}

object Asn1Data does ByteRepresenting {
    Asn1Data(origin is Asn1Value);
    bytes() is @byte;
}
```

## - [ ] crypto/x509

```okkama
object Certificate does ByteRepresenting, TextRepresenting {
    Certificate(origin is ByteRepresenting);
    bytes() is @byte;
    text() is string;
    valid(at is DateTime) is bool;
    matches(host is string) is bool;
}
```

## - [ ] crypto/tls

```okkama
object TlsConnection {
    TlsConnection(origin is TcpConnection, config is TlsConfig);

    receive(size is int) is @byte;
    send(data is ByteRepresenting) is int;
    peer() is Address;
    close();
}
```

---

# Stage 19 — concurrency

## - [ ] async/task

```okkama
object Task<T> {
    done() is bool;
    cancelled() is bool;
    result() is T;
    cancel() is bool;
}
```

## - [ ] async/all

```okkama
object All<T> does Projecting<@T> {
    All(tasks is @Task<T>);
    value() is @T async;
}
```

## - [ ] async/any

```okkama
object Any<T> does Projecting<T> {
    Any(tasks is @Task<T>);
    value() is T async;
}
```

## - [ ] async/timed

```okkama
object Timed<T> does Projecting<T> {
    Timed(task is Task<T>, limit is Duration);
    value() is T async;
}
```

## - [ ] async/completed

```okkama
object Completed<T> does Iterating<Task<T>> {
    Completed(tasks is @Task<T>);
    cursor() is Cursor<Task<T>>;
}
```

## - [ ] async/channel

```okkama
object Channel<T> {
    send(value is T) async;
    receive() is T async;
    closed() is bool;
    close();
}
```

## - [ ] sync policy

- [ ] Keep mutex/RW-lock/condition/atomic primitives runtime-internal initially.
- [ ] Do not copy Go's synchronization package until Okkama has a concrete immutable-object use case.
- [ ] Re-evaluate after process/async semantics are stable.

---

# Stage 20 — networking primitives

## - [ ] net/ip

```okkama
object Ip does Comparing<Ip>, TextRepresenting, ByteRepresenting {
    Ip(origin is TextRepresenting);

    compare(other is Ip) is Ordering;
    text() is string;
    bytes() is @byte;
    private() is bool;
    loopback() is bool;
}
```

## - [ ] net/network

```okkama
object Network does TextRepresenting {
    Network(origin is TextRepresenting);
    contains(address is Ip) is bool;
    text() is string;
}
```

## - [ ] net/address

```okkama
object Address does TextRepresenting {
    Address(host is string, port is int);
    text() is string;
}
```

## - [ ] net/dns

No `Resolver`.

```okkama
object HostAddresses does Sequencing<Ip> {
    HostAddresses(host is string);
    at(index is int) is Ip;
    first() is Ip;
    last() is Ip;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Ip>;
}

object HostName does TextRepresenting {
    HostName(address is Ip);
    text() is string;
}

object MxRecords does Iterating<MxRecord> {
    MxRecords(host is string);
    cursor() is Cursor<MxRecord>;
}

object TxtRecords does Iterating<string> {
    TxtRecords(host is string);
    cursor() is Cursor<string>;
}

object SrvRecords does Iterating<SrvRecord> {
    SrvRecords(service is string, protocol is string, host is string);
    cursor() is Cursor<SrvRecord>;
}
```

## - [ ] net/tcp

```okkama
object TcpConnection {
    TcpConnection(address is Address);

    receive(size is int) is @byte;
    send(data is ByteRepresenting) is int;
    peer() is Address;
    close();
}
```

## - [ ] net/tcp-endpoint

No server/listener object.

```okkama
object TcpEndpoint does Iterating<TcpConnection> {
    TcpEndpoint(address is Address);
    cursor() is Cursor<TcpConnection>;
}
```

## - [ ] net/udp

```okkama
object Datagram {
    data() is ByteRepresenting;
    source() is Address;
}

object UdpEndpoint {
    UdpEndpoint(address is Address);

    receive(size is int) is Datagram;
    send(data is ByteRepresenting, destination is Address) is int;
    close();
}
```

## - [ ] net/unix

```okkama
object UnixConnection {
    UnixConnection(path is Path);

    receive(size is int) is @byte;
    send(data is ByteRepresenting) is int;
    close();
}

object UnixEndpoint does Iterating<UnixConnection> {
    UnixEndpoint(path is Path);
    cursor() is Cursor<UnixConnection>;
}
```

---

# Stage 21 — URL / MIME / mail

## - [ ] net/url

```okkama
object Url does TextRepresenting {
    Url(origin is TextRepresenting);

    scheme() is string;
    host() is string;
    path() is string;
    text() is string;
}

object UrlPort does Projecting<int> {
    UrlPort(origin is Url);
    value() is int;
}

object UrlQuery does Projecting<Query> {
    UrlQuery(origin is Url);
    value() is Query;
}

object ResolvedUrl does Projecting<Url> {
    ResolvedUrl(base is Url, relative is Url);
    value() is Url;
}
```

## - [ ] net/query

```okkama
object Query does TextRepresenting {
    Query(origin is TextRepresenting);

    contains(name is string) is bool;
    value(name is string) is string;
    values(name is string) is @string;
    text() is string;
}
```

## - [ ] net/mime

```okkama
object MediaType does TextRepresenting {
    MediaType(origin is TextRepresenting);

    type() is string;
    subtype() is string;
    parameter(name is string) is string;
    text() is string;
}
```

## - [ ] net/multipart

```okkama
object Multipart does Iterating<Part> {
    Multipart(origin is ByteRepresenting, boundary is string);
    cursor() is Cursor<Part>;
}

object Part {
    name() is string;
    data() is ByteRepresenting;
    media_type() is MediaType;
}
```

## - [ ] net/mail/address

```okkama
object MailAddress does TextRepresenting {
    MailAddress(origin is TextRepresenting);

    name() is string;
    email() is string;
    text() is string;
}
```

## - [ ] net/mail/message

```okkama
object MailMessage does ByteRepresenting {
    MailMessage(headers is MailHeaders, body is ByteRepresenting);
    bytes() is @byte;
}

object ParsedMail does Projecting<MailMessage> {
    ParsedMail(origin is ByteRepresenting);
    value() is MailMessage;
}
```

## - [ ] net/smtp

No `SmtpClient`.

```okkama
object SmtpDelivery does Projecting<MailReceipt> {
    SmtpDelivery(
        endpoint is Address,
        message is MailMessage,
        credentials is SmtpCredentials
    );

    value() is MailReceipt;
}
```

---

# Stage 22 — HTTP

## - [ ] net/http/request

```okkama
object Request does ByteRepresenting {
    Request(method is Method, url is Url, headers is Headers, body is ByteRepresenting);
    bytes() is @byte;
}

object RequestMethod does Projecting<Method> {
    RequestMethod(origin is Request);
    value() is Method;
}

object RequestUrl does Projecting<Url> {
    RequestUrl(origin is Request);
    value() is Url;
}

object RequestHeaders does Projecting<Headers> {
    RequestHeaders(origin is Request);
    value() is Headers;
}

object RequestBody does ByteRepresenting {
    RequestBody(origin is Request);
    bytes() is @byte;
}
```

## - [ ] net/http/response

```okkama
object Response does ByteRepresenting {
    Response(status is Status, headers is Headers, body is ByteRepresenting);
    bytes() is @byte;
}

object ResponseStatus does Projecting<Status> {
    ResponseStatus(origin is Response);
    value() is Status;
}

object ResponseHeaders does Projecting<Headers> {
    ResponseHeaders(origin is Response);
    value() is Headers;
}

object ResponseBody does ByteRepresenting {
    ResponseBody(origin is Response);
    bytes() is @byte;
}
```

## - [ ] net/http/remote-response

Replaces HTTP Client.

```okkama
object RemoteResponse does Projecting<Response> {
    RemoteResponse(request is Request);
    RemoteResponse(request is Request, timeout is Duration);
    value() is Response async;
}
```

## - [ ] net/http/headers

```okkama
object Headers {
    contains(name is string) is bool;
    value(name is string) is string;
    values(name is string) is @string;
    added(name is string, value is string) is Headers;
}
```

## - [ ] net/http/cookie

```okkama
object Cookie does TextRepresenting {
    Cookie(origin is TextRepresenting);

    name() is string;
    value() is string;
    text() is string;
}
```

## - [ ] net/http/cookies

```okkama
object Cookies does Iterating<Cookie> {
    Cookies(origin is Headers);
    cursor() is Cursor<Cookie>;
}
```

## - [ ] net/http/form

```okkama
object Form does ByteRepresenting {
    Form(fields is %string:@string);
    bytes() is @byte;
}

object ParsedForm {
    ParsedForm(origin is ByteRepresenting);

    contains(name is string) is bool;
    value(name is string) is string;
    values(name is string) is @string;
}
```

## - [ ] net/http/routes

No router/handler.

```okkama
role Responding {
    response(request is Request) is Response async;
}

object Routes does Responding {
    Routes(routes is %Route:Responding);
    response(request is Request) is Response async;
}
```

## - [ ] net/http/endpoint

No `HttpServer`.

```okkama
object HttpEndpoint {
    HttpEndpoint(endpoint is TcpEndpoint, resource is Responding);
    serve() async;
    close();
}
```

## - [ ] net/http/static

```okkama
object StaticFiles does Responding {
    StaticFiles(root is Directory);
    response(request is Request) is Response async;
}
```

## - [ ] net/http/json-response

```okkama
object JsonResponse does Projecting<Response> {
    JsonResponse(status is Status, value is JsonRepresenting);
    value() is Response;
}
```

## - [ ] net/http/redirect

```okkama
object Redirect does Projecting<Response> {
    Redirect(status is Status, location is Url);
    value() is Response;
}
```

## - [ ] net/http/auth

```okkama
object BasicCredentials does TextRepresenting {
    BasicCredentials(username is string, password is string);
    text() is string;
}

object BearerToken does TextRepresenting {
    BearerToken(token is string);
    text() is string;
}
```

## - [ ] net/http/proxy

```okkama
object ProxiedResponse does Projecting<Response> {
    ProxiedResponse(request is Request, proxy is Url);
    value() is Response async;
}
```

## - [ ] net/websocket

```okkama
object WebSocket {
    WebSocket(url is Url);

    receive() is WebSocketMessage async;
    send(message is WebSocketMessage) async;
    close();
}
```

## - [ ] net/robots

```okkama
object Robots {
    Robots(origin is TextRepresenting);

    allowed(agent is string, url is Url) is bool;
    crawl_delay(agent is string) is Duration;
    site_maps() is Sequencing<Url>;
}
```

## - [ ] net/jsonrpc

```okkama
object JsonRpcResponse does Projecting<JsonRepresenting> {
    JsonRpcResponse(endpoint is Url, method is string, parameters is JsonRepresenting);
    value() is JsonRepresenting async;
}
```

---

# Stage 23 — database

## - [ ] database/sql/database

```okkama
# naming candidates, best -> worst: Querying, Persisting, DataAccessing, Transacting, Storing
role Database {
    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}
```

## - [ ] database/sql/query

```okkama
object SqlQuery does TextRepresenting {
    SqlQuery(sql is string, parameters is @SqlValue);

    text() is string;
    parameters() is @SqlValue;
}
```

## - [ ] database/sql/rows

```okkama
object Rows does Iterating<Row> {
    cursor() is Cursor<Row>;
}
```

## - [ ] database/sql/row

```okkama
object Row {
    contains(column is string) is bool;
    value(column is string) is SqlValue;
    columns() is @string;
}
```

## - [ ] database/sql/transaction

```okkama
object Transaction does Database {
    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
    commit();
    rollback();
}
```

## - [ ] database/sql/effect

```okkama
object Effect {
    affected() is int;
    inserted_id() is int;                   # throws NoInsertedId
}
```

## - [ ] database/sqlite

```okkama
object Sqlite does Database {
    Sqlite(file is File);

    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}

object MemorySqlite does Database {
    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}
```

- [ ] Keep PostgreSQL/MySQL drivers third-party initially.

---

# Stage 24 — config / CLI / terminal

## - [ ] config/environment

```okkama
object EnvironmentConfig {
    EnvironmentConfig(environment is Environment, prefix is string);

    contains(name is string) is bool;
    text(name is string) is string;
    int(name is string) is int;
}
```

## - [ ] config/toml

```okkama
object TomlConfig {
    TomlConfig(document is TomlDocument);

    contains(name is string) is bool;
    text(name is string) is string;
    int(name is string) is int;
}
```

## - [ ] config/ini

```okkama
object IniConfig {
    IniConfig(document is IniDocument);

    contains(name is string) is bool;
    text(name is string) is string;
    int(name is string) is int;
}
```

## - [ ] cli/arguments

```okkama
object Arguments does Sequencing<string> {
    Arguments();

    at(index is int) is string;
    first() is string;
    last() is string;
    size() is int;
    empty() is bool;
    cursor() is Cursor<string>;
}
```

## - [ ] cli/options

```okkama
object Options {
    Options(arguments is Arguments, specification is OptionSpecification);

    contains(name is string) is bool;
    value(name is string) is string;
    rest() is @string;
}
```

## - [ ] cli/command

```okkama
role Executing {
    run(arguments is Arguments) is Exit;
}
```

Concrete objects should be domain commands such as `Compile`, `Test`, `Format`, never `CommandHandler`.

## - [ ] terminal

```okkama
object Terminal {
    width() is int;
    height() is int;
    interactive() is bool;
    password(prompt is TextRepresenting) is string;
}
```

## - [ ] terminal/styled

```okkama
object Bold does TextRepresenting { Bold(origin is TextRepresenting); text() is string; }
object Dim does TextRepresenting { Dim(origin is TextRepresenting); text() is string; }
object Underlined does TextRepresenting { Underlined(origin is TextRepresenting); text() is string; }
object Red does TextRepresenting { Red(origin is TextRepresenting); text() is string; }
object Green does TextRepresenting { Green(origin is TextRepresenting); text() is string; }
object Yellow does TextRepresenting { Yellow(origin is TextRepresenting); text() is string; }
object Blue does TextRepresenting { Blue(origin is TextRepresenting); text() is string; }
```

---

# Stage 25 — logging

## - [ ] log/record

```okkama
object Record does TextRepresenting {
    Record(level is Level, message is TextRepresenting, fields is %string:any);
    text() is string;
}
```

## - [ ] log/log

```okkama
role Logging {
    add(record is Record) is void;
}
```

## - [ ] log/text

```okkama
object TextLog does Logging {
    TextLog(output is Stdout);
    add(record is Record);
}
```

## - [ ] log/json

```okkama
object JsonLog does Logging {
    JsonLog(output is Stdout);
    add(record is Record);
}
```

## - [ ] log/file

```okkama
object FileLog does Logging {
    FileLog(file is File);
    add(record is Record);
}
```

## - [ ] log/filtered

```okkama
object FilteredLog does Logging {
    FilteredLog(origin is Logging, minimum is Level);
    add(record is Record);
}
```

## - [ ] log/prefixed

```okkama
object PrefixedLog does Logging {
    PrefixedLog(origin is Logging, prefix is TextRepresenting);
    add(record is Record);
}
```

## - [ ] log/rotating

```okkama
object RotatingLog does Logging {
    RotatingLog(origin is FileLog, maximum is int, backups is int);
    add(record is Record);
}
```

---

# Stage 26 — templates / locale

## - [ ] template/text

```okkama
object Template does TextRepresenting {
    Template(source is TextRepresenting, values is %string:any);
    text() is string;
}
```

## - [ ] template/html

```okkama
object HtmlTemplate does TextRepresenting {
    HtmlTemplate(source is TextRepresenting, values is %string:any);
    text() is string;                       # automatic escaping
}
```

## - [ ] locale

```okkama
object Locale does TextRepresenting {
    Locale(origin is TextRepresenting);
    text() is string;
}
```

## - [ ] locale/number

```okkama
object LocalInteger does TextRepresenting {
    LocalInteger(value is int, locale is Locale);
    text() is string;
}

object LocalDecimal does TextRepresenting {
    LocalDecimal(value is Decimal, locale is Locale);
    text() is string;
}
```

## - [ ] locale/currency

```okkama
object LocalCurrency does TextRepresenting {
    LocalCurrency(amount is Decimal, currency is string, locale is Locale);
    text() is string;
}
```

## - [ ] locale/messages

```okkama
object Messages {
    Messages(locale is Locale, catalog is MessageCatalog);

    text(key is string) is string;
    plural(key is string, amount is int) is string;
}
```

---

# Stage 27 — testing

## - [ ] testing/assertions

Prefer language-native test assertions over an `Assert` helper.

- [ ] equality assertion
- [ ] boolean assertion
- [ ] ordering assertion
- [ ] typed-error assertion
- [ ] no-error assertion

## - [ ] testing/fake-clock

```okkama
object FakeClock does Timing {
    FakeClock(now is DateTime, monotonic is Instant);

    now() is DateTime;
    monotonic() is Instant;
}
```

Time evolution must use successors, not setters.

## - [ ] testing/fake-database

```okkama
object FakeDatabase does Database {
    FakeDatabase(rows is %SqlQuery:Rows);

    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}
```

## - [ ] testing/fake-log

```okkama
object FakeLog does Logging {
    add(record is Record);
}
```

Do not introduce shared mutable in-memory storage solely to make this fake work; use scenario/predefined objects if necessary.

## - [ ] testing/http-resource

```okkama
object StaticResource does Responding {
    StaticResource(response is Response);
    response(request is Request) is Response async;
}
```

## - [ ] testing/property

```okkama
role Sampling<T> does Iterating<T> {}

object RandomInts does Sampling<int> {
    RandomInts(random is Random, count is int, minimum is int, maximum is int);
    cursor() is Cursor<int>;
}

object Property<T> does Projecting<bool> {
    Property(samples is Sampling<T>, condition is action(T) is bool);
    value() is bool;
}
```

## - [ ] testing/benchmark

```okkama
object Measurement {
    duration() is Duration;
    iterations() is int;
}

object Benchmark does Projecting<Measurement> {
    Benchmark(action is action(), iterations is int, clock is Timing);
    value() is Measurement;
}
```

## - [ ] testing/fuzz

```okkama
object FuzzCases<T> does Iterating<T> {
    FuzzCases(seed is T, mutation is action(T) is T, count is int);
    cursor() is Cursor<T>;
}
```

---

# Stage 28 — diagnostics / observability

## - [ ] diagnostics/stack

```okkama
object StackTrace does TextRepresenting, Iterating<StackFrame> {
    text() is string;
    cursor() is Cursor<StackFrame>;
}
```

## - [ ] diagnostics/cpu-profile

```okkama
object CpuProfile does ByteRepresenting {
    CpuProfile(duration is Duration);
    bytes() is @byte;
}
```

## - [ ] diagnostics/memory-profile

```okkama
object MemoryProfile does ByteRepresenting {
    bytes() is @byte;
}
```

## - [ ] diagnostics/timing

```okkama
object TimedAction does Projecting<Duration> {
    TimedAction(action is action(), clock is Timing);
    value() is Duration;
}
```

## - [ ] diagnostics/warning

```okkama
object Warning does Error {
    text() is string;
}
```

## - [ ] metrics

```okkama
# naming candidates, best -> worst: Measuring, Observing, Instrumenting, Reporting, Recording
role Metric {
    name() is string;
}

object Counter does Metric {
    name() is string;
    increment();
    add(value is int);
}

object Gauge does Metric {
    name() is string;
    set(value is float);
}

object Histogram does Metric {
    name() is string;
    observe(value is float);
}
```

These are instruments/resources, not worker objects.

## - [ ] tracing/span

```okkama
object Span {
    child(name is string) is Span;
    event(name is string);
    finish();
}
```

Keep tracing API intentionally small until semantics are concrete.

---

# Stage 29 — image / audio

## - [ ] image

```okkama
# naming candidates, best -> worst: Imaging, PixelProviding, RasterRepresenting, Picturing, Rendering
role Image {
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/png

```okkama
object Png does ByteRepresenting {
    Png(origin is Image);
    bytes() is @byte;
}

object PngImage does Image {
    PngImage(origin is ByteRepresenting);
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/jpeg

```okkama
object Jpeg does ByteRepresenting {
    Jpeg(origin is Image, quality is int);
    bytes() is @byte;
}

object JpegImage does Image {
    JpegImage(origin is ByteRepresenting);
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/gif

```okkama
object Gif does ByteRepresenting {
    Gif(origin is Image);
    bytes() is @byte;
}

object GifImage does Image {
    GifImage(origin is ByteRepresenting);
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/color

```okkama
object RGB {
    red() is int;
    green() is int;
    blue() is int;
}

object RGBA {
    red() is int;
    green() is int;
    blue() is int;
    alpha() is int;
}
```

## - [ ] audio/wav

```okkama
# naming candidates, best -> worst: Sampling, AudioRepresenting, SoundRepresenting, Sounding, AudioSampling
role Audio {
    samples() is Iterating<Sample>;
    rate() is int;
    channels() is int;
}

object Wav does ByteRepresenting {
    Wav(audio is Audio);
    bytes() is @byte;
}

object WavAudio does Audio {
    WavAudio(origin is ByteRepresenting);
    samples() is Iterating<Sample>;
    rate() is int;
    channels() is int;
}
```

---

# Stage 30 — reflection / modules / versions

## - [ ] reflection policy

- [ ] No general runtime reflection in stdlib 1.0.
- [ ] No private-field inspection.
- [ ] No invocation by string.
- [ ] No mutation through reflection.
- [ ] Compiler tooling may inspect source/compiler models, not live object internals.

## - [ ] module/resource

```okkama
object Responding does ByteRepresenting, TextRepresenting {
    Responding(module is Module, path is Path);

    bytes() is @byte;
    text() is string;
}
```

## - [ ] module/metadata

```okkama
object ModuleMetadata {
    name() is string;
    version() is Version;
    dependencies() is Sequencing<Dependency>;
}
```

## - [ ] version

```okkama
object Version does Comparing<Version>, TextRepresenting {
    Version(origin is TextRepresenting);

    compare(other is Version) is Ordering;
    text() is string;
}
```

## - [ ] version/constraint

```okkama
object VersionConstraint does TextRepresenting {
    VersionConstraint(origin is TextRepresenting);

    matches(version is Version) is bool;
    text() is string;
}
```

---

# Stage 31 — big numbers / cache / FFI

## - [ ] numbers/bigint

```okkama
object BigInt does Comparing<BigInt>, TextRepresenting {
    BigInt(origin is TextRepresenting);

    plus(other is BigInt) is BigInt;
    minus(other is BigInt) is BigInt;
    multiplied(other is BigInt) is BigInt;
    divided(other is BigInt) is BigInt;
    compare(other is BigInt) is Ordering;
    text() is string;
}

object BigPower does Projecting<BigInt> {
    BigPower(base is BigInt, exponent is BigInt);
    value() is BigInt;
}

object BigGcd does Projecting<BigInt> {
    BigGcd(left is BigInt, right is BigInt);
    value() is BigInt;
}
```

## - [ ] numbers/bigrational

```okkama
object BigRational does Comparing<BigRational>, TextRepresenting {
    BigRational(numerator is BigInt, denominator is BigInt);

    plus(other is BigRational) is BigRational;
    minus(other is BigRational) is BigRational;
    multiplied(other is BigRational) is BigRational;
    divided(other is BigRational) is BigRational;
    compare(other is BigRational) is Ordering;
    text() is string;
}
```

## - [ ] cache policy

```okkama
role Caching<K,V> {
    contains(key is K) is bool;
    value(key is K) is V;
    put(key is K, value is V) is void;
}
```

- [ ] Do not force an in-memory mutable cache into the language if it conflicts with Okkama state semantics.
- [ ] Prefer external/process cache resources later.
- [ ] No `CacheManager`.

## - [ ] ffi/library

```okkama
object Library {
    Library(path is Path);

    symbol(name is string) is ForeignFunction;
    close();
}
```

## - [ ] ffi/function

```okkama
object ForeignFunction {
    call(arguments is @ForeignValue) is ForeignValue;
}
```

## - [ ] ffi/pointer

```okkama
object Pointer<T> {
    address() is int;
    value() is T;
}
```

- [ ] Keep raw mutation out until memory/FFI semantics are deliberately designed.

---

# Stage 32 — Okkama language tooling

## - [ ] okkama/source

```okkama
object Source does TextRepresenting {
    Source(path is Path);
    text() is string;
}

object Position {
    file() is Path;
    line() is int;
    column() is int;
}

object Span {
    start() is Position;
    end() is Position;
}
```

## - [ ] okkama/tokens

No Lexer.

```okkama
object Tokens does Sequencing<Token> {
    Tokens(source is Source);

    at(index is int) is Token;
    first() is Token;
    last() is Token;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Token>;
}
```

## - [ ] okkama/program

No Parser.

```okkama
object Program {
    Program(tokens is Tokens);

    modules() is Sequencing<ModuleNode>;
    diagnostics() is Sequencing<Diagnostic>;
}
```

## - [ ] okkama/typed

No TypeChecker worker.

```okkama
object TypedProgram {
    TypedProgram(origin is Program);

    program() is Program;
    diagnostics() is Sequencing<Diagnostic>;
}
```

## - [ ] okkama/formatted

No Formatter.

```okkama
object FormattedSource does TextRepresenting {
    FormattedSource(origin is Source);
    text() is string;
}
```

## - [ ] okkama/documentation

```okkama
object Documentation does TextRepresenting {
    Documentation(program is Program);
    text() is string;
}
```

## - [ ] okkama/bytecode

No generic Compiler worker.

```okkama
object Bytecode does ByteRepresenting {
    Bytecode(program is TypedProgram);
    bytes() is @byte;
}
```

## - [ ] okkama/disassembly

```okkama
object Disassembly does TextRepresenting {
    Disassembly(bytecode is Bytecode);
    text() is string;
}
```

---

# Stage 33 — project / package / embed

## - [ ] okkama/project

```okkama
object Project {
    Project(root is Directory);

    name() is string;
    version() is Version;
    modules() is Sequencing<Module>;
}
```

## - [ ] okkama/package

```okkama
object Package does ByteRepresenting {
    Package(project is Project);
    bytes() is @byte;
}
```

## - [ ] okkama/build

No Builder.

```okkama
object Build does Projecting<BuildResult> {
    Build(project is Project, target is BuildTarget);
    value() is BuildResult;
}
```

## - [ ] embed

```okkama
object EmbeddedFile does ByteRepresenting, TextRepresenting {
    EmbeddedFile(path is string);

    bytes() is @byte;
    text() is string;
}

object EmbeddedDirectory does Iterating<EmbeddedResource> {
    EmbeddedDirectory(path is string);
    cursor() is Cursor<EmbeddedResource>;
}
```

---

# Stage 34 — platform-specific

## - [ ] os/unix

- [ ] UID/GID objects.
- [ ] POSIX permissions.
- [ ] resource limits.
- [ ] pipes only after a proper domain model exists.
- [ ] raw `fork()` only after Okkama process/async semantics are final.
- [ ] raw `ioctl()` stays FFI/platform-low-level, not general stdlib.

## - [ ] os/windows/registry

```okkama
object RegistryKey {
    RegistryKey(root is RegistryRoot, path is string);

    contains(name is string) is bool;
    value(name is string) is RegistryValue;
    close();
}
```

---

# Stage 35 — deliberately excluded from core stdlib

- [ ] NO pickle-style arbitrary object serialization.
- [ ] NO shelve/dbm/marshal runtime-object persistence.
- [ ] NO generic utility modules (`utils`, `helpers`, `common`).
- [ ] NO generic Reader/Writer/Parser/Encoder/Decoder abstractions.
- [ ] NO ORM / ActiveRecord.
- [ ] NO reflection-based DI.
- [ ] NO mocking framework.
- [ ] NO FTP/Telnet/NNTP/POP in core initially.
- [ ] NO GUI toolkit in core initially.
- [ ] NO YAML in core.
- [ ] NO PostgreSQL/MySQL drivers in core initially.
- [ ] NO mutable synchronization zoo until Okkama concurrency semantics demand it.

---

# Stage 36 — coverage checklist

## Python breadth

- [ ] text processing
- [ ] binary data
- [ ] numeric/math
- [ ] iteration/combinatorics
- [ ] collections
- [ ] time/date/calendar/timezone
- [ ] filesystem/path/temp/glob/watch/mmap
- [ ] OS/environment/users/process/signals
- [ ] compression
- [ ] archives
- [ ] SQLite/database contracts
- [ ] JSON/CSV/TOML/INI/XML/HTML
- [ ] base encodings/binary encodings/PEM/ASN.1
- [ ] checksums/digests/crypto
- [ ] DNS/IP/TCP/UDP/Unix sockets
- [ ] URL/MIME/mail/SMTP
- [ ] HTTP/WebSocket/robots/RPC
- [ ] async/concurrency
- [ ] CLI/terminal
- [ ] logging
- [ ] config
- [ ] templates
- [ ] locale/i18n
- [ ] testing/property/fuzz/benchmark
- [ ] diagnostics/profiling/metrics/tracing
- [ ] image/audio basics
- [ ] resources/module metadata/versioning
- [ ] FFI/platform
- [ ] language/compiler tooling

## Go structure

- [ ] cohesive packages
- [ ] package paths generally <= 2 levels
- [ ] small cross-package contracts
- [ ] OS-specific APIs isolated
- [ ] testing support close to domain
- [ ] source/compiler models first-class
- [ ] no package becomes a dumping ground
- [ ] adjacent packages compose without orchestration objects

## EO review for every public object

- [ ] Named by what it **is**, not what it **does**.
- [ ] No avoidable `-er/-or` job-title name.
- [ ] Declarative object possible instead? (`Sorter -> Sorted`, `FileReader -> DataFile`)
- [ ] No utility/static/free procedural API.
- [ ] Immutable.
- [ ] Constructor only binds.
- [ ] No getters/setters as exposed state plumbing.
- [ ] Behavior, not data-bag design.
- [ ] Small public surface.
- [ ] Contract exists where polymorphism matters.
- [ ] Decorators preferred over flags/settings.
- [ ] Typed, fail-fast errors.
- [ ] No hidden null/optional assumption.
- [ ] Fakes instead of mocks.
- [ ] No implementation inheritance.
- [ ] No runtime reflection/casting.
- [ ] Composition instead of orchestration.

---

# Definition of done per item

- [ ] EO naming review
- [ ] API contract approved
- [ ] implementation
- [ ] positive tests
- [ ] boundary tests
- [ ] typed-error tests
- [ ] immutability tests
- [ ] composition test with adjacent stdlib object
- [ ] runnable example
- [ ] minimal docs
- [ ] no duplicate responsibility elsewhere
